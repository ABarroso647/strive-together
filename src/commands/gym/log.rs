use super::Context;
use crate::db::gym::queries;
use crate::util::time::format_datetime;
use crate::Error;
use chrono::Utc;
use poise::serenity_prelude as serenity;

/// Log a workout
#[poise::command(slash_command, guild_only)]
pub async fn log(
    ctx: Context<'_>,
    #[description = "Activity type"]
    #[autocomplete = "autocomplete_activity_type"]
    activity_type: String,
    #[description = "Additional user 1"] user2: Option<serenity::User>,
    #[description = "Additional user 2"] user3: Option<serenity::User>,
    #[description = "Optional image"] image: Option<serenity::Attachment>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let activity_type = activity_type.trim().to_lowercase();

    // Collect all users to log for
    let mut users_to_log = vec![ctx.author().id.get()];
    if let Some(u) = &user2 {
        if !users_to_log.contains(&u.id.get()) {
            users_to_log.push(u.id.get());
        }
    }
    if let Some(u) = &user3 {
        if !users_to_log.contains(&u.id.get()) {
            users_to_log.push(u.id.get());
        }
    }

    // Process in database scope — returns what we need to send + store
    struct LogData {
        response: String,
        period_id: i64,
        valid_users: Vec<u64>,
    }

    let log_data = {
        let db = &ctx.data().db;
        let conn = db.conn();

        let config = match queries::get_guild_config(&conn, guild_id)? {
            Some(c) => c,
            None => return Err("Gym tracker not set up.".into()),
        };
        if !config.started {
            return Err("Tracking hasn't started yet. An admin needs to run `/gym start`.".into());
        }

        if !queries::activity_type_exists(&conn, guild_id, &activity_type)? {
            let types = queries::get_activity_types(&conn, guild_id)?;
            return Err(format!(
                "Activity type '{}' doesn't exist.\nAvailable types: {}",
                activity_type,
                types.join(", ")
            ).into());
        }

        let period = match queries::get_current_period(&conn, guild_id)? {
            Some(p) => p,
            None => return Err("No active period. An admin needs to run `/gym start`.".into()),
        };

        let mut valid_users = Vec::new();
        let mut invalid_users = Vec::new();
        for user_id in &users_to_log {
            if queries::user_exists(&conn, guild_id, *user_id)? {
                valid_users.push(*user_id);
            } else {
                invalid_users.push(*user_id);
            }
        }
        if valid_users.is_empty() {
            return Err("None of the specified users are in the gym tracker.".into());
        }

        let logged_at = format_datetime(&Utc::now());
        for user_id in &valid_users {
            queries::insert_log(&conn, guild_id, *user_id, period.id, &activity_type, &logged_at)?;
        }

        let mentions: Vec<String> = valid_users.iter().map(|id| format!("<@{}>", id)).collect();
        let mut response = format!("💪 **{}** — {}", activity_type, mentions.join(", "));
        if !invalid_users.is_empty() {
            let skipped: Vec<String> = invalid_users.iter().map(|id| format!("<@{}>", id)).collect();
            response.push_str(&format!("\n*(Skipped {} — not in tracker)*", skipped.join(", ")));
        }

        LogData { response, period_id: period.id, valid_users }
    };

    // Send reply — link to image via camera emoji so Discord previews without exposing filename
    let reply_content = match &image {
        Some(att) => format!("{} [📷]({})", log_data.response, att.url),
        None => log_data.response.clone(),
    };
    let reply = ctx.send(poise::CreateReply::default().content(reply_content)).await?;

    // Add 🔥 reaction and store message ID + any attachments
    if let Ok(msg) = reply.message().await {
        let message_id = msg.id.get();
        let channel_id = msg.channel_id.get();

        // Best-effort: react + store (don't fail the command if these error)
        let _ = msg.react(ctx.http(), serenity::ReactionType::Unicode("🔥".to_string())).await;

        let db = &ctx.data().db;
        let conn = db.conn();
        let _ = queries::insert_log_message(&conn, message_id, guild_id, channel_id, log_data.period_id);

        // Store original image attachment URL from the interaction (not the sent message,
        // since we embed the URL as text rather than re-uploading)
        if let Some(ref att) = image {
            let now_str = format_datetime(&Utc::now());
            let author_id = ctx.author().id.get();
            let _ = queries::insert_log_attachment(&conn, message_id, guild_id, author_id, &att.url, &att.filename, &now_str);
        }

        tracing::info!("guild={} user={} log activity_type={} users={:?} message_id={}",
            guild_id, ctx.author().id.get(), activity_type, log_data.valid_users, message_id);
    }

    Ok(())
}

