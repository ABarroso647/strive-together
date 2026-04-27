use super::Context;
use crate::db::gym::queries;
use crate::Error;
use poise::serenity_prelude as serenity;
use std::collections::HashMap;

/// Show your current week progress
#[poise::command(slash_command, guild_only)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = ctx.author().id.get();

    let embed = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        let config = match queries::get_guild_config(&conn, guild_id)? {
            Some(c) => c,
            None => return Err("Gym tracker not set up.".into()),
        };

        if !config.started {
            return Err("Tracking hasn't started yet.".into());
        }

        // Check if user is registered
        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err("You're not in the gym tracker. Ask an admin to add you.".into());
        }

        // Get current period
        let period = match queries::get_current_period(&conn, guild_id)? {
            Some(p) => p,
            None => return Err("No active period.".into()),
        };

        // Get user's counts for this period
        let total_count = queries::get_user_period_count(&conn, period.id, user_id)?;
        let type_counts = queries::get_user_period_type_counts(&conn, period.id, user_id)?;

        // Get user's goal config
        let goal_config = queries::get_user_goal_config(&conn, guild_id, user_id)?
            .ok_or("Could not find your goal configuration.")?;

        // Determine goal status
        let type_group_map = queries::get_all_type_groups(&conn, guild_id)?;
        let goal_config_opt = Some(goal_config.clone());
        let goal_met = crate::images::gym::summary::evaluate_goal_met(
            &conn, guild_id, user_id, total_count, &type_counts, &goal_config_opt, &type_group_map,
        )?;

        // Build goal progress string: total + any type/group sub-constraints
        let mut progress_parts: Vec<String> = vec![
            format!("Total: {}/{}", total_count, goal_config.total_goal)
        ];
        let mut stmt = conn.prepare(
            "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ? ORDER BY activity_type"
        )?;
        let type_goals: Vec<(String, i32)> = stmt
            .query_map(rusqlite::params![guild_id, user_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        for (t, g) in &type_goals {
            let count = type_counts.get(t).copied().unwrap_or(0);
            let sym = if count >= *g { "✓" } else { "✗" };
            progress_parts.push(format!("{}: {}/{}{}", t, count, g, sym));
        }
        let group_goals = queries::get_user_group_goals(&conn, guild_id, user_id)?;
        for (grp, goal) in &group_goals {
            let group_total: i32 = type_counts.iter()
                .filter(|(t, _)| type_group_map.get(*t).map(|g| g == grp).unwrap_or(false))
                .map(|(_, c)| c)
                .sum();
            let sym = if group_total >= *goal { "✓" } else { "✗" };
            progress_parts.push(format!("{} group: {}/{}{}", grp, group_total, goal, sym));
        }
        let goal_progress = progress_parts.join("\n");

        // Build type breakdown string
        let type_breakdown = if type_counts.is_empty() {
            "No workouts logged yet".to_string()
        } else {
            let mut sorted: Vec<_> = type_counts.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            sorted.iter()
                .map(|(t, c)| format!("{}: {}", t, c))
                .collect::<Vec<_>>()
                .join(", ")
        };

        // Get period dates
        let period_str = format!(
            "{} to {}",
            &period.start_time[..10],
            &period.end_time[..10]
        );

        let status_emoji = if goal_met { "✅" } else { "⏳" };
        let color = if goal_met { 0x00ff00 } else { 0xffaa00 };

        serenity::CreateEmbed::new()
            .title(format!("{} Your Weekly Progress", status_emoji))
            .field("Period", period_str, true)
            .field("Total Workouts", total_count.to_string(), true)
            .field("Goal Progress", goal_progress, true)
            .field("Breakdown", type_breakdown, false)
            .color(color)
    };

    tracing::debug!("guild={} user={} cmd=status", guild_id, user_id);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Show the current week summary image
#[poise::command(slash_command, guild_only)]
pub async fn summary(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let period = {
        let db = &ctx.data().db;
        let conn = db.conn();
        let config = queries::get_guild_config(&conn, guild_id)?.ok_or("Gym tracker not set up.")?;
        if !config.started { return Err("Tracking hasn't started yet.".into()); }
        queries::get_current_period(&conn, guild_id)?.ok_or("No active period.")?
    };

    ctx.defer().await?;
    let image_data = crate::images::gym::summary::build_period_summary_png(
        &ctx.data().db,
        ctx.serenity_context().http.as_ref(),
        guild_id,
        &period,
        "Weekly Summary",
    ).await?;

    tracing::debug!("guild={} user={} cmd=summary", guild_id, ctx.author().id.get());
    let attachment = serenity::CreateAttachment::bytes(image_data, "summary.png");
    ctx.send(poise::CreateReply::default().attachment(attachment)).await?;
    Ok(())
}

/// Show season stats — totals, goal streaks, and activity breakdown since season start
#[poise::command(slash_command, guild_only)]
pub async fn totals(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    {
        let db = &ctx.data().db;
        let conn = db.conn();
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }
    }

    ctx.defer().await?;
    let image_data = crate::images::gym::season::build_season_stats_png(
        &ctx.data().db,
        ctx.serenity_context().http.as_ref(),
        guild_id,
    ).await?;

    tracing::debug!("guild={} user={} cmd=totals", guild_id, ctx.author().id.get());
    let attachment = serenity::CreateAttachment::bytes(image_data, "season_stats.png");
    ctx.send(poise::CreateReply::default().attachment(attachment)).await?;
    Ok(())
}


