// Season stats table image — shows per-user stats across all completed periods

use crate::db::gym::queries;
use crate::db::Database;
use crate::images::{escape_svg, render_svg_to_png};
use crate::images::gym::summary::capitalize_first;
use poise::serenity_prelude as serenity;
use std::collections::HashMap;


pub struct SeasonUserRow {
    pub name: String,
    pub total: i32,
    pub goals_met: i32,
    pub goals_missed: i32,
    /// Per-type counts this season (activity_type → count)
    pub type_counts: HashMap<String, i32>,
}

pub async fn build_season_stats_png(
    db: &Database,
    http: &serenity::Http,
    guild_id: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let (user_stats, type_stats, week_count, active_types, season_label, season_date_range) = {
        let conn = db.conn();

        // Determine which season to show (current season, or all-time if none)
        let (season_id, season_label, season_date_range) = match queries::get_current_season(&conn, guild_id)? {
            Some(s) => {
                let start = &s.start_time[..10];
                let range = match &s.end_time {
                    None => format!("{} → today", start),
                    Some(end) => format!("{} → {}", start, &end[..10]),
                };
                (Some(s.id), s.name.clone(), range)
            }
            None => (None, "Season Stats".to_string(), String::new()),
        };

        let user_stats = queries::get_season_user_stats(&conn, guild_id, season_id)?;
        if user_stats.is_empty() {
            return Err("No completed weeks yet — run a rollover first.".into());
        }

        let type_stats = queries::get_season_type_stats(&conn, guild_id, season_id)?;
        let week_count = queries::get_completed_period_count(&conn, guild_id, season_id)?;

        // Activity types that at least one user logged this season, sorted by total usage
        let mut type_usage: HashMap<String, i32> = HashMap::new();
        for (_, per_user) in &type_stats {
            for (t, c) in per_user {
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

        (user_stats, type_stats, week_count, active_types, season_label, season_date_range)
    };

    // Fetch Discord names outside DB scope
    let guild = serenity::GuildId::new(guild_id);
    let mut rows = Vec::new();
    for (user_id, total, met, missed) in user_stats {
        let name = match guild.member(http, serenity::UserId::new(user_id)).await {
            Ok(m) => m.display_name().to_string(),
            Err(_) => format!("User {}", user_id),
        };
        let type_counts = type_stats.get(&user_id).cloned().unwrap_or_default();
        rows.push(SeasonUserRow { name, total, goals_met: met, goals_missed: missed, type_counts });
    }

    generate_season_table(&rows, &active_types, week_count, &season_label, &season_date_range)
}

pub fn generate_season_table(
    rows: &[SeasonUserRow],
    activity_types: &[String],
    week_count: i32,
    title: &str,
    date_range: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    const PADDING: u32 = 12;
    const HEADER_H: u32 = 56;
    const ROW_H: u32 = 34;
    const COL_HDR_H: u32 = 32;
    const NAME_W: u32 = 130;
    const TOTAL_W: u32 = 54;
    const MET_W: u32 = 46;
    const MISSED_W: u32 = 52;
    const RATE_W: u32 = 54;
    const TYPE_W: u32 = 50;
    let n_types = activity_types.len() as u32;
    let stats_w = TOTAL_W + MET_W + MISSED_W + RATE_W;
    let types_w = n_types * TYPE_W;
    let table_w = NAME_W + stats_w + types_w;
    let image_w = PADDING * 2 + table_w;

    let image_h = HEADER_H + COL_HDR_H + (rows.len() as u32 * ROW_H) + PADDING;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"##,
        image_w, image_h
    ));
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="#2f3136"/>"##,
        image_w, image_h
    ));

    // Header
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="18" font-weight="bold" fill="#ffffff">{}</text>"##,
        PADDING, PADDING + 22, escape_svg(title)
    ));
    let week_label = if week_count == 1 { "1 week".to_string() } else { format!("{} weeks", week_count) };
    let subheader = if date_range.is_empty() {
        format!("{} of data", week_label)
    } else {
        format!("{} · {} of data", date_range, week_label)
    };
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="12" fill="#b9bbbe">{}</text>"##,
        PADDING, PADDING + 42, escape_svg(&subheader)
    ));

    // Column header row
    let col_hdr_y = HEADER_H;
    svg.push_str(&format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#202225"/>"##,
        PADDING, col_hdr_y, table_w, COL_HDR_H
    ));

    let col_text_y = col_hdr_y + 22;
    // Name header
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="12" font-weight="bold" fill="#dcddde">Name</text>"##,
        PADDING + 6, col_text_y
    ));

    let mut cx = PADDING + NAME_W;
    for &(label, w) in &[("Total", TOTAL_W), ("✓ Met", MET_W), ("✗ Miss", MISSED_W), ("Rate", RATE_W)] {
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="11" font-weight="bold" fill="#dcddde" text-anchor="middle">{}</text>"##,
            cx + w / 2, col_text_y, label
        ));
        cx += w;
    }
    for at in activity_types {
        let abbr = abbreviate(at, 5);
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="11" font-weight="bold" fill="#dcddde" text-anchor="middle">{}</text>"##,
            cx + TYPE_W / 2, col_text_y, escape_svg(&abbr)
        ));
        cx += TYPE_W;
    }

    // Data rows
    for (i, row) in rows.iter().enumerate() {
        let row_y = HEADER_H + COL_HDR_H + i as u32 * ROW_H;
        let row_bg = if i % 2 == 0 { "#2f3136" } else { "#36393f" };
        let text_y = row_y + 22;

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"##,
            PADDING, row_y, table_w, ROW_H, row_bg
        ));

        // Name
        let display_name = truncate_name(&row.name, 14);
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="13" font-weight="bold" fill="#ffffff">{}</text>"##,
            PADDING + 6, text_y, escape_svg(&display_name)
        ));

        let mut cx = PADDING + NAME_W;

        // Total
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="14" font-weight="bold" fill="#ffffff" text-anchor="middle">{}</text>"##,
            cx + TOTAL_W / 2, text_y, row.total
        ));
        cx += TOTAL_W;

        // Goals met (green)
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="14" font-weight="bold" fill="#43b581" text-anchor="middle">{}</text>"##,
            cx + MET_W / 2, text_y, row.goals_met
        ));
        cx += MET_W;

        // Goals missed (red)
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="14" font-weight="bold" fill="#f04747" text-anchor="middle">{}</text>"##,
            cx + MISSED_W / 2, text_y, row.goals_missed
        ));
        cx += MISSED_W;

        // Rate %
        let total_weeks = row.goals_met + row.goals_missed;
        let rate = if total_weeks > 0 { row.goals_met * 100 / total_weeks } else { 0 };
        let rate_color = if rate >= 75 { "#43b581" } else if rate >= 50 { "#faa61a" } else { "#f04747" };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="13" font-weight="bold" fill="{}" text-anchor="middle">{}%</text>"##,
            cx + RATE_W / 2, text_y, rate_color, rate
        ));
        cx += RATE_W;

        // Per-type counts
        for at in activity_types {
            let count = row.type_counts.get(at).copied().unwrap_or(0);
            let (tc, tw) = if count > 0 { ("#dcddde", "normal") } else { ("#4f545c", "normal") };
            let display = if count > 0 { count.to_string() } else { "—".to_string() };
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="13" font-weight="{}" fill="{}" text-anchor="middle">{}</text>"##,
                cx + TYPE_W / 2, text_y, tw, tc, display
            ));
            cx += TYPE_W;
        }
    }

    // Separator line between header and data
    svg.push_str(&format!(
        r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#40444b" stroke-width="1"/>"##,
        PADDING, HEADER_H + COL_HDR_H,
        PADDING + table_w, HEADER_H + COL_HDR_H
    ));

    svg.push_str("</svg>");
    render_svg_to_png(&svg, image_w, image_h)
}

fn abbreviate(s: &str, max: usize) -> String {
    let cap = capitalize_first(s);
    let chars: Vec<char> = cap.chars().collect();
    if chars.len() <= max {
        cap
    } else {
        chars[..max].iter().collect()
    }
}

fn truncate_name(name: &str, max_chars: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max_chars {
        name.to_string()
    } else {
        format!("{}…", chars[..max_chars - 1].iter().collect::<String>())
    }
}
