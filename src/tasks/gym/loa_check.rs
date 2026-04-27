use chrono::Utc;
use poise::serenity_prelude::{self as serenity, ChannelId, Http, MessageId, ReactionType, UserId};
use std::sync::Arc;
use tokio::time;

use crate::db::gym::queries;
use crate::util::time::{format_datetime, parse_datetime};
use crate::Data;

/// Return the earliest pending LOA vote_ends_at for smart sleep.
fn next_vote_end_time(data: &Data) -> Option<chrono::DateTime<Utc>> {
    let conn = data.db.conn();
    queries::get_earliest_pending_vote_end(&conn)
        .and_then(|s| parse_datetime(&s).ok())
}

pub fn start_loa_check_task(http: Arc<Http>, data: Arc<Data>) {
    tokio::spawn(async move {
        // Stagger behind the weekly check startup
        time::sleep(time::Duration::from_secs(90)).await;

        loop {
            // Process any expired votes first
            if let Err(e) = check_loa_votes(&http, &data).await {
                tracing::error!("LOA vote check error: {}", e);
            }

            // Sleep until the next vote window expires (+ 30s buffer)
            match next_vote_end_time(&data) {
                None => {
                    // No pending votes — check every hour in case one comes in
                    tracing::debug!("No pending LOA votes; sleeping 1h");
                    time::sleep(time::Duration::from_secs(3600)).await;
                }
                Some(end_time) => {
                    let now = Utc::now();
                    if end_time > now {
                        let secs = (end_time - now).num_seconds().max(0) as u64;
                        tracing::info!(
                            "Next LOA vote closes at {} (in {}m {}s)",
                            end_time.format("%Y-%m-%d %H:%M UTC"),
                            secs / 60,
                            secs % 60,
                        );
                        time::sleep(time::Duration::from_secs(secs + 30)).await;
                    }
                    // If overdue: loop immediately
                }
            }
        }
    });
}

async fn check_loa_votes(
    http: &Http,
    data: &Data,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now_str = format_datetime(&Utc::now());
    let expired = {
        let conn = data.db.conn();
        queries::get_expired_pending_loas(&conn, &now_str)?
    };
    for loa in expired {
        if let Err(e) = resolve_loa_vote(http, data, &loa).await {
            tracing::error!(
                "Failed to resolve LOA vote id={} guild={} user={}: {}",
                loa.id, loa.guild_id, loa.user_id, e
            );
        }
    }
    Ok(())
}

async fn resolve_loa_vote(
    http: &Http,
    data: &Data,
    loa: &crate::db::gym::models::LoaRequest,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let channel = ChannelId::new(loa.vote_channel_id);
    let bot_id = http.get_current_user().await?.id;
    let requester_id = UserId::new(loa.user_id);

    let (yes_votes, no_votes) = if let Some(msg_id) = loa.vote_message_id {
        let message_id = MessageId::new(msg_id);

        // Fetch the full voter lists so we can exclude the bot and the requester
        let yes_users = http
            .get_reaction_users(channel, message_id, &ReactionType::Unicode("✅".to_string()), 100, None)
            .await
            .unwrap_or_default();
        let no_users = http
            .get_reaction_users(channel, message_id, &ReactionType::Unicode("❌".to_string()), 100, None)
            .await
            .unwrap_or_default();

        let is_voter = |users: &[serenity::User], uid: UserId| users.iter().any(|u| u.id == uid);

        // Exclude bot's seeded reaction and requester's own vote
        let yes = yes_users.len() as i64
            - if is_voter(&yes_users, bot_id) { 1 } else { 0 }
            - if is_voter(&yes_users, requester_id) { 1 } else { 0 };
        let no = no_users.len() as i64
            - if is_voter(&no_users, bot_id) { 1 } else { 0 }
            - if is_voter(&no_users, requester_id) { 1 } else { 0 };

        (yes.max(0) as u64, no.max(0) as u64)
    } else {
        (0, 0)
    };

    // No votes at all → auto-approve
    let approved = yes_votes > no_votes || (yes_votes == 0 && no_votes == 0);
    let status = if approved { "approved" } else { "denied" };

    {
        let conn = data.db.conn();
        queries::resolve_loa(&conn, loa.id, status)?;
    }

    tracing::info!(
        "LOA id={} guild={} user={} resolved={} ({}✅ {}❌)",
        loa.id, loa.guild_id, loa.user_id, status, yes_votes, no_votes
    );

    let result_msg = if approved {
        let end_str = loa.loa_end.as_deref().map(|s| &s[..10]).unwrap_or("?");
        if yes_votes == 0 && no_votes == 0 {
            format!(
                "✅ **LOA Approved** (no votes cast — auto-approved) — <@{}>'s {}-week leave is granted through {}. Goal stats are paused until then.",
                loa.user_id, loa.weeks, end_str
            )
        } else {
            format!(
                "✅ **LOA Approved** ({} for, {} against) — <@{}>'s {}-week leave is granted through {}. Goal stats are paused until then.",
                yes_votes, no_votes, loa.user_id, loa.weeks, end_str
            )
        }
    } else {
        format!(
            "❌ **LOA Denied** ({} for, {} against) — <@{}>'s leave request did not pass.",
            yes_votes, no_votes, loa.user_id
        )
    };

    channel.send_message(http, serenity::CreateMessage::new().content(result_msg)).await?;
    Ok(())
}
