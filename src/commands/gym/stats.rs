use super::Context;
use crate::db::gym::queries;
use crate::images::gym::summary::{generate_summary_image, UserSummary};
use crate::images::gym::totals::{generate_totals_image, UserTotals as ImageUserTotals};
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
        let (goal_met, goal_progress) = if goal_config.goal_mode == crate::db::gym::models::GoalMode::Total {
            let met = total_count >= goal_config.total_goal;
            let progress = format!("{}/{}", total_count, goal_config.total_goal);
            (met, progress)
        } else {
            // For by_type mode, check each type goal
            let mut stmt = conn.prepare(
                "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ?"
            )?;
            let type_goals: HashMap<String, i32> = stmt
                .query_map(rusqlite::params![guild_id, user_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut all_met = true;
            let mut progress_parts = Vec::new();
            for (activity_type, goal) in &type_goals {
                let count = type_counts.get(activity_type).copied().unwrap_or(0);
                if count < *goal {
                    all_met = false;
                }
                progress_parts.push(format!("{}: {}/{}", activity_type, count, goal));
            }

            (all_met, if progress_parts.is_empty() {
                format!("Total: {}", total_count)
            } else {
                progress_parts.join(", ")
            })
        };

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

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Show the current week summary image
#[poise::command(slash_command, guild_only)]
pub async fn summary(ctx: Context<'_>) -> Result<(), Error> {
    // Defer to give us time to generate the image
    ctx.defer().await?;

    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    // Gather data within DB scope
    let (period_str, users_data, activity_types) = {
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

        // Get current period
        let period = match queries::get_current_period(&conn, guild_id)? {
            Some(p) => p,
            None => return Err("No active period.".into()),
        };

        let period_str = format!("{} to {}", &period.start_time[..10], &period.end_time[..10]);

        // Get all users
        let user_ids = queries::get_users(&conn, guild_id)?;

        // Get activity types
        let activity_types = queries::get_activity_types(&conn, guild_id)?;

        // Build summary for each user
        let mut users_data: Vec<(u64, i32, i32, bool, Vec<(String, i32)>)> = Vec::new();
        for user_id in user_ids {
            let total = queries::get_user_period_count(&conn, period.id, user_id)?;
            let type_counts = queries::get_user_period_type_counts(&conn, period.id, user_id)?;
            let goal_config = queries::get_user_goal_config(&conn, guild_id, user_id)?;
            let goal = goal_config.map(|g| g.total_goal).unwrap_or(5);
            let goal_met = total >= goal;
            let type_vec: Vec<_> = type_counts.into_iter().collect();
            users_data.push((user_id, total, goal, goal_met, type_vec));
        }

        (period_str, users_data, activity_types)
    };

    // Fetch user names from Discord (outside DB scope)
    let guild = ctx.guild_id().ok_or("Must be used in a guild")?;
    let mut user_summaries = Vec::new();

    for (user_id, total, goal, goal_met, type_counts) in users_data {
        let name = match guild.member(ctx.http(), serenity::UserId::new(user_id)).await {
            Ok(member) => member.display_name().to_string(),
            Err(_) => format!("User {}", user_id),
        };
        user_summaries.push(UserSummary {
            name,
            total,
            goal,
            goal_met,
            type_counts,
        });
    }

    // Sort by total descending
    user_summaries.sort_by(|a, b| b.total.cmp(&a.total));

    // Generate image
    let image_data = generate_summary_image(
        "Weekly Summary",
        &period_str,
        &user_summaries,
        &activity_types,
    )?;

    // Send as attachment
    let attachment = serenity::CreateAttachment::bytes(image_data, "summary.png");
    ctx.send(poise::CreateReply::default().attachment(attachment)).await?;

    Ok(())
}

/// Show the all-time totals leaderboard
#[poise::command(slash_command, guild_only)]
pub async fn totals(ctx: Context<'_>) -> Result<(), Error> {
    // Defer to give us time to generate the image
    ctx.defer().await?;

    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    // Gather data within DB scope
    let (users_data, activity_types) = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        // Get all user totals
        let totals = queries::get_all_user_totals(&conn, guild_id)?;

        if totals.is_empty() {
            return Err("No users in the tracker yet.".into());
        }

        // Get activity types
        let activity_types = queries::get_activity_types(&conn, guild_id)?;

        // Get type totals for each user
        let mut users_data: Vec<(u64, i32, i32, i32, Vec<(String, i32)>)> = Vec::new();
        for user_total in totals {
            let type_totals = queries::get_user_type_totals(&conn, guild_id, user_total.user_id)?;
            let type_vec: Vec<_> = type_totals.into_iter().collect();
            users_data.push((
                user_total.user_id,
                user_total.total_count,
                user_total.achieved_goals,
                user_total.missed_goals,
                type_vec,
            ));
        }

        (users_data, activity_types)
    };

    // Fetch user names from Discord (outside DB scope)
    let guild = ctx.guild_id().ok_or("Must be used in a guild")?;
    let mut user_totals = Vec::new();

    for (i, (user_id, total_count, achieved_goals, missed_goals, type_totals)) in users_data.into_iter().enumerate() {
        let name = match guild.member(ctx.http(), serenity::UserId::new(user_id)).await {
            Ok(member) => member.display_name().to_string(),
            Err(_) => format!("User {}", user_id),
        };
        user_totals.push(ImageUserTotals {
            rank: i + 1,
            name,
            total_count,
            achieved_goals,
            missed_goals,
            type_totals,
        });
    }

    // Generate image
    let image_data = generate_totals_image(&user_totals, &activity_types)?;

    // Send as attachment
    let attachment = serenity::CreateAttachment::bytes(image_data, "totals.png");
    ctx.send(poise::CreateReply::default().attachment(attachment)).await?;

    Ok(())
}

/// Show week-by-week history
#[poise::command(slash_command, guild_only)]
pub async fn history(
    ctx: Context<'_>,
    #[description = "User to show history for (defaults to you)"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let target_user = user.as_ref().unwrap_or(ctx.author());
    let user_id = target_user.id.get();

    let embed = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        // Check if user exists
        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err(format!("{} is not in the gym tracker.", target_user.name).into());
        }

        // Get period results for this user
        let mut stmt = conn.prepare(
            "SELECT p.start_time, p.end_time, pr.total_count, pr.goal_met
             FROM gym_period_results pr
             JOIN gym_periods p ON pr.period_id = p.id
             WHERE p.guild_id = ? AND pr.user_id = ?
             ORDER BY p.start_time DESC
             LIMIT 10"
        )?;

        let results: Vec<(String, String, i32, bool)> = stmt
            .query_map(rusqlite::params![guild_id, user_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, i32>(3)? != 0,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        if results.is_empty() {
            serenity::CreateEmbed::new()
                .title(format!("📊 History for {}", target_user.name))
                .description("No completed weeks yet.")
                .color(0x888888)
        } else {
            let mut history_str = String::new();
            for (start, _end, count, met) in results {
                let status = if met { "✅" } else { "❌" };
                let week = &start[..10];
                history_str.push_str(&format!("{} Week of {}: **{}** workouts\n", status, week, count));
            }

            serenity::CreateEmbed::new()
                .title(format!("📊 History for {}", target_user.name))
                .description(history_str)
                .color(0x00aaff)
        }
    };

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}
