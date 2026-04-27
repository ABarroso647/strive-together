use super::Context;
use crate::db::gym::queries;
use crate::tasks::gym::weekly_check::rollover_period;
use crate::util::time::format_datetime;
use crate::Error;
use chrono::Utc;

/// Season management
#[poise::command(slash_command, guild_only, subcommands("new", "end", "list"))]
pub async fn season(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Close the current season and start the next one; current week carries over.
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn new(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        let now = Utc::now();
        let now_str = format_datetime(&now);
        let season_count = queries::count_seasons(&conn, guild_id)?;

        if season_count == 0 {
            let szn1_id = queries::insert_season(&conn, guild_id, "Szn 1", &now_str)?;
            queries::tag_unassigned_periods(&conn, guild_id, szn1_id)?;
            queries::close_current_season(&conn, guild_id, &now_str)?;

            let szn2_id = queries::insert_season(&conn, guild_id, "Szn 2", &now_str)?;
            if let Some(period) = queries::get_current_period(&conn, guild_id)? {
                queries::set_period_season(&conn, period.id, szn2_id)?;
            }

            tracing::info!("guild={} user={} cmd=season_new szn1_id={} szn2_id={}", guild_id, ctx.author().id.get(), szn1_id, szn2_id);
            "All past history labeled **Szn 1**. **Szn 2** is now active — new weeks will count toward it.".to_string()
        } else {
            let next_num = season_count + 1;
            let next_name = format!("Szn {}", next_num);

            queries::close_current_season(&conn, guild_id, &now_str)?;
            let new_id = queries::insert_season(&conn, guild_id, &next_name, &now_str)?;

            if let Some(period) = queries::get_current_period(&conn, guild_id)? {
                queries::set_period_season(&conn, period.id, new_id)?;
            }

            tracing::info!("guild={} user={} cmd=season_new new_season={} id={}", guild_id, ctx.author().id.get(), next_name, new_id);
            format!("**Szn {}** has ended. **{}** is now active — the current week carries over.", season_count, next_name)
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// End the current season without starting a new one (off-season)
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn end(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let (guild_config, current_period, season_name) = {
        let db = &ctx.data().db;
        let conn = db.conn();

        let config = queries::get_guild_config(&conn, guild_id)?
            .ok_or("Gym tracker not set up.")?;

        let season = queries::get_current_season(&conn, guild_id)?;
        let name = match &season {
            None => return Err("No active season to end.".into()),
            Some(s) => s.name.clone(),
        };

        let period = if config.started {
            queries::get_current_period(&conn, guild_id)?
        } else {
            None
        };

        (config, period, name)
    };

    let now = Utc::now();
    let now_str = format_datetime(&now);

    // Roll over the current period immediately (old season still active → final season
    // stats image posts correctly). New period is left unassigned (off-season).
    if let Some(ref period) = current_period {
        ctx.defer().await?;
        rollover_period(
            &ctx.serenity_context().http,
            ctx.data(),
            &guild_config,
            period,
            Some(now),  // extra-long period from now → next Sunday
            Some(None), // off-season: no season assigned
        ).await?;
    }

    {
        let db = &ctx.data().db;
        let conn = db.conn();
        queries::close_current_season(&conn, guild_id, &now_str)?;
    }

    tracing::info!("guild={} user={} cmd=season_end season={}", guild_id, ctx.author().id.get(), season_name);
    ctx.say(format!(
        "**{}** has ended and its final week has been posted. Use `/gym season new` to start the next one.",
        season_name
    )).await?;
    Ok(())
}

/// List all seasons for this server
#[poise::command(slash_command, guild_only)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let seasons = {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        queries::get_all_seasons(&conn, guild_id)?
    };

    if seasons.is_empty() {
        ctx.say("No seasons yet. Use `/gym season new` to label your history and start a new season.").await?;
        return Ok(());
    }

    let mut lines = Vec::new();
    for s in &seasons {
        let start = &s.start_time[..10];
        let end = s.end_time.as_deref().map(|e| &e[..10]).unwrap_or("now");
        let status = if s.is_current { " *(current)*" } else { "" };
        lines.push(format!("• **{}** — {} to {}{}", s.name, start, end, status));
    }

    ctx.say(lines.join("\n")).await?;
    Ok(())
}
