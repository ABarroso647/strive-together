use super::Context;
use crate::db::gym::queries;
use crate::util::time::{format_datetime, get_weekly_period_bounds_with_hour, parse_datetime};
use crate::Error;
use chrono::Utc;
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

            tracing::info!("guild={} user={} cmd=setup channel={}", guild_id, ctx.author().id.get(), channel_id);
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
            // Create first period using this guild's rollover hour
            let (start, end) = get_weekly_period_bounds_with_hour(config.rollover_hour);
            let start_str = format_datetime(&start);
            let end_str = format_datetime(&end);

            let period_id = queries::insert_period(&conn, guild_id, &start_str, &end_str)?;
            queries::update_guild_started(&conn, guild_id, true)?;

            // Auto-create Szn 1 and tag the first period with it
            let season_id = queries::insert_season(&conn, guild_id, "Szn 1", &start_str)?;
            queries::set_period_season(&conn, period_id, season_id)?;

            tracing::info!("guild={} user={} cmd=start period_id={} season_id={}", guild_id, ctx.author().id.get(), period_id, season_id);
            format!(
                "Tracking started! **Szn 1** has begun.\n\
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
            tracing::info!("guild={} user={} cmd=stop", guild_id, ctx.author().id.get());
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
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", subcommands("config_goal", "config_rollover"))]
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

    tracing::info!("guild={} user={} cmd=config_goal amount={}", guild_id, ctx.author().id.get(), amount);
    ctx.say(format!("Default goal set to **{}** workouts per week.", amount)).await?;
    Ok(())
}

/// Set the hour (UTC) on Sunday when the weekly rollover fires
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "rollover")]
pub async fn config_rollover(
    ctx: Context<'_>,
    #[description = "Hour of day in UTC (0–23) on Sunday when the week ends"]
    #[min = 0_i32]
    #[max = 23_i32]
    hour: i32,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    {
        let db = &ctx.data().db;
        let conn = db.conn();
        queries::get_guild_config(&conn, guild_id)?.ok_or("Gym tracker not set up.")?;
        queries::update_rollover_hour(&conn, guild_id, hour as u32)?;
    }

    tracing::info!("guild={} user={} cmd=config_rollover hour={}", guild_id, ctx.author().id.get(), hour);
    ctx.say(format!(
        "Rollover time set to **Sunday {:02}:00 UTC**. Takes effect on the next period created.",
        hour
    )).await?;
    Ok(())
}

/// Show current period dates and time until rollover
#[poise::command(slash_command, guild_only)]
pub async fn period_info(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let period = {
        let db = &ctx.data().db;
        let conn = db.conn();
        queries::get_guild_config(&conn, guild_id)?.ok_or("Gym tracker not set up.")?;
        queries::get_current_period(&conn, guild_id)?.ok_or("No active period. Run `/gym start` first.")?
    };

    let end_time = parse_datetime(&period.end_time)?;
    let now = Utc::now();
    let diff = end_time - now;

    let time_str = if diff.num_seconds() <= 0 {
        "**Overdue** — rollover will fire on next hourly check".to_string()
    } else {
        let hours = diff.num_hours();
        let minutes = diff.num_minutes() % 60;
        format!("**{}h {}m** remaining", hours, minutes)
    };

    ctx.say(format!(
        "**Current Period**\nStart: `{}`\nEnd: `{}`\nTime until rollover: {}",
        &period.start_time, &period.end_time, time_str
    )).await?;
    Ok(())
}

/// Change when the current period ends (admin only)
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "set_period_end")]
pub async fn set_period_end(
    ctx: Context<'_>,
    #[description = "New end time in RFC3339 format (e.g. 2024-01-08T00:00:00+00:00), or 'now' to end immediately"]
    end_time: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let resolved_time = if end_time.trim().eq_ignore_ascii_case("now") {
        Utc::now().to_rfc3339()
    } else {
        parse_datetime(&end_time)
            .map_err(|_| "Invalid format. Use RFC3339 (e.g. 2024-01-08T00:00:00+00:00) or 'now'")?;
        end_time.clone()
    };

    {
        let db = &ctx.data().db;
        let conn = db.conn();
        queries::get_guild_config(&conn, guild_id)?.ok_or("Gym tracker not set up.")?;
        conn.execute(
            "UPDATE gym_periods SET end_time = ? WHERE guild_id = ? AND is_current = 1",
            rusqlite::params![resolved_time, guild_id],
        )?;
    }

    tracing::info!("guild={} user={} cmd=set_period_end end_time={}", guild_id, ctx.author().id.get(), resolved_time);
    ctx.say(format!(
        "Period end time updated to `{}`.",
        resolved_time
    )).await?;
    Ok(())
}
