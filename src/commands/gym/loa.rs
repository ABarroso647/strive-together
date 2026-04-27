use super::Context;
use crate::db::gym::queries;
use crate::util::time::format_datetime;
use crate::Error;
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use poise::serenity_prelude::{self as serenity, ChannelId, Role};

/// Leave of absence commands
#[poise::command(slash_command, guild_only, subcommands("loa_request"))]
pub async fn loa(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Request a leave of absence — server votes to approve or deny within 24 hours
#[poise::command(slash_command, guild_only, rename = "request")]
pub async fn loa_request(
    ctx: Context<'_>,
    #[description = "How many weeks (1–12)"]
    #[min = 1_i32]
    #[max = 12_i32]
    weeks: i32,
    #[description = "Start date (YYYY-MM-DD). Defaults to today if not set."]
    start_date: Option<String>,
    #[description = "Role to @mention in the vote post (e.g. @Gym Crew)"]
    mention_role: Option<Role>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = ctx.author().id.get();

    let channel_id = {
        let db = &ctx.data().db;
        let conn = db.conn();

        let users = queries::get_users(&conn, guild_id)?;
        if !users.contains(&user_id) {
            return Err("You're not tracked in this guild's gym tracker.".into());
        }

        let config = queries::get_guild_config(&conn, guild_id)?
            .ok_or("Gym tracker not set up.")?;

        if queries::get_pending_loa_for_user(&conn, guild_id, user_id)?.is_some() {
            return Err("You already have a pending LOA request.".into());
        }

        config.channel_id
    };

    // Resolve LOA window
    let now = Utc::now();
    let loa_start = match start_date {
        None => now,
        Some(ref s) => {
            let nd = NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| "Invalid start date — use YYYY-MM-DD format.")?;
            Utc.from_utc_datetime(&nd.and_hms_opt(0, 0, 0).unwrap())
        }
    };
    let loa_end = loa_start + Duration::weeks(weeks as i64);
    let loa_start_str = format_datetime(&loa_start);
    let loa_end_str = format_datetime(&loa_end);

    let vote_ends_at = now + Duration::hours(24);
    let vote_ends_str = format_datetime(&vote_ends_at);
    let vote_ends_unix = vote_ends_at.timestamp();

    let start_display = &loa_start_str[..10];
    let end_display = &loa_end_str[..10];

    let mention_str = mention_role
        .as_ref()
        .map(|r| format!("<@&{}>", r.id.get()))
        .unwrap_or_default();

    let vote_content = format!(
        "{}🏖️ **Leave of Absence Request**\n<@{}> is requesting a **{}-week** leave ({} → {}).\nThey can still log workouts, but missed-goal weeks won't count against them.\n\nReact ✅ to **approve** or ❌ to **deny** — voting closes <t:{}:R>.",
        if mention_str.is_empty() { String::new() } else { format!("{} ", mention_str) },
        user_id, weeks, start_display, end_display, vote_ends_unix
    );

    let channel = ChannelId::new(channel_id);
    let vote_msg = channel.send_message(
        ctx.serenity_context(),
        serenity::CreateMessage::new().content(vote_content),
    ).await?;

    // Seed reactions so members can click them (bot's own reactions are excluded at tally time)
    vote_msg.react(ctx.serenity_context(), serenity::ReactionType::Unicode("✅".to_string())).await?;
    vote_msg.react(ctx.serenity_context(), serenity::ReactionType::Unicode("❌".to_string())).await?;

    let loa_id = {
        let db = &ctx.data().db;
        let conn = db.conn();
        let id = queries::insert_loa_request(
            &conn, guild_id, user_id, &format_datetime(&now),
            weeks as i64, channel_id, &vote_ends_str,
            &loa_start_str, &loa_end_str,
        )?;
        queries::set_loa_vote_message(&conn, id, vote_msg.id.get())?;
        id
    };

    tracing::info!(
        "guild={} user={} cmd=loa_request weeks={} start={} end={} loa_id={}",
        guild_id, user_id, weeks, start_display, end_display, loa_id
    );

    ctx.say(format!(
        "Your LOA request has been posted! Voting closes <t:{}:R>.",
        vote_ends_unix
    )).await?;
    Ok(())
}
