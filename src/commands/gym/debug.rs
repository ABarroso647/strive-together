use super::Context;
use crate::Error;
use crate::db::gym::queries;
use crate::tasks::gym::weekly_check::rollover_period;
use poise::serenity_prelude as serenity;

/// Force re-registration of slash commands to this guild (dev only)
#[poise::command(
    slash_command,
    guild_only,
    required_permissions = "ADMINISTRATOR",
    rename = "register"
)]
pub async fn force_register(ctx: Context<'_>) -> Result<(), Error> {
    if std::env::var("ENVIRONMENT").as_deref() != Ok("development") {
        return Err("This command is only available in development.".into());
    }
    let guild_id = serenity::GuildId::new(ctx.guild_id().ok_or("Must be used in a guild")?.get());
    poise::builtins::register_in_guild(
        ctx.serenity_context(),
        &ctx.framework().options().commands,
        guild_id,
    )
    .await?;
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

        let config =
            queries::get_guild_config(&conn, guild_id)?.ok_or("Gym tracker not set up.")?;

        if !config.started {
            return Err("Tracking hasn't started yet.".into());
        }

        let period = queries::get_current_period(&conn, guild_id)?.ok_or("No active period.")?;

        (config, period)
    };

    let http = ctx.serenity_context().http.clone();
    rollover_period(&http, ctx.data(), &guild_config, &period, None, None).await?;

    tracing::info!(
        "guild={} user={} cmd=force_rollover period_id={}",
        guild_id,
        ctx.author().id.get(),
        period.id
    );
    ctx.say("Rollover complete! Check the configured channel for the summary.")
        .await?;
    Ok(())
}

/// Re-check every user's goal for each completed week this season and fix wrong results
#[poise::command(
    slash_command,
    guild_only,
    required_permissions = "ADMINISTRATOR",
    rename = "re-evaluate"
)]
pub async fn re_evaluate(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let (week_count, fixed) = {
        let db = &ctx.data().db;
        let conn = db.conn();

        let season = queries::get_current_season(&conn, guild_id)?.ok_or("No active season.")?;
        let periods = queries::get_all_completed_periods_in_season(&conn, guild_id, season.id)?;

        let mut fixed = Vec::new();
        for period in &periods {
            for (user_id, _, goal_met, loa_exempt) in queries::get_period_results(&conn, period.id)?
            {
                if queries::reevaluate_period_goal(
                    &conn, guild_id, period.id, user_id, goal_met, loa_exempt,
                )? {
                    fixed.push(format!(
                        "<@{}> week of {} to {}: {} → {}",
                        user_id,
                        &period.start_time[..10],
                        &period.end_time[..10],
                        if goal_met { "✓" } else { "✗" },
                        if goal_met { "✗" } else { "✓" },
                    ));
                }
            }
        }
        (periods.len(), fixed)
    };

    tracing::info!(
        "guild={} user={} cmd=re_evaluate weeks={} fixed={}",
        guild_id,
        ctx.author().id.get(),
        week_count,
        fixed.len()
    );
    let response = if fixed.is_empty() {
        format!(
            "Re-evaluated {} week(s) this season — all goal results were correct.",
            week_count
        )
    } else {
        // Keep the reply under Discord's 2000-char limit
        const MAX_LINES: usize = 20;
        let mut lines = fixed.iter().take(MAX_LINES).cloned().collect::<Vec<_>>();
        if fixed.len() > MAX_LINES {
            lines.push(format!("…and {} more", fixed.len() - MAX_LINES));
        }
        format!(
            "Re-evaluated {} week(s) this season — fixed {} result(s):\n{}",
            week_count,
            fixed.len(),
            lines.join("\n")
        )
    };
    ctx.say(response).await?;
    Ok(())
}
