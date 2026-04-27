// Weekly summary image generation

use crate::db::gym::queries;
use crate::db::Database;
use crate::images::{escape_svg, render_svg_to_png};
use poise::serenity_prelude as serenity;
use rusqlite::params;
use std::collections::HashMap;

/// A single sub-goal (per type or per group) for display on a card
pub struct SubGoal {
    pub label: String,  // e.g. "Gym" or "Push"
    pub target: i32,
    pub actual: i32,
    pub met: bool,
}

/// Data for a single user's weekly summary card
pub struct UserSummary {
    pub name: String,
    pub total: i32,
    pub effective_goal: i32,
    pub goal_met: bool,
    pub type_counts: Vec<(String, i32)>,
    pub sub_goals: Vec<SubGoal>,  // empty for Total mode
}

pub async fn build_period_summary_png(
    db: &Database,
    http: &serenity::Http,
    guild_id: u64,
    period: &crate::db::gym::models::Period,
    title: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let (users_data, active_types) = {
        let conn = db.conn();
        let user_ids = queries::get_users(&conn, guild_id)?;
        let type_group_map = queries::get_all_type_groups(&conn, guild_id)?;

        let mut users_data: Vec<(u64, UserSummary)> = Vec::new();
        for user_id in user_ids {
            let total = queries::get_user_period_count(&conn, period.id, user_id)?;
            let type_counts = queries::get_user_period_type_counts(&conn, period.id, user_id)?;
            let goal_config = queries::get_user_goal_config(&conn, guild_id, user_id)?;

            let goal_met = evaluate_goal_met(
                &conn, guild_id, user_id, total, &type_counts, &goal_config, &type_group_map,
            )?;
            let effective_goal = compute_effective_goal(&goal_config);
            let sub_goals = collect_sub_goals(
                &conn, guild_id, user_id, &type_counts, &type_group_map,
            )?;

            let type_vec: Vec<_> = type_counts.into_iter().collect();
            users_data.push((user_id, UserSummary {
                name: String::new(), // filled after DB scope
                total,
                effective_goal,
                goal_met,
                type_counts: type_vec,
                sub_goals,
            }));
        }

        // Active types = any type logged by any user this period, sorted by usage desc
        let mut type_usage: HashMap<String, i32> = HashMap::new();
        for (_, s) in &users_data {
            for (t, c) in &s.type_counts {
                *type_usage.entry(t.clone()).or_insert(0) += c;
            }
        }
        let mut active_types: Vec<String> = queries::get_activity_types(&conn, guild_id)?
            .into_iter()
            .filter(|t| type_usage.get(t).copied().unwrap_or(0) > 0)
            .collect();
        active_types.sort_by(|a, b| {
            type_usage.get(b).unwrap_or(&0).cmp(type_usage.get(a).unwrap_or(&0))
        });

        (users_data, active_types)
    };

    // Fetch Discord display names outside DB scope
    let guild_snowflake = serenity::GuildId::new(guild_id);
    let mut user_summaries = Vec::new();
    for (user_id, mut summary) in users_data {
        summary.name = match guild_snowflake.member(http, serenity::UserId::new(user_id)).await {
            Ok(member) => member.display_name().to_string(),
            Err(_) => format!("User {}", user_id),
        };
        user_summaries.push(summary);
    }
    user_summaries.sort_by(|a, b| b.total.cmp(&a.total));

    let period_str = format!("{} to {}", &period.start_time[..10], &period.end_time[..10]);
    generate_summary_image(title, &period_str, &user_summaries, &active_types)
}

