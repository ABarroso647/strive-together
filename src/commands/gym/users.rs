use super::Context;
use crate::db::gym::queries;
use crate::Error;
use poise::serenity_prelude as serenity;

/// Manage tracked users
#[poise::command(slash_command, guild_only, subcommands("add_user", "remove_user", "list_users", "import_user", "set_type_total", "set_goal_stats"))]
pub async fn user(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Add a user to the gym tracker
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "add")]
pub async fn add_user(
    ctx: Context<'_>,
    #[description = "User to add"] user: serenity::User,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = user.id.get();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        let config = match queries::get_guild_config(&conn, guild_id)? {
            Some(c) => c,
            None => return Err("Gym tracker not set up. Run `/gym setup` first.".into()),
        };

        // Check if user already exists
        if queries::user_exists(&conn, guild_id, user_id)? {
            format!("<@{}> is already in the gym tracker.", user_id)
        } else {
            queries::insert_user(&conn, guild_id, user_id, config.default_goal)?;

            // Initialize type totals for all existing types
            let types = queries::get_activity_types(&conn, guild_id)?;
            for activity_type in types {
                queries::set_user_type_total(&conn, guild_id, user_id, &activity_type, 0)?;
            }

            tracing::info!("guild={} admin={} cmd=user_add target_user={}", guild_id, ctx.author().id.get(), user_id);
            format!(
                "Added <@{}> to the gym tracker with a goal of {} workouts/week.",
                user_id, config.default_goal
            )
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// Remove a user from the gym tracker
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "remove")]
pub async fn remove_user(
    ctx: Context<'_>,
    #[description = "User to remove"] user: serenity::User,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = user.id.get();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        if queries::delete_user(&conn, guild_id, user_id)? {
            tracing::info!("guild={} admin={} cmd=user_remove target_user={}", guild_id, ctx.author().id.get(), user_id);
            format!("Removed <@{}> from the gym tracker.", user_id)
        } else {
            format!("<@{}> was not in the gym tracker.", user_id)
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// List all users in the gym tracker
#[poise::command(slash_command, guild_only, rename = "list")]
pub async fn list_users(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let users = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        queries::get_users(&conn, guild_id)?
    };

    if users.is_empty() {
        ctx.say("No users in the gym tracker yet. Add users with `/gym add_user @user`.").await?;
    } else {
        let mentions: Vec<String> = users.iter().map(|id| format!("<@{}>", id)).collect();
        let embed = serenity::CreateEmbed::new()
            .title("Gym Tracker Users")
            .description(mentions.join("\n"))
            .field("Total", users.len().to_string(), true)
            .color(0x00aaff);

        ctx.send(poise::CreateReply::default().embed(embed)).await?;
    }

    Ok(())
}

/// Import user data from JSON
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "import")]
pub async fn import_user(
    ctx: Context<'_>,
    #[description = "User to import data for"] user: serenity::User,
    #[description = "JSON data"] json: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = user.id.get();

    // Parse JSON
    let data: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        let config = match queries::get_guild_config(&conn, guild_id)? {
            Some(c) => c,
            None => return Err("Gym tracker not set up.".into()),
        };

        // Ensure user exists (create if not)
        if !queries::user_exists(&conn, guild_id, user_id)? {
            queries::insert_user(&conn, guild_id, user_id, config.default_goal)?;
        }

        let mut imported = Vec::new();

        // Import type totals
        if let Some(type_totals) = data.get("type_totals").and_then(|v| v.as_object()) {
            for (activity_type, count) in type_totals {
                if let Some(count) = count.as_i64() {
                    queries::set_user_type_total(&conn, guild_id, user_id, activity_type, count as i32)?;
                    imported.push(format!("{}: {}", activity_type, count));
                }
            }
        }

        // Import goal stats
        if let (Some(achieved), Some(missed)) = (
            data.get("achieved_goals").and_then(|v| v.as_i64()),
            data.get("missed_goals").and_then(|v| v.as_i64()),
        ) {
            queries::set_user_goal_stats(&conn, guild_id, user_id, achieved as i32, missed as i32)?;
            imported.push(format!("Goals: {} achieved, {} missed", achieved, missed));
        }

        // Import total count
        if let Some(total) = data.get("total_count").and_then(|v| v.as_i64()) {
            // We need to update user_totals directly
            conn.execute(
                "UPDATE gym_user_totals SET total_count = ? WHERE guild_id = ? AND user_id = ?",
                rusqlite::params![total as i32, guild_id, user_id],
            )?;
            imported.push(format!("Total: {}", total));
        }

        if imported.is_empty() {
            "No valid data found in JSON.".to_string()
        } else {
            format!("Imported for {}:\n{}", user.name, imported.join("\n"))
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// Set a user's total for a specific activity type
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "set_type")]
pub async fn set_type_total(
    ctx: Context<'_>,
    #[description = "User"] user: serenity::User,
    #[description = "Activity type"]
    #[autocomplete = "autocomplete_activity_type"]
    activity_type: String,
    #[description = "Total count"]
    #[min = 0]
    count: i32,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = user.id.get();
    let activity_type = activity_type.trim().to_lowercase();

    {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Validate
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }
        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err(format!("{} is not in the gym tracker.", user.name).into());
        }
        if !queries::activity_type_exists(&conn, guild_id, &activity_type)? {
            return Err(format!("Activity type '{}' doesn't exist.", activity_type).into());
        }

        queries::set_user_type_total(&conn, guild_id, user_id, &activity_type, count)?;
    }

    tracing::info!("guild={} admin={} cmd=user_set_type target={} type={} count={}", guild_id, ctx.author().id.get(), user_id, activity_type, count);
    ctx.say(format!(
        "Set {}'s **{}** total to **{}**.",
        user.name, activity_type, count
    )).await?;
    Ok(())
}

/// Set a user's goal statistics
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "set_goals")]
pub async fn set_goal_stats(
    ctx: Context<'_>,
    #[description = "User"] user: serenity::User,
    #[description = "Goals achieved"]
    #[min = 0]
    achieved: i32,
    #[description = "Goals missed"]
    #[min = 0]
    missed: i32,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = user.id.get();

    {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }
        if !queries::user_exists(&conn, guild_id, user_id)? {
            return Err(format!("{} is not in the gym tracker.", user.name).into());
        }

        queries::set_user_goal_stats(&conn, guild_id, user_id, achieved, missed)?;
    }

    tracing::info!("guild={} admin={} cmd=user_set_goals target={} achieved={} missed={}", guild_id, ctx.author().id.get(), user_id, achieved, missed);
    ctx.say(format!(
        "Set {}'s goals to **{}** achieved, **{}** missed.",
        user.name, achieved, missed
    )).await?;
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