/// Week-by-week history. Optionally filter to a specific user or past season.
#[poise::command(slash_command, guild_only)]
pub async fn history(
    ctx: Context<'_>,
    #[description = "Show full breakdown for a specific user"] user: Option<serenity::User>,
    #[description = "Season to view (defaults to current)"]
    #[autocomplete = "autocomplete_season"]
    season: Option<String>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    // --- DB scope ---
    struct HistoryDb {
        user_ids: Vec<u64>,
        periods: Vec<crate::db::gym::models::Period>,
        /// period index → user_id → (count, goal_met, type_counts_sorted)
        period_data: Vec<HashMap<u64, (i32, bool, bool, Vec<(String, i32)>)>>,
        season_name: String,
    }

    let db_result: Option<HistoryDb> = {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        let user_ids = queries::get_users(&conn, guild_id)?;
        if user_ids.is_empty() {
            return Err("No users in the tracker yet.".into());
        }

        let (periods, season_name) = if let Some(ref season_name_filter) = season {
            // Explicit season requested
            let all_seasons = queries::get_all_seasons(&conn, guild_id)?;
            match all_seasons.into_iter().find(|s| s.name.to_lowercase() == season_name_filter.to_lowercase()) {
                Some(s) => {
                    let name = s.name.clone();
                    (queries::get_all_completed_periods_in_season(&conn, guild_id, s.id)?, name)
                }
                None => return Err(format!("Season '{}' not found.", season_name_filter).into()),
            }
        } else {
            match queries::get_current_season(&conn, guild_id)? {
                Some(s) => {
                    let name = s.name.clone();
                    (queries::get_all_completed_periods_in_season(&conn, guild_id, s.id)?, name)
                }
                None => (queries::get_all_completed_periods(&conn, guild_id)?, "History".to_string()),
            }
        };

        if periods.is_empty() {
            None
        } else {
            let mut period_data = Vec::new();
            for period in &periods {
                let results = queries::get_period_results(&conn, period.id)?;
                let type_counts_map = queries::get_all_period_type_counts(&conn, period.id)?;
                let map: HashMap<u64, (i32, bool, bool, Vec<(String, i32)>)> = results
                    .into_iter()
                    .map(|(uid, c, g, loa_exempt)| {
                        let types = type_counts_map.get(&uid).cloned().unwrap_or_default();
                        (uid, (c, g, loa_exempt, types))
                    })
                    .collect();
                period_data.push(map);
            }
            Some(HistoryDb { user_ids, periods, period_data, season_name })
        }
    };

    let hdb = match db_result {
        None => {
            ctx.send(poise::CreateReply::default()
                .content("No completed weeks in this season yet. Use `/gym force_rollover` or wait for the weekly rollover.")
                .ephemeral(true)
            ).await?;
            return Ok(());
        }
        Some(d) => d,
    };

    // All validation passed — defer before async Discord calls + image generation
    ctx.defer().await?;
    let guild = ctx.guild_id().ok_or("Must be used in a guild")?;

    // Build week label list once (used by both paths)
    let week_labels: Vec<String> = hdb.periods.iter().map(|p| period_label(&p.start_time)).collect();

    if let Some(target_user) = user {
        // --- Single-user full breakdown (image table) ---
        let target_id = target_user.id.get();
        if !hdb.user_ids.contains(&target_id) {
            return Err(format!("{} is not in the gym tracker.", target_user.name).into());
        }

        let display_name = match guild.member(ctx.http(), target_user.id).await {
            Ok(m) => m.display_name().to_string(),
            Err(_) => target_user.name.clone(),
        };

        // Fetch goal config + all goal changes for this user
        let (goal_summary, goal_changes): (String, Vec<(String, String)>) = {
            let db = &ctx.data().db;
            let conn = db.conn();

            let summary = if let Ok(Some(gc)) = queries::get_user_goal_config(&conn, guild_id, target_id) {
                let mut parts = vec![format!("{}/week", gc.total_goal)];
                if let Ok(type_goals) = {
                    let mut stmt = conn.prepare(
                        "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ? ORDER BY activity_type"
                    ).unwrap();
                    stmt.query_map(rusqlite::params![guild_id, target_id], |row| Ok((row.get::<_,String>(0)?, row.get::<_,i32>(1)?)))
                        .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
                } {
                    for (t, g) in &type_goals {
                        parts.push(format!("{} ≥ {}", t, g));
                    }
                }
                if let Ok(group_goals) = queries::get_user_group_goals(&conn, guild_id, target_id) {
                    for (g, c) in &group_goals {
                        parts.push(format!("{} group ≥ {}", g, c));
                    }
                }
                if parts.len() == 1 {
                    format!("Goal: {}", parts[0])
                } else {
                    format!("Goal: {}  +  {}", parts[0], parts[1..].join("  •  "))
                }
            } else {
                String::new()
            };

            let changes = queries::get_all_goal_changes(&conn, guild_id, target_id).unwrap_or_default();
            (summary, changes)
        };

        let mut total_count = 0i32;
        let mut total_met = 0i32;
        let mut total_missed = 0i32;

        // Build interleaved entries: week rows + goal change rows
        let mut entries: Vec<crate::images::gym::history::UserHistoryEntry> = Vec::new();
        let mut gc_idx = 0usize;

        for (i, period) in hdb.periods.iter().enumerate() {
            // Goal changes that happened BEFORE this period started (show before first period,
            // or between the previous period end and this period start)
            let window_start = if i == 0 { "0000-00-00".to_string() } else { hdb.periods[i - 1].end_time.clone() };
            while gc_idx < goal_changes.len() && goal_changes[gc_idx].0 < period.start_time
                && goal_changes[gc_idx].0 >= window_start
            {
                entries.push(crate::images::gym::history::UserHistoryEntry::GoalChange {
                    description: goal_changes[gc_idx].1.clone(),
                });
                gc_idx += 1;
            }

            let week_label = format!("{} – {}", period_label(&period.start_time), period_label(&period.end_time));
            let result = hdb.period_data[i].get(&target_id).cloned();
            if let Some((count, goal_met, loa_exempt, _)) = &result {
                total_count += count;
                if !loa_exempt {
                    if *goal_met { total_met += 1; } else { total_missed += 1; }
                }
            }
            entries.push(crate::images::gym::history::UserHistoryEntry::Week { week_label, result });

            // Goal changes that happened DURING this period
            while gc_idx < goal_changes.len() && goal_changes[gc_idx].0 <= period.end_time {
                entries.push(crate::images::gym::history::UserHistoryEntry::GoalChange {
                    description: goal_changes[gc_idx].1.clone(),
                });
                gc_idx += 1;
            }
        }
        // Any remaining goal changes after the last period
        while gc_idx < goal_changes.len() {
            entries.push(crate::images::gym::history::UserHistoryEntry::GoalChange {
                description: goal_changes[gc_idx].1.clone(),
            });
            gc_idx += 1;
        }

        let image_data = crate::images::gym::history::generate_user_history_image(
            &display_name,
            &hdb.season_name,
            &goal_summary,
            &entries,
            total_count,
            total_met,
            total_missed,
        )?;

        tracing::info!("guild={} user={} cmd=history target={} weeks={}", guild_id, ctx.author().id.get(), display_name, hdb.periods.len());
        let attachment = serenity::CreateAttachment::bytes(image_data, "user_history.png");
        ctx.send(poise::CreateReply::default().attachment(attachment)).await?;
    } else {
        // --- Full overview heatmap ---
        let mut history_rows = Vec::new();
        for user_id in &hdb.user_ids {
            let name = match guild.member(ctx.http(), serenity::UserId::new(*user_id)).await {
                Ok(member) => member.display_name().to_string(),
                Err(_) => format!("User {}", user_id),
            };
            let weeks: Vec<Option<(i32, bool, bool, Vec<(String, i32)>)>> = hdb.period_data.iter()
                .map(|period_map| period_map.get(user_id).cloned())
                .collect();
            history_rows.push(crate::images::gym::history::HistoryRow { name, weeks });
        }

        // Sort rows by total count across all periods (desc)
        history_rows.sort_by(|a, b| {
            let sum_a: i32 = a.weeks.iter().filter_map(|w| w.as_ref().map(|(c, _, _, _)| *c)).sum();
            let sum_b: i32 = b.weeks.iter().filter_map(|w| w.as_ref().map(|(c, _, _, _)| *c)).sum();
            sum_b.cmp(&sum_a)
        });

        let image_data = crate::images::gym::history::generate_history_image(&history_rows, &week_labels)?;
        let attachment = serenity::CreateAttachment::bytes(image_data, "history.png");
        ctx.send(poise::CreateReply::default().attachment(attachment)).await?;
        tracing::info!("guild={} history overview sent ({} users, {} weeks)", guild_id, history_rows.len(), week_labels.len());
    }

    Ok(())
}

async fn autocomplete_season<'a>(ctx: Context<'a>, partial: &'a str) -> Vec<String> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.get(),
        None => return vec![],
    };
    let db = &ctx.data().db;
    let conn = db.conn();
    queries::get_all_seasons(&conn, guild_id)
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.name)
        .filter(|n| n.to_lowercase().contains(&partial.to_lowercase()))
        .take(25)
        .collect()
}

/// Format a period start/end timestamp into a short "Jan 1" label.
fn period_label(dt_str: &str) -> String {
    if dt_str.len() < 10 { return dt_str.to_string(); }
    let month = &dt_str[5..7];
    let day_str = &dt_str[8..10];
    let month_name = match month {
        "01" => "Jan", "02" => "Feb", "03" => "Mar", "04" => "Apr",
        "05" => "May", "06" => "Jun", "07" => "Jul", "08" => "Aug",
        "09" => "Sep", "10" => "Oct", "11" => "Nov", "12" => "Dec",
        _ => month,
    };
    let day_num: u32 = day_str.parse().unwrap_or(0);
    format!("{} {}", month_name, day_num)
}
