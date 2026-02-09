use super::Context;
use crate::db::gym::queries;
use crate::util::time::{format_datetime, get_weekly_period_bounds};
use crate::Error;
use poise::serenity_prelude as serenity;

/// Set up the gym tracker in this channel
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn setup(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let channel_id = ctx.channel_id().get();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if already set up
        if let Some(config) = queries::get_guild_config(&conn, guild_id)? {
            format!(
                "Gym tracker already set up in <#{}>.\nUse `/gym start` to begin tracking.",
                config.channel_id
            )
        } else {
            // Initialize guild config
            queries::insert_guild_config(&conn, guild_id, channel_id)?;

            // Add default activity types
            for activity_type in crate::db::gym::schema::DEFAULT_ACTIVITY_TYPES {
                queries::insert_activity_type(&conn, guild_id, activity_type)?;
            }

            format!(
                "Gym tracker set up in this channel!\n\
                Next steps:\n\
                1. Add users with `/gym add_user @user`\n\
                2. (Optional) Customize activity types with `/gym add_type` or `/gym remove_type`\n\
                3. Start tracking with `/gym start`"
            )
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// Start tracking (creates first period)
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn start(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if set up
        let config = match queries::get_guild_config(&conn, guild_id)? {
            Some(c) => c,
            None => return Err("Gym tracker not set up. Run `/gym setup` first.".into()),
        };

        if config.started {
            "Tracking is already active.".to_string()
        } else {
            // Create first period
            let (start, end) = get_weekly_period_bounds();
            let start_str = format_datetime(&start);
            let end_str = format_datetime(&end);

            queries::insert_period(&conn, guild_id, &start_str, &end_str)?;
            queries::update_guild_started(&conn, guild_id, true)?;

            format!(
                "Tracking started!\n\
                Current period: {} to {}\n\
                Users can now log workouts with `/gym log <type>`",
                &start_str[..10],
                &end_str[..10]
            )
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// Stop tracking
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if set up
        let config = match queries::get_guild_config(&conn, guild_id)? {
            Some(c) => c,
            None => return Err("Gym tracker not set up.".into()),
        };

        if !config.started {
            "Tracking is already stopped.".to_string()
        } else {
            queries::update_guild_started(&conn, guild_id, false)?;
            "Tracking stopped. Use `/gym start` to resume.".to_string()
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// Show current tracker configuration
#[poise::command(slash_command, guild_only)]
pub async fn info(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let embed = {
        let db = &ctx.data().db;
        let conn = db.conn();

        let config = match queries::get_guild_config(&conn, guild_id)? {
            Some(c) => c,
            None => return Err("Gym tracker not set up. Run `/gym setup` first.".into()),
        };

        let users = queries::get_users(&conn, guild_id)?;
        let types = queries::get_activity_types(&conn, guild_id)?;

        let period_info = if config.started {
            if let Some(period) = queries::get_current_period(&conn, guild_id)? {
                format!("{} to {}", &period.start_time[..10], &period.end_time[..10])
            } else {
                "No active period".to_string()
            }
        } else {
            "Tracking not started".to_string()
        };

        serenity::CreateEmbed::new()
            .title("Gym Tracker Configuration")
            .field("Channel", format!("<#{}>", config.channel_id), true)
            .field("Status", if config.started { "Active" } else { "Stopped" }, true)
            .field("Default Goal", config.default_goal.to_string(), true)
            .field("Current Period", period_info, false)
            .field("Users", format!("{} tracked", users.len()), true)
            .field("Activity Types", format!("{} configured", types.len()), true)
            .color(if config.started { 0x00ff00 } else { 0xff0000 })
    };

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Configure tracker settings
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", subcommands("config_goal"))]
pub async fn config(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Set the default weekly goal for new users
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "goal")]
pub async fn config_goal(
    ctx: Context<'_>,
    #[description = "Default weekly goal"]
    #[min = 1]
    #[max = 100]
    amount: i32,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        queries::update_default_goal(&conn, guild_id, amount)?;
    }

    ctx.say(format!("Default goal set to **{}** workouts per week.", amount)).await?;
    Ok(())
}
