use super::Context;
use crate::db::gym::queries;
use crate::util::time::format_datetime;
use crate::Error;
use chrono::Utc;

/// Set your weekly goal
#[poise::command(slash_command, guild_only, subcommands("goal_total", "goal_by_type", "goal_by_group", "goal_view", "goal_overview", "goal_reset"))]
pub async fn goal(_ctx: Context<'_>) -> Result<(), Error> {
    // Parent command - subcommands handle the actual work
    Ok(())
}

/// Set your total weekly goal
#[poise::command(slash_command, guild_only, rename = "total")]
pub async fn goal_total(
    ctx: Context<'_>,
    #[description = "Weekly goal count"]
    #[min = 1]
    #[max = 100]
    count: i32,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = ctx.author().id.get();

    {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        // Check if user is registered
        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err("You're not in the gym tracker. Ask an admin to add you.".into());
        }

        // Update goal
        queries::update_user_total_goal(&conn, guild_id, user_id, count)?;
        let now = format_datetime(&Utc::now());
        let _ = queries::record_goal_change(&conn, guild_id, user_id, &now, &format!("total goal → {}/week", count));
    }

    tracing::info!("guild={} user={} cmd=goal_total count={}", guild_id, user_id, count);
    ctx.say(format!(
        "Your weekly goal is now **{}** total workouts.\n\
        Your goal will be met when you log {} or more activities in a week.",
        count, count
    )).await?;
    Ok(())
}

/// Add a type-specific requirement on top of your total goal
#[poise::command(slash_command, guild_only, rename = "by_type")]
pub async fn goal_by_type(
    ctx: Context<'_>,
    #[description = "Activity type"]
    #[autocomplete = "autocomplete_activity_type"]
    activity_type: String,
    #[description = "Goal count for this type"]
    #[min = 1]
    #[max = 50]
    count: i32,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = ctx.author().id.get();
    let activity_type = activity_type.trim().to_lowercase();

    {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        // Check if user is registered
        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err("You're not in the gym tracker. Ask an admin to add you.".into());
        }

        // Check if activity type exists
        if !queries::activity_type_exists(&conn, guild_id, &activity_type)? {
            return Err(format!("Activity type '{}' doesn't exist.", activity_type).into());
        }

        queries::set_user_type_goal(&conn, guild_id, user_id, &activity_type, count)?;
        let now = format_datetime(&Utc::now());
        let _ = queries::record_goal_change(&conn, guild_id, user_id, &now, &format!("added: {} ≥ {}/week", activity_type, count));
    }

    tracing::info!("guild={} user={} cmd=goal_by_type type={} count={}", guild_id, user_id, activity_type, count);
    ctx.say(format!(
        "Added type requirement: **{}** ≥ **{}/week**.\n\
        This is an extra constraint — your total goal still applies too.",
        activity_type, count
    )).await?;
    Ok(())
}

/// View your current goal settings
#[poise::command(slash_command, guild_only, rename = "view")]
pub async fn goal_view(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = ctx.author().id.get();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err("You're not in the gym tracker. Ask an admin to add you.".into());
        }

        let goal_config = queries::get_user_goal_config(&conn, guild_id, user_id)?
            .ok_or("Could not find your goal configuration.")?;

        let mut lines = vec![
            "**Your Goal Settings:**".to_string(),
            format!("Total: **{} workouts/week** (always required)", goal_config.total_goal),
        ];

        let mut stmt = conn.prepare(
            "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ? ORDER BY activity_type"
        )?;
        let type_goals: Vec<(String, i32)> = stmt
            .query_map(rusqlite::params![guild_id, user_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        if !type_goals.is_empty() {
            lines.push("Type requirements (AND):".to_string());
            for (t, g) in &type_goals {
                lines.push(format!("  {} ≥ {}", t, g));
            }
        }

        let group_goals = queries::get_user_group_goals(&conn, guild_id, user_id)?;
        if !group_goals.is_empty() {
            lines.push("Group requirements (AND):".to_string());
            for (g, c) in &group_goals {
                lines.push(format!("  {} ≥ {}", g, c));
            }
        }

        lines.push(String::new());
        lines.push("Adjust with `/gym goal total`, `/gym goal by_type`, `/gym goal by_group`, or `/gym goal reset`.".to_string());

        lines.join("\n")
    };

    ctx.say(response).await?;
    Ok(())
}

