// Gym tracker weekly check background task
use chrono::Utc;
use poise::serenity_prelude::{self as serenity, ChannelId, Http};
use rusqlite::Connection;
use std::sync::Arc;
use tokio::time::Duration;

use crate::db::gym::queries;
use crate::util::time::{get_period_end_time, get_period_start_time, parse_datetime};
use crate::Data;

/// Convert an RFC3339 datetime string to a short human-readable date like "Jan 6"
fn format_short_date(dt_str: &str) -> String {
    match parse_datetime(dt_str) {
        Ok(dt) => dt.format("%b %-d").to_string(),
        Err(_) => dt_str[..10].to_string(),
    }
}

/// Find the earliest current period end_time across all started guilds.
fn next_rollover_time(conn: &Connection) -> Option<chrono::DateTime<Utc>> {
    let result: rusqlite::Result<Option<String>> = conn.query_row(
        "SELECT MIN(p.end_time) FROM gym_periods p
         JOIN gym_guild_config c ON p.guild_id = c.guild_id
         WHERE c.started = 1 AND p.is_current = 1",
        [],
        |row| row.get(0),
    );
    result.ok().flatten().and_then(|s| parse_datetime(&s).ok())
}

/// Start the background task that handles gym period rollovers.
/// On each iteration it:
///   1. Processes any overdue rollovers immediately.
///   2. Reads the next period end_time from DB and sleeps until then.
/// On restart the DB already has the stored end_time so the sleep recalculates correctly.
pub fn start_weekly_check_task(http: Arc<Http>, data: Arc<Data>) {
    tokio::spawn(async move {
        // Brief startup delay
        tokio::time::sleep(Duration::from_secs(30)).await;

        loop {
            // Process any overdue rollovers first
            if let Err(e) = check_and_rollover_periods(&http, &data).await {
                tracing::error!("Error in gym weekly check task: {}", e);
            }

            // Read next rollover time from DB
            let next = {
                let conn = data.db.conn();
                next_rollover_time(&conn)
            };

            match next {
                None => {
                    // No active guilds — check again in an hour
                    tracing::debug!("No active gym guilds; sleeping 1h");
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                }
                Some(end_time) => {
                    let now = Utc::now();
                    if end_time > now {
                        let secs = (end_time - now).num_seconds().max(0) as u64;
                        tracing::info!(
                            "Next gym rollover at {} (in {}m {}s)",
                            end_time.format("%Y-%m-%d %H:%M UTC"),
                            secs / 60,
                            secs % 60,
                        );
                        // Sleep until 30 seconds after the rollover time as a small buffer
                        tokio::time::sleep(Duration::from_secs(secs + 30)).await;
                    }
                    // If overdue: loop immediately to process
                }
            }
        }
    });
}

async fn check_and_rollover_periods(
    http: &Http,
    data: &Data,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::debug!("Gym weekly check task running...");

    let now = Utc::now();

    let guilds_to_process: Vec<_> = {
        let conn = data.db.conn();
        let guilds = queries::get_started_guilds(&conn)?;
        let mut to_process = Vec::new();

        for guild in guilds {
            if let Some(period) = queries::get_current_period(&conn, guild.guild_id)? {
                if let Ok(end_time) = parse_datetime(&period.end_time) {
                    if now >= end_time {
                        to_process.push((guild, period));
                    }
                }
            }
        }
        to_process
    };

    for (guild_config, period) in guilds_to_process {
        tracing::info!(
            "Rolling over gym period for guild {} (period {})",
            guild_config.guild_id,
            period.id
        );
        if let Err(e) = rollover_period(http, data, &guild_config, &period, None, None).await {
            tracing::error!(
                "Error rolling over gym period for guild {}: {}",
                guild_config.guild_id,
                e
            );
        }
    }

    Ok(())
}

