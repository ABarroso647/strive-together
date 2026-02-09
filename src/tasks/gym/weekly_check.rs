// Gym tracker weekly check background task
use chrono::Utc;
use poise::serenity_prelude::{self as serenity, ChannelId, Http};
use std::sync::Arc;
use tokio::time::{interval, Duration};

use crate::db::gym::queries;
use crate::images::gym::summary::{generate_summary_image, UserSummary};
use crate::util::time::{get_weekly_period_bounds, parse_datetime};
use crate::Data;

/// Start the background task that checks for gym period rollovers
pub fn start_weekly_check_task(http: Arc<Http>, data: Arc<Data>) {
    tokio::spawn(async move {
        // Wait a bit before first check (let bot fully initialize)
        tokio::time::sleep(Duration::from_secs(30)).await;

        // Run every hour
        let mut interval = interval(Duration::from_secs(60 * 60));

        loop {
            interval.tick().await;
            if let Err(e) = check_and_rollover_periods(&http, &data).await {
                tracing::error!("Error in gym weekly check task: {}", e);
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

    // Get all started guilds and their periods
    let guilds_to_process: Vec<_> = {
        let conn = data.db.conn();

        let guilds = queries::get_started_guilds(&conn)?;
        let mut to_process = Vec::new();

        for guild in guilds {
            if let Some(period) = queries::get_current_period(&conn, guild.guild_id)? {
                // Check if period has ended
                if let Ok(end_time) = parse_datetime(&period.end_time) {
                    if now >= end_time {
                        to_process.push((guild, period));
                    }
                }
            }
        }

        to_process
    };

    // Process each guild that needs rollover
    for (guild_config, period) in guilds_to_process {
        tracing::info!(
            "Rolling over gym period for guild {} (period {})",
            guild_config.guild_id,
            period.id
        );

        if let Err(e) = rollover_period(http, data, &guild_config, &period).await {
            tracing::error!(
                "Error rolling over gym period for guild {}: {}",
                guild_config.guild_id,
                e
            );
        }
    }

    Ok(())
}

async fn rollover_period(
    http: &Http,
    data: &Data,
    guild_config: &crate::db::gym::models::GuildConfig,
    period: &crate::db::gym::models::Period,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let guild_id = guild_config.guild_id;

    // Collect all data we need for the summary
    let (users_data, activity_types, period_str) = {
        let conn = data.db.conn();

        let user_ids = queries::get_users(&conn, guild_id)?;
        let activity_types = queries::get_activity_types(&conn, guild_id)?;
        let period_str = format!(
            "{} to {}",
            &period.start_time[..10],
            &period.end_time[..10]
        );

        let mut users_data = Vec::new();
        for user_id in user_ids {
            let total = queries::get_user_period_count(&conn, period.id, user_id)?;
            let type_counts = queries::get_user_period_type_counts(&conn, period.id, user_id)?;
            let goal_config = queries::get_user_goal_config(&conn, guild_id, user_id)?;
            let goal = goal_config.as_ref().map(|g| g.total_goal).unwrap_or(5);

            // Determine if goal is met
            let goal_met = if let Some(ref gc) = goal_config {
                if gc.goal_mode == crate::db::gym::models::GoalMode::Total {
                    total >= gc.total_goal
                } else {
                    // For by_type, check all type goals
                    let mut stmt = conn.prepare(
                        "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ?"
                    )?;
                    let type_goals: Vec<(String, i32)> = stmt
                        .query_map(rusqlite::params![guild_id, user_id], |row| {
                            Ok((row.get(0)?, row.get(1)?))
                        })?
                        .filter_map(|r| r.ok())
                        .collect();

                    type_goals.iter().all(|(activity_type, goal)| {
                        type_counts.get(activity_type).copied().unwrap_or(0) >= *goal
                    })
                }
            } else {
                total >= 5 // default goal
            };

            users_data.push((user_id, total, goal, goal_met, type_counts));
        }

        (users_data, activity_types, period_str)
    };

    // Save period results and update totals
    {
        let conn = data.db.conn();

        for (user_id, total, _goal, goal_met, type_counts) in &users_data {
            // Save period result
            queries::insert_period_result(&conn, period.id, *user_id, *total, *goal_met)?;

            // Save type counts for this period
            for (activity_type, count) in type_counts {
                queries::insert_period_type_count(&conn, period.id, *user_id, activity_type, *count)?;
            }

            // Update user totals
            let achieved_delta = if *goal_met { 1 } else { 0 };
            let missed_delta = if *goal_met { 0 } else { 1 };
            queries::update_user_totals(&conn, guild_id, *user_id, *total, achieved_delta, missed_delta)?;

            // Update user type totals
            for (activity_type, count) in type_counts {
                queries::increment_user_type_total(&conn, guild_id, *user_id, activity_type, *count)?;
            }
        }

        // Close current period
        queries::close_current_period(&conn, guild_id)?;

        // Create new period
        let (start, end) = get_weekly_period_bounds();
        let start_str = crate::util::time::format_datetime(&start);
        let end_str = crate::util::time::format_datetime(&end);
        queries::insert_period(&conn, guild_id, &start_str, &end_str)?;
    }

    // Fetch user names and generate summary image
    let guild_id_snowflake = serenity::GuildId::new(guild_id);
    let mut user_summaries = Vec::new();

    for (user_id, total, goal, goal_met, type_counts) in users_data {
        let name = match guild_id_snowflake.member(http, serenity::UserId::new(user_id)).await {
            Ok(member) => member.display_name().to_string(),
            Err(_) => format!("User {}", user_id),
        };
        let type_vec: Vec<_> = type_counts.into_iter().collect();
        user_summaries.push(UserSummary {
            name,
            total,
            goal,
            goal_met,
            type_counts: type_vec,
        });
    }

    // Sort by total descending
    user_summaries.sort_by(|a, b| b.total.cmp(&a.total));

    // Generate summary image
    let image_data = generate_summary_image(
        "Weekly Summary - Period Complete",
        &period_str,
        &user_summaries,
        &activity_types,
    )?;

    // Send to configured channel
    let channel = ChannelId::new(guild_config.channel_id);
    let attachment = serenity::CreateAttachment::bytes(image_data, "weekly_summary.png");

    channel
        .send_message(
            http,
            serenity::CreateMessage::new()
                .content("📊 **Gym Tracker Weekly Summary** - The tracking period has ended!")
                .add_file(attachment),
        )
        .await?;

    tracing::info!(
        "Successfully rolled over gym period for guild {}",
        guild_config.guild_id
    );

    Ok(())
}