/// Log a workout retroactively for a past week
#[poise::command(slash_command, guild_only)]
pub async fn log_past(
    ctx: Context<'_>,
    #[description = "Activity type"]
    #[autocomplete = "autocomplete_activity_type"]
    activity_type: String,
    #[description = "How many weeks ago (1 = last week, 2 = two weeks ago, up to 12)"]
    #[min = 1_i32]
    #[max = 12_i32]
    weeks_ago: i32,
    #[description = "Additional user 1"] user2: Option<serenity::User>,
    #[description = "Additional user 2"] user3: Option<serenity::User>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let activity_type = activity_type.trim().to_lowercase();

    let mut users_to_log = vec![ctx.author().id.get()];
    if let Some(u) = &user2 {
        if !users_to_log.contains(&u.id.get()) { users_to_log.push(u.id.get()); }
    }
    if let Some(u) = &user3 {
        if !users_to_log.contains(&u.id.get()) { users_to_log.push(u.id.get()); }
    }

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        let config = match queries::get_guild_config(&conn, guild_id)? {
            Some(c) => c,
            None => return Err("Gym tracker not set up.".into()),
        };
        if !config.started {
            return Err("Tracking hasn't started yet. An admin needs to run `/gym start`.".into());
        }
        if !queries::activity_type_exists(&conn, guild_id, &activity_type)? {
            let types = queries::get_activity_types(&conn, guild_id)?;
            return Err(format!(
                "Activity type '{}' doesn't exist.\nAvailable types: {}",
                activity_type,
                types.join(", ")
            ).into());
        }

        // Fetch the target past period (limit = weeks_ago, result[0] = oldest = target)
        let periods = queries::get_completed_periods(&conn, guild_id, weeks_ago as usize)?;
        if (periods.len() as i32) < weeks_ago {
            return Err(format!(
                "Only {} completed week(s) exist so far — can't log for {} weeks ago.",
                periods.len(), weeks_ago
            ).into());
        }
        // get_completed_periods returns oldest→newest; [0] is the oldest = the furthest-back week
        let period = &periods[0];
        let period_id = period.id;
        let period_start = &period.start_time[..10];
        let period_end = &period.end_time[..10];

        let mut valid_users = Vec::new();
        let mut invalid_users = Vec::new();
        for user_id in &users_to_log {
            if queries::user_exists(&conn, guild_id, *user_id)? {
                valid_users.push(*user_id);
            } else {
                invalid_users.push(*user_id);
            }
        }
        if valid_users.is_empty() {
            return Err("None of the specified users are in the gym tracker.".into());
        }

        let now = chrono::Utc::now();
        let logged_at = format_datetime(&now);

        for user_id in &valid_users {
            // Insert the log entry into the past period
            queries::insert_log(&conn, guild_id, *user_id, period_id, &activity_type, &logged_at)?;
            // Update the archived period summaries
            queries::increment_period_type_count_upsert(&conn, period_id, *user_id, &activity_type, 1)?;
            queries::increment_period_result_count(&conn, period_id, *user_id, 1)?;
            // Update all-time totals
            queries::update_user_totals(&conn, guild_id, *user_id, 1, 0, 0)?;
            queries::increment_user_type_total(&conn, guild_id, *user_id, &activity_type, 1)?;
        }

        let mentions: Vec<String> = valid_users.iter().map(|id| format!("<@{}>", id)).collect();
        let mut msg = format!(
            "Retroactively logged **{}** for {} — week of {} to {}.",
            activity_type,
            mentions.join(", "),
            period_start,
            period_end
        );
        if !invalid_users.is_empty() {
            let skipped: Vec<String> = invalid_users.iter().map(|id| format!("<@{}>", id)).collect();
            msg.push_str(&format!("\n(Skipped {} — not in tracker)", skipped.join(", ")));
        }
        tracing::info!("guild={} user={} cmd=log_past activity_type={} weeks_ago={} users={:?}",
            guild_id, ctx.author().id.get(), activity_type, weeks_ago, valid_users);
        msg
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
