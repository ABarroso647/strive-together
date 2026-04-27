use super::Context;
use crate::db::gym::queries;
use crate::Error;

/// Set your weekly goal
#[poise::command(slash_command, guild_only, subcommands("goal_total", "goal_by_type", "goal_view"))]
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
    }

    ctx.say(format!(
        "Your weekly goal is now **{}** total workouts.\n\
        Your goal will be met when you log {} or more activities in a week.",
        count, count
    )).await?;
    Ok(())
}

/// Set a goal for a specific activity type
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

        // Set the type-specific goal (this also switches mode to by_type)
        queries::set_user_type_goal(&conn, guild_id, user_id, &activity_type, count)?;
    }

    ctx.say(format!(
        "Set goal for **{}** to **{}** per week.\n\
        Your goal mode is now **by_type** - you need to meet all type-specific goals.",
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

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        // Check if user is registered
        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err("You're not in the gym tracker. Ask an admin to add you.".into());
        }

        // Get user's goal config
        let goal_config = queries::get_user_goal_config(&conn, guild_id, user_id)?
            .ok_or("Could not find your goal configuration.")?;

        if goal_config.goal_mode == crate::db::gym::models::GoalMode::Total {
            format!(
                "**Your Goal Settings:**\n\
                Mode: **Total**\n\
                Weekly goal: **{}** workouts\n\n\
                Change with `/gym goal total <count>` or `/gym goal by_type <type> <count>`",
                goal_config.total_goal
            )
        } else {
            // Get type-specific goals
            let mut stmt = conn.prepare(
                "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ? ORDER BY activity_type"
            )?;
            let type_goals: Vec<(String, i32)> = stmt
                .query_map(rusqlite::params![guild_id, user_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let goals_str = if type_goals.is_empty() {
                "None set".to_string()
            } else {
                type_goals.iter()
                    .map(|(t, g)| format!("  {} → {}", t, g))
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            format!(
                "**Your Goal Settings:**\n\
                Mode: **By Type**\n\
                Type goals:\n{}\n\n\
                Add more with `/gym goal by_type <type> <count>`\n\
                Switch to total mode with `/gym goal total <count>`",
                goals_str
            )
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// Autocomplete function for activity types
async fn autocomplete_activity_type<'a>(
    ctx: Context<'a>,
    partial: &'a str,
) -> Vec<String> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.get(),
        None => return vec![],
    };

    let types = {
        let db = &ctx.data().db;
        let conn = db.conn();
        queries::get_activity_types(&conn, guild_id).unwrap_or_default()
    };

    types
        .into_iter()
        .filter(|t| t.to_lowercase().contains(&partial.to_lowercase()))
        .take(25)
        .collect()
}