/// Show goal settings for all tracked users
#[poise::command(slash_command, guild_only, rename = "overview")]
pub async fn goal_overview(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        let config = queries::get_guild_config(&conn, guild_id)?.ok_or("Gym tracker not set up.")?;
        let user_ids = queries::get_users(&conn, guild_id)?;

        if user_ids.is_empty() {
            return Err("No users in the tracker yet.".into());
        }

        let mut lines = vec![format!("**Goal Overview** (server default: {})", config.default_goal)];

        for uid in &user_ids {
            let goal_config = match queries::get_user_goal_config(&conn, guild_id, *uid)? {
                Some(gc) => gc,
                None => continue,
            };

            let on_default = goal_config.total_goal == config.default_goal;

            let mut parts: Vec<String> = Vec::new();

            // Total goal — mark if on default
            if on_default {
                parts.push(format!("{}/week", goal_config.total_goal));
            } else {
                parts.push(format!("**{}/week**", goal_config.total_goal));
            }

            // Type requirements
            let mut stmt = conn.prepare(
                "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ? ORDER BY activity_type"
            )?;
            let type_goals: Vec<(String, i32)> = stmt
                .query_map(rusqlite::params![guild_id, uid], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            for (t, g) in &type_goals {
                parts.push(format!("{} ≥ {}", t, g));
            }

            // Group requirements
            let group_goals = queries::get_user_group_goals(&conn, guild_id, *uid)?;
            for (g, c) in &group_goals {
                parts.push(format!("{} ≥ {}", g, c));
            }

            lines.push(format!("<@{}> — {}", uid, parts.join(", ")));
        }

        lines.join("\n")
    };

    ctx.say(response).await?;
    Ok(())
}

/// Add a group-based requirement on top of your total goal
#[poise::command(slash_command, guild_only, rename = "by_group")]
pub async fn goal_by_group(
    ctx: Context<'_>,
    #[description = "Activity group"]
    #[autocomplete = "autocomplete_group"]
    group: String,
    #[description = "Goal count for this group"]
    #[min = 1]
    #[max = 50]
    count: i32,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = ctx.author().id.get();
    let group = group.trim().to_lowercase();

    {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err("You're not in the gym tracker. Ask an admin to add you.".into());
        }

        if !queries::group_exists(&conn, guild_id, &group)? {
            let groups = queries::get_activity_groups(&conn, guild_id)?;
            if groups.is_empty() {
                return Err("No groups exist yet. Ask an admin to create groups with `/gym group create`.".into());
            }
            return Err(format!(
                "Group '{}' doesn't exist.\nAvailable groups: {}",
                group,
                groups.join(", ")
            ).into());
        }

        queries::set_user_group_goal(&conn, guild_id, user_id, &group, count)?;
        let now = format_datetime(&Utc::now());
        let _ = queries::record_goal_change(&conn, guild_id, user_id, &now, &format!("added group: {} ≥ {}/week", group, count));
    }

    tracing::info!("guild={} user={} cmd=goal_by_group group={} count={}", guild_id, user_id, group, count);
    ctx.say(format!(
        "Added group requirement: **{}** ≥ **{}/week**.\n\
        This is an extra constraint — your total goal still applies too.",
        group, count
    )).await?;
    Ok(())
}

/// Reset your goal back to the server default
#[poise::command(slash_command, guild_only, rename = "reset")]
pub async fn goal_reset(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = ctx.author().id.get();

    let default_goal = {
        let db = &ctx.data().db;
        let conn = db.conn();
        let config = queries::get_guild_config(&conn, guild_id)?.ok_or("Gym tracker not set up.")?;
        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err("You're not in the gym tracker.".into());
        }
        // Reset to total mode with server default, clear any type/group goals
        queries::update_user_total_goal(&conn, guild_id, user_id, config.default_goal)?;
        conn.execute(
            "DELETE FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ?",
            rusqlite::params![guild_id, user_id],
        )?;
        conn.execute(
            "DELETE FROM gym_user_group_goals WHERE guild_id = ? AND user_id = ?",
            rusqlite::params![guild_id, user_id],
        )?;
        let now = format_datetime(&Utc::now());
        let _ = queries::record_goal_change(&conn, guild_id, user_id, &now, &format!("reset to default: {}/week (all extra requirements cleared)", config.default_goal));
        config.default_goal
    };

    tracing::info!("guild={} user={} cmd=goal_reset default={}", guild_id, user_id, default_goal);
    ctx.say(format!("Your goal has been reset to the server default: **{}** workouts/week. All type and group requirements cleared.", default_goal)).await?;
    Ok(())
}

/// Autocomplete function for activity types
async fn autocomplete_activity_type<'a>(ctx: Context<'a>, partial: &'a str) -> Vec<String> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.get(),
        None => return vec![],
    };
    let db = &ctx.data().db;
    let conn = db.conn();
    queries::get_activity_types(&conn, guild_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|t| t.to_lowercase().contains(&partial.to_lowercase()))
        .take(25)
        .collect()
}

/// Autocomplete function for group names
async fn autocomplete_group<'a>(ctx: Context<'a>, partial: &'a str) -> Vec<String> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.get(),
        None => return vec![],
    };
    let db = &ctx.data().db;
    let conn = db.conn();
    queries::get_activity_groups(&conn, guild_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|g| g.to_lowercase().contains(&partial.to_lowercase()))
        .take(25)
        .collect()
}
