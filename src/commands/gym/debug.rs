use super::Context;
use crate::db::gym::queries;
use crate::tasks::gym::weekly_check::rollover_period;
use crate::Error;
use poise::serenity_prelude as serenity;

/// Force re-registration of slash commands to this guild (dev only)
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "register")]
pub async fn force_register(ctx: Context<'_>) -> Result<(), Error> {
    if std::env::var("ENVIRONMENT").as_deref() != Ok("development") {
        return Err("This command is only available in development.".into());
    }
    let guild_id = serenity::GuildId::new(ctx.guild_id().ok_or("Must be used in a guild")?.get());
    poise::builtins::register_in_guild(
        ctx.serenity_context(),
        &ctx.framework().options().commands,
        guild_id,
    ).await?;
    ctx.say("Commands re-registered.").await?;
    Ok(())
}

/// Force a weekly rollover (dev only)
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn force_rollover(ctx: Context<'_>) -> Result<(), Error> {
    if std::env::var("ENVIRONMENT").as_deref() != Ok("development") {
        return Err("This command is only available in development.".into());
    }

    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let (guild_config, period) = {
        let db = &ctx.data().db;
        let conn = db.conn();

        let config = queries::get_guild_config(&conn, guild_id)?
            .ok_or("Gym tracker not set up.")?;

        if !config.started {
            return Err("Tracking hasn't started yet.".into());
        }

        let period = queries::get_current_period(&conn, guild_id)?
            .ok_or("No active period.")?;

        (config, period)
    };

    let http = ctx.serenity_context().http.clone();
    rollover_period(&http, ctx.data(), &guild_config, &period, None, None).await?;

    tracing::info!("guild={} user={} cmd=force_rollover period_id={}", guild_id, ctx.author().id.get(), period.id);
    ctx.say("Rollover complete! Check the configured channel for the summary.").await?;
    Ok(())
}
