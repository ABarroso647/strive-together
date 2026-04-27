use crate::db::gym::queries;
use crate::util::time::{format_datetime, parse_datetime};
use crate::{Data, Error};
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use poise::serenity_prelude::{self as serenity, ChannelId, Role};

type Context<'a> = poise::Context<'a, Data, Error>;

/// Leave of absence commands
#[poise::command(slash_command, guild_only, subcommands("loa_request", "loa_resolve"))]
pub async fn loa(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[derive(Debug, poise::Modal)]
#[name = "Leave of Absence Request"]
struct LoaModal {
    #[name = "Number of weeks (1–12)"]
    #[placeholder = "e.g. 2"]
    #[min_length = 1]
    #[max_length = 2]
    weeks: String,

    #[name = "Start date (optional)"]
    #[placeholder = "YYYY-MM-DD — leave blank for the current period"]
    start_date: Option<String>,
}

async fn followup_ephemeral(
    ctx: poise::ApplicationContext<'_, Data, Error>,
    content: &str,
) -> Result<(), Error> {
    ctx.interaction
        .create_followup(
            ctx.serenity_context,
            serenity::CreateInteractionResponseFollowup::new()
                .content(content)
                .ephemeral(true),
        )
        .await?;
    Ok(())
}

/// Request a leave of absence — server votes to approve or deny within 24 hours
#[poise::command(slash_command, guild_only, rename = "request")]
pub async fn loa_request(
    ctx: poise::ApplicationContext<'_, Data, Error>,
    #[description = "Role to @mention in the vote post (e.g. @Gym Crew)"]
    mention_role: Option<Role>,
) -> Result<(), Error> {
    let modal_data = match poise::execute_modal(ctx, None::<LoaModal>, None).await? {
        Some(data) => data,
        None => {
            followup_ephemeral(ctx, "LOA request cancelled.").await?;
            return Ok(());
        }
    };

    // --- Parse weeks ---
    let weeks: i32 = match modal_data.weeks.trim().parse() {
        Ok(n) if (1..=12).contains(&n) => n,
        _ => {
            followup_ephemeral(
                ctx,
                "Invalid number of weeks — must be a whole number between 1 and 12.",
            )
            .await?;
            return Ok(());
        }
    };

    let start_date_str = modal_data
        .start_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let guild_id = ctx.interaction.guild_id.ok_or("Must be used in a guild")?.get();
    let user_id = ctx.interaction.user.id.get();
    let mention_role_id = mention_role.as_ref().map(|r| r.id.get());

    // --- DB validation + fetch period (all before any await) ---
    struct DbResult {
        channel_id: u64,
        period_start: chrono::DateTime<Utc>,
    }

    enum DbOutcome {
        Ok(DbResult),
        NotTracked,
        NotSetUp,
        PendingLoa,
        NoPeriod,
        DbError(Box<dyn std::error::Error + Send + Sync>),
    }

    let db_outcome = {
        let conn = ctx.data.db.conn();
        let result = (|| -> Result<DbOutcome, Box<dyn std::error::Error + Send + Sync>> {
            let users = queries::get_users(&conn, guild_id)?;
            if !users.contains(&user_id) {
                return Ok(DbOutcome::NotTracked);
            }
            let config = match queries::get_guild_config(&conn, guild_id)? {
                Some(c) => c,
                None => return Ok(DbOutcome::NotSetUp),
            };
            if queries::get_pending_loa_for_user(&conn, guild_id, user_id)?.is_some() {
                return Ok(DbOutcome::PendingLoa);
            }
            let period = match queries::get_current_period(&conn, guild_id)? {
                Some(p) => p,
                None => return Ok(DbOutcome::NoPeriod),
            };
            let period_start = parse_datetime(&period.start_time)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(DbOutcome::Ok(DbResult { channel_id: config.channel_id, period_start }))
        })();
        match result {
            Ok(o) => o,
            Err(e) => DbOutcome::DbError(e),
        }
    };

    let db_result = match db_outcome {
        DbOutcome::Ok(r) => r,
        DbOutcome::NotTracked => {
            followup_ephemeral(ctx, "You're not tracked in this guild's gym tracker.").await?;
            return Ok(());
        }
        DbOutcome::NotSetUp => {
            followup_ephemeral(ctx, "Gym tracker not set up.").await?;
            return Ok(());
        }
        DbOutcome::PendingLoa => {
            followup_ephemeral(ctx, "You already have a pending LOA request.").await?;
            return Ok(());
        }
        DbOutcome::NoPeriod => {
            followup_ephemeral(ctx, "No active tracking period. Ask an admin to run `/gym start`.").await?;
            return Ok(());
        }
        DbOutcome::DbError(e) => return Err(e),
    };

    // --- Snap LOA window to period boundaries ---
    // Periods are exactly 7 days. LOA always starts at a period boundary and
    // covers exactly N complete periods (so rollover exemption is clean).
    let period_start = db_result.period_start;

    let loa_start = match start_date_str {
        None => period_start,
        Some(ref s) => {
            let nd = match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => {
                    followup_ephemeral(
                        ctx,
                        "Invalid start date — use YYYY-MM-DD format (e.g. 2025-06-01).",
                    )
                    .await?;
                    return Ok(());
                }
            };
            let requested = Utc.from_utc_datetime(&nd.and_hms_opt(0, 0, 0).unwrap());
            // Snap forward to the nearest period boundary on or containing the requested date
            // (floor division: requested within a period → that period's start)
            let days_diff = (requested - period_start).num_days().max(0);
            period_start + Duration::weeks(days_diff / 7)
        }
    };
    let loa_end = loa_start + Duration::weeks(weeks as i64);

    // Build covered-period list for the confirmation preview
    let covered: Vec<(i64, i64)> = (0..weeks)
        .map(|i| {
            let s = (loa_start + Duration::weeks(i as i64)).timestamp();
            let e = (loa_start + Duration::weeks(i as i64 + 1)).timestamp();
            (s, e)
        })
        .collect();

    let periods_preview = covered.iter()
        .enumerate()
        .map(|(i, (s, e))| format!("Period {}: <t:{}:D> → <t:{}:D>", i + 1, s, e))
        .collect::<Vec<_>>()
        .join("\n");

    let mention_str = mention_role
        .as_ref()
        .map(|r| format!("<@&{}>", r.id.get()))
        .unwrap_or_default();

    // --- Confirmation step ---
    let mut confirm_msg = ctx.interaction
        .create_followup(
            ctx.serenity_context,
            serenity::CreateInteractionResponseFollowup::new()
                .content(format!(
                    "**{}-week LOA** — this will pause your goal tracking for:\n{}\n{}Post vote to <#{}>?",
                    weeks,
                    periods_preview,
                    if mention_str.is_empty() { String::new() } else { format!("Notifying {}\n", mention_str) },
                    db_result.channel_id,
                ))
                .ephemeral(true)
                .components(vec![serenity::CreateActionRow::Buttons(vec![
                    serenity::CreateButton::new("loa_confirm")
                        .label("Post Vote")
                        .style(serenity::ButtonStyle::Success),
                    serenity::CreateButton::new("loa_cancel")
                        .label("Cancel")
                        .style(serenity::ButtonStyle::Secondary),
                ])]),
        )
        .await?;

    let requester_discord_id = ctx.interaction.user.id;
    let confirm_msg_id = confirm_msg.id;
    let clicked = serenity::ComponentInteractionCollector::new(ctx.serenity_context)
        .filter(move |i| i.message.id == confirm_msg_id && i.user.id == requester_discord_id)
        .timeout(std::time::Duration::from_secs(60))
        .await;

    let mci = match clicked {
        None => {
            // Timed out — remove buttons so it can't be clicked late
            confirm_msg.edit(
                ctx.serenity_context,
                serenity::EditMessage::new()
                    .content("LOA request timed out.")
                    .components(vec![]),
            ).await.ok();
            return Ok(());
        }
        Some(mci) => mci,
    };

    // Dismiss the confirmation message entirely
    mci.create_response(ctx.serenity_context, serenity::CreateInteractionResponse::Acknowledge).await?;
    ctx.interaction.delete_followup(ctx.serenity_context, confirm_msg.id).await.ok();

    if mci.data.custom_id == "loa_cancel" {
        return Ok(());
    }

    // --- Post vote message ---
    let now = Utc::now();
    let vote_ends_at = now + Duration::hours(24);
    let vote_ends_str = format_datetime(&vote_ends_at);
    let vote_ends_unix = vote_ends_at.timestamp();

    let vote_content = format!(
        "{}🏖️ **Leave of Absence Request**\n<@{}> is requesting a **{}-week** leave (<t:{}:D> → <t:{}:D>).\nThey can still log workouts, but missed-goal weeks won't count against them.\n\nReact ✅ to **approve** or ❌ to **deny** — voting closes <t:{}:R>.",
        if mention_str.is_empty() { String::new() } else { format!("{} ", mention_str) },
        user_id,
        weeks,
        loa_start.timestamp(),
        loa_end.timestamp(),
        vote_ends_unix
    );

    let channel = ChannelId::new(db_result.channel_id);
    let vote_msg = channel
        .send_message(ctx.serenity_context, serenity::CreateMessage::new().content(vote_content))
        .await?;

    vote_msg.react(ctx.serenity_context, serenity::ReactionType::Unicode("✅".to_string())).await?;
    vote_msg.react(ctx.serenity_context, serenity::ReactionType::Unicode("❌".to_string())).await?;

    // --- Persist ---
    let loa_start_str = format_datetime(&loa_start);
    let loa_end_str = format_datetime(&loa_end);

    let loa_id = {
        let conn = ctx.data.db.conn();
        let id = queries::insert_loa_request(
            &conn,
            guild_id,
            user_id,
            &format_datetime(&now),
            weeks as i64,
            db_result.channel_id,
            &vote_ends_str,
            &loa_start_str,
            &loa_end_str,
            mention_role_id,
        )?;
        queries::set_loa_vote_message(&conn, id, vote_msg.id.get())?;
        id
    };

    tracing::info!(
        "guild={} user={} cmd=loa_request weeks={} start={} end={} loa_id={}",
        guild_id, user_id, weeks, &loa_start_str[..10], &loa_end_str[..10], loa_id
    );

    Ok(())
}

/// Force-resolve a pending LOA vote immediately based on current reactions (admin only)
#[poise::command(
    slash_command,
    guild_only,
    rename = "resolve",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn loa_resolve(
    ctx: Context<'_>,
    #[description = "User whose pending LOA vote to resolve now"] user: serenity::User,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let loa = {
        let conn = ctx.data().db.conn();
        queries::get_pending_loa_for_user(&conn, guild_id, user.id.get())?
    };

    let loa = match loa {
        Some(l) => l,
        None => {
            ctx.say(format!("<@{}> has no pending LOA request.", user.id.get())).await?;
            return Ok(());
        }
    };

    crate::tasks::gym::resolve_loa_vote(&ctx.serenity_context().http, ctx.data(), &loa).await?;

    ctx.say("LOA vote resolved.").await?;
    Ok(())
}