/// Roll over a period: archive results, post images, create the next period.
///
/// `new_period_start`: override the start of the next period (e.g. `Utc::now()` when
///   ending a season mid-week to get an extra-long next period). `None` uses the standard
///   7-days-before-end calculation.
///
/// `new_period_season_override`: `None` = auto-assign to current season (normal weekly
///   rollover), `Some(None)` = leave the new period unassigned (caller will assign it),
///   `Some(Some(id))` = assign to a specific season.
pub async fn rollover_period(
    http: &Http,
    data: &Data,
    guild_config: &crate::db::gym::models::GuildConfig,
    period: &crate::db::gym::models::Period,
    new_period_start: Option<chrono::DateTime<Utc>>,
    new_period_season_override: Option<Option<i64>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let guild_id = guild_config.guild_id;

    // Gather per-user results
    let users_data = {
        let conn = data.db.conn();
        let user_ids = queries::get_users(&conn, guild_id)?;
        let type_group_map = queries::get_all_type_groups(&conn, guild_id)?;
        let mut users_data = Vec::new();
        for user_id in user_ids {
            let total = queries::get_user_period_count(&conn, period.id, user_id)?;
            let type_counts = queries::get_user_period_type_counts(&conn, period.id, user_id)?;
            let goal_config = queries::get_user_goal_config(&conn, guild_id, user_id)?;
            let goal_met = crate::images::gym::summary::evaluate_goal_met(
                &conn, guild_id, user_id, total, &type_counts, &goal_config, &type_group_map,
            )?;
            users_data.push((user_id, total, goal_met, type_counts));
        }
        users_data
    };

    // Save results and create next period
    {
        let conn = data.db.conn();

        for (user_id, total, goal_met, type_counts) in &users_data {
            // Check if user has an active LOA covering this period
            let on_loa = queries::get_active_loa_for_user(
                &conn, guild_id, *user_id, &period.start_time, &period.end_time
            )?.is_some();

            queries::insert_period_result(&conn, period.id, *user_id, *total, if on_loa { false } else { *goal_met }, on_loa)?;
            for (activity_type, count) in type_counts {
                queries::insert_period_type_count(&conn, period.id, *user_id, activity_type, *count)?;
            }
            // LOA: count still accumulates, but goal stats are frozen
            let achieved_delta = if !on_loa && *goal_met { 1 } else { 0 };
            let missed_delta = if !on_loa && !*goal_met { 1 } else { 0 };
            queries::update_user_totals(&conn, guild_id, *user_id, *total, achieved_delta, missed_delta)?;
            for (activity_type, count) in type_counts {
                queries::increment_user_type_total(&conn, guild_id, *user_id, activity_type, *count)?;
            }
        }

        queries::close_current_period(&conn, guild_id)?;

        // Calculate next period bounds
        let end = get_period_end_time(guild_config.rollover_hour);
        let start = new_period_start.unwrap_or_else(|| get_period_start_time(&end));
        let start_str = crate::util::time::format_datetime(&start);
        let end_str = crate::util::time::format_datetime(&end);
        let new_period_id = queries::insert_period(&conn, guild_id, &start_str, &end_str)?;

        match new_period_season_override {
            None => {
                // Auto: assign to current season (normal weekly rollover)
                if let Some(season) = queries::get_current_season(&conn, guild_id)? {
                    queries::set_period_season(&conn, new_period_id, season.id)?;
                }
            }
            Some(None) => {
                // Deliberately unassigned — caller will assign after changing season state
            }
            Some(Some(season_id)) => {
                queries::set_period_season(&conn, new_period_id, season_id)?;
            }
        }
    }

    // Format headers
    let period_start = format_short_date(&period.start_time);
    let period_end = format_short_date(&period.end_time);
    let season_name = {
        let conn = data.db.conn();
        queries::get_current_season(&conn, guild_id)?
            .map(|s| s.name)
            .unwrap_or_else(|| "Season".to_string())
    };

    // Generate and post images
    let summary_data = crate::images::gym::summary::build_period_summary_png(
        &data.db,
        http,
        guild_id,
        period,
        "Weekly Summary - Period Complete",
    ).await?;

    let season_data = match crate::images::gym::season::build_season_stats_png(
        &data.db,
        http,
        guild_id,
    ).await {
        Ok(d) => Some(d),
        Err(e) => {
            tracing::error!("Failed to build season stats image for guild {}: {}", guild_id, e);
            None
        }
    };

    let channel = ChannelId::new(guild_config.channel_id);

    let summary_header = format!("**Weekly Summary — {} to {}**", period_start, period_end);
    channel.send_message(http, serenity::CreateMessage::new()
        .content(summary_header)
        .add_file(serenity::CreateAttachment::bytes(summary_data, "weekly_summary.png"))
    ).await?;

    if let Some(season) = season_data {
        let season_header = format!("**{} Stats — totals through {}**", season_name, period_end);
        channel.send_message(http, serenity::CreateMessage::new()
            .content(season_header)
            .add_file(serenity::CreateAttachment::bytes(season, "season_stats.png"))
        ).await?;
    }

    tracing::info!("Successfully rolled over gym period for guild {}", guild_config.guild_id);
    Ok(())
}