fn collect_sub_goals(
    conn: &rusqlite::Connection,
    guild_id: u64,
    user_id: u64,
    type_counts: &HashMap<String, i32>,
    type_group_map: &HashMap<String, String>,
) -> Result<Vec<SubGoal>, rusqlite::Error> {
    let mut sub_goals: Vec<SubGoal> = Vec::new();

    // Type-specific minimums (if any)
    let mut stmt = conn.prepare(
        "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ? ORDER BY activity_type"
    )?;
    let type_goals: Vec<(String, i32)> = stmt
        .query_map(params![guild_id, user_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (t, g) in type_goals {
        let actual = type_counts.get(&t).copied().unwrap_or(0);
        sub_goals.push(SubGoal { label: capitalize_first(&t), target: g, actual, met: actual >= g });
    }

    // Group-specific minimums (if any)
    let group_goals = queries::get_user_group_goals(conn, guild_id, user_id)?;
    for (grp, g) in group_goals {
        let actual: i32 = type_counts.iter()
            .filter(|(t, _)| type_group_map.get(*t).map(|gr| gr == &grp).unwrap_or(false))
            .map(|(_, c)| c)
            .sum();
        sub_goals.push(SubGoal { label: capitalize_first(&grp), target: g, actual, met: actual >= g });
    }

    Ok(sub_goals)
}

fn compute_effective_goal(goal_config: &Option<crate::db::gym::models::UserGoalConfig>) -> i32 {
    goal_config.as_ref().map(|gc| gc.total_goal).unwrap_or(5)
}

pub fn evaluate_goal_met(
    conn: &rusqlite::Connection,
    guild_id: u64,
    user_id: u64,
    total: i32,
    type_counts: &HashMap<String, i32>,
    goal_config: &Option<crate::db::gym::models::UserGoalConfig>,
    type_group_map: &HashMap<String, String>,
) -> Result<bool, rusqlite::Error> {
    let total_goal = goal_config.as_ref().map(|gc| gc.total_goal).unwrap_or(5);

    // 1. Total minimum always required
    if total < total_goal {
        return Ok(false);
    }

    // 2. Any per-type minimums (additive — all must be met)
    let mut stmt = conn.prepare(
        "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ?"
    )?;
    let type_goals: Vec<(String, i32)> = stmt
        .query_map(params![guild_id, user_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (t, g) in &type_goals {
        if type_counts.get(t).copied().unwrap_or(0) < *g {
            return Ok(false);
        }
    }

    // 3. Any per-group minimums (additive — all must be met)
    let group_goals = queries::get_user_group_goals(conn, guild_id, user_id)?;
    for (grp, goal) in &group_goals {
        let group_total: i32 = type_counts.iter()
            .filter(|(t, _)| type_group_map.get(*t).map(|g| g == grp).unwrap_or(false))
            .map(|(_, c)| c)
            .sum();
        if group_total < *goal {
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn generate_summary_image(
    title: &str,
    period_str: &str,
    users: &[UserSummary],
    activity_types: &[String],
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    const IMAGE_W: u32 = 500;
    const MARGIN: u32 = 10;
    const CARD_W: u32 = IMAGE_W - MARGIN * 2;    // 480
    const CARD_PAD: u32 = 14;
    const BORDER_W: u32 = 6;                      // colored left stripe
    const INNER_W: u32 = CARD_W - CARD_PAD * 2;  // 452
    const HEADER_H: u32 = 70;
    const NAME_H: u32 = 32;
    const BAR_H: u32 = 12;
    const BAR_MARGIN: u32 = 5;
    const SUB_SECTION_GAP: u32 = 6;              // gap before sub-goals
    const SUB_ROW_H: u32 = 22;                   // height per sub-goal row
    const SUB_LABEL_W: u32 = 75;
    const SUB_COUNT_W: u32 = 60;
    const SUB_BAR_W: u32 = INNER_W - SUB_LABEL_W - SUB_COUNT_W; // 317
    const CHIP_H: u32 = 24;
    const CHIPS_PER_ROW: u32 = 4;
    const CHIP_W: u32 = INNER_W / CHIPS_PER_ROW;  // 113
    const CHIP_SECTION_GAP: u32 = 8;              // gap before chips
    const GOAL_LABEL_H: u32 = 20;                  // "Goal: N" row for Total mode
    const CARD_GAP: u32 = 8;
    const CARD_TOP_PAD: u32 = 10;
    const CARD_BOT_PAD: u32 = 10;

    let n_active = activity_types.len() as u32;
    let n_type_rows = if n_active == 0 { 0 } else { (n_active + CHIPS_PER_ROW - 1) / CHIPS_PER_ROW };

    // Compute per-card height (variable because sub-goals differ per user)
    let card_height = |user: &UserSummary| -> u32 {
        let n_sub = user.sub_goals.len() as u32;
        // Total mode shows a "Goal: N" label instead of sub-goal bars
        let goal_row_h = if user.sub_goals.is_empty() { GOAL_LABEL_H } else { 0 };
        let sub_h = if n_sub > 0 { SUB_SECTION_GAP + n_sub * SUB_ROW_H } else { 0 };
        let chip_h = if n_type_rows > 0 { CHIP_SECTION_GAP + n_type_rows * CHIP_H } else { 0 };
        CARD_TOP_PAD + NAME_H + BAR_MARGIN + BAR_H + BAR_MARGIN
            + goal_row_h
            + sub_h
            + chip_h
            + CARD_BOT_PAD
    };

    let card_heights: Vec<u32> = users.iter().map(|u| card_height(u)).collect();
    let total_cards_h: u32 = if users.is_empty() {
        0
    } else {
        card_heights.iter().sum::<u32>() + (users.len() as u32 - 1) * CARD_GAP
    };
    let image_h = HEADER_H + MARGIN + total_cards_h + MARGIN;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"##,
        IMAGE_W, image_h
    ));
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="#2f3136"/>"##,
        IMAGE_W, image_h
    ));

    // Header
    svg.push_str(&format!(
        r##"<text x="{}" y="32" font-family="DejaVu Sans" font-size="20" font-weight="bold" fill="#ffffff">{}</text>"##,
        MARGIN + 4, escape_svg(title)
    ));
    svg.push_str(&format!(
        r##"<text x="{}" y="54" font-family="DejaVu Sans" font-size="13" fill="#b9bbbe">{}</text>"##,
        MARGIN + 4, escape_svg(period_str)
    ));

    if users.is_empty() {
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="14" fill="#72767d">No users tracked this period.</text>"##,
            MARGIN + 4, HEADER_H + MARGIN + 20
        ));
        svg.push_str("</svg>");
        return render_svg_to_png(&svg, IMAGE_W, image_h);
    }

    let mut card_y = HEADER_H + MARGIN;

    for (i, user) in users.iter().enumerate() {
        let ch = card_heights[i];
        let clip_id = format!("c{}", i);

        // Clip path for rounded corners on the left border stripe
        svg.push_str(&format!(
            r##"<defs><clipPath id="{}"><rect x="{}" y="{}" width="{}" height="{}" rx="6"/></clipPath></defs>"##,
            clip_id, MARGIN, card_y, CARD_W, ch
        ));

        // Card background — subtle tint based on goal status
        let card_bg = if user.goal_met { "#1a2e20" } else { "#2e1a1a" };
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}"/>"##,
            MARGIN, card_y, CARD_W, ch, card_bg
        ));

        // Colored left stripe (clipped to card shape)
        let stripe_color = if user.goal_met { "#43b581" } else { "#f04747" };
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" clip-path="url(#{})"/>"##,
            MARGIN, card_y, BORDER_W, ch, stripe_color, clip_id
        ));

        let name_text_y = card_y + CARD_TOP_PAD + 24;
        let left_x = MARGIN + CARD_PAD + BORDER_W;
        let right_x = MARGIN + CARD_W - CARD_PAD;

        // Username
        let display_name = truncate_name(&user.name, 17);
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="15" font-weight="bold" fill="#ffffff">{}</text>"##,
            left_x, name_text_y, escape_svg(&display_name)
        ));

        // Goal count X/Y right-aligned
        let count_str = format!("{}/{}", user.total, user.effective_goal);
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="14" fill="#dcddde" text-anchor="end">{}</text>"##,
            right_x, name_text_y, escape_svg(&count_str)
        ));

        // ✓/✗ just left of the count
        let count_px_w = count_str.len() as u32 * 8 + 6;
        let (status_sym, status_color) = if user.goal_met { ("✓", "#43b581") } else { ("✗", "#f04747") };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="18" font-weight="bold" fill="{}" text-anchor="end">{}</text>"##,
            right_x - count_px_w, name_text_y, status_color, status_sym
        ));

        // Main progress bar
        let bar_x = left_x;
        let bar_y = card_y + CARD_TOP_PAD + NAME_H + BAR_MARGIN;
        let bar_w = CARD_W - CARD_PAD - BORDER_W - CARD_PAD; // inner width
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="#36393f" stroke="#4f545c" stroke-width="1"/>"##,
            bar_x, bar_y, bar_w, BAR_H
        ));
        if user.total == 0 {
            // Empty state: centered "no workouts" label inside the bar
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="10" fill="#72767d" text-anchor="middle" dominant-baseline="middle">no workouts</text>"##,
                bar_x + bar_w / 2, bar_y + BAR_H / 2
            ));
        } else if user.effective_goal > 0 {
            let fill_ratio = (user.total as f32 / user.effective_goal as f32).min(1.0);
            let fill_w = (fill_ratio * bar_w as f32) as u32;
            if fill_w > 0 {
                let bar_color = if user.goal_met { "#43b581" } else { "#faa61a" };
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{}"/>"##,
                    bar_x, bar_y, fill_w, BAR_H, bar_color
                ));
            }
        }

        let mut section_y = card_y + CARD_TOP_PAD + NAME_H + BAR_MARGIN + BAR_H + BAR_MARGIN;

        // For Total mode: show explicit "Goal: N workouts/week" label
        if user.sub_goals.is_empty() {
            let goal_text = format!("Goal: {} workouts/week", user.effective_goal);
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="11" fill="#72767d">{}</text>"##,
                left_x, section_y + 14, escape_svg(&goal_text)
            ));
            section_y += GOAL_LABEL_H;
        }

        // Sub-goal bars (by_type or by_group)
        if !user.sub_goals.is_empty() {
            section_y += SUB_SECTION_GAP;
            for sub in &user.sub_goals {
                let text_y = section_y + 16;
                let (sub_color, sub_sym) = if sub.met { ("#43b581", "✓") } else { ("#f04747", "✗") };

                // Label
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="12" fill="#b9bbbe">{}</text>"##,
                    left_x, text_y, escape_svg(&sub.label)
                ));

                // Mini bar background
                let mb_x = left_x + SUB_LABEL_W;
                let mb_y = section_y + 5;
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="8" rx="2" fill="#36393f"/>"##,
                    mb_x, mb_y, SUB_BAR_W
                ));
                // Mini bar fill
                if sub.target > 0 {
                    let ratio = (sub.actual as f32 / sub.target as f32).min(1.0);
                    let fw = (ratio * SUB_BAR_W as f32) as u32;
                    if fw > 0 {
                        svg.push_str(&format!(
                            r##"<rect x="{}" y="{}" width="{}" height="8" rx="2" fill="{}"/>"##,
                            mb_x, mb_y, fw, sub_color
                        ));
                    }
                }

                // Count + status
                let count_str = format!("{}/{} {}", sub.actual, sub.target, sub_sym);
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="12" font-weight="bold" fill="{}" text-anchor="end">{}</text>"##,
                    right_x, text_y, sub_color, escape_svg(&count_str)
                ));

                section_y += SUB_ROW_H;
            }
        }

        // Type chips — sparse grid, fixed positions per global ordering
        if n_type_rows > 0 {
            section_y += CHIP_SECTION_GAP;
            let user_counts: HashMap<&str, i32> = user.type_counts.iter()
                .map(|(t, c)| (t.as_str(), *c))
                .collect();
            let has_any = activity_types.iter().any(|t| user_counts.get(t.as_str()).copied().unwrap_or(0) > 0);
            if !has_any {
                // No types logged — show a placeholder in the first chip slot
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="12" fill="#4f545c" font-style="italic">nothing logged</text>"##,
                    left_x, section_y + 18
                ));
            } else {
                for (idx, activity_type) in activity_types.iter().enumerate() {
                    let count = user_counts.get(activity_type.as_str()).copied().unwrap_or(0);
                    if count == 0 { continue; }
                    let col = idx as u32 % CHIPS_PER_ROW;
                    let row = idx as u32 / CHIPS_PER_ROW;
                    let chip_x = left_x + col * CHIP_W;
                    let chip_y = section_y + row * CHIP_H + 18;
                    let label = format!("{}({})", capitalize_first(activity_type), count);
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="14" fill="#dcddde">{}</text>"##,
                        chip_x, chip_y, escape_svg(&label)
                    ));
                }
            }
        }

        card_y += ch + CARD_GAP;
    }

    svg.push_str("</svg>");
    render_svg_to_png(&svg, IMAGE_W, image_h)
}

fn truncate_name(name: &str, max_chars: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max_chars {
        name.to_string()
    } else {
        format!("{}…", chars[..max_chars - 1].iter().collect::<String>())
    }
}

pub fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
