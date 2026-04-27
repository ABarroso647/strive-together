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

    // Process in database scope
    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up and started
        let config = match queries::get_guild_config(&conn, guild_id)? {
            Some(c) => c,
            None => return Err("Gym tracker not set up.".into()),
        };

        if !config.started {
            return Err("Tracking hasn't started yet. An admin needs to run `/gym start`.".into());
        }

        // Check if activity type exists
        if !queries::activity_type_exists(&conn, guild_id, &activity_type)? {
            let types = queries::get_activity_types(&conn, guild_id)?;
            return Err(format!(
                "Activity type '{}' doesn't exist.\nAvailable types: {}",
                activity_type,
                types.join(", ")
            ).into());
        }

        // Get current period
        let period = match queries::get_current_period(&conn, guild_id)? {
            Some(p) => p,
            None => return Err("No active period. An admin needs to run `/gym start`.".into()),
        };

        // Validate all users are registered
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

        // Log for each valid user
        let now = Utc::now();
        let logged_at = format_datetime(&now);

        for user_id in &valid_users {
            queries::insert_log(&conn, guild_id, *user_id, period.id, &activity_type, &logged_at)?;
        }

        // Build response
        let mentions: Vec<String> = valid_users.iter().map(|id| format!("<@{}>", id)).collect();
        let mut response = format!(
            "Logged **{}** for: {}",
            activity_type,
            mentions.join(", ")
        );

        if !invalid_users.is_empty() {
            let invalid_mentions: Vec<String> = invalid_users.iter().map(|id| format!("<@{}>", id)).collect();
            response.push_str(&format!(
                "\n(Skipped {} - not in tracker)",
                invalid_mentions.join(", ")
            ));
        }

        response
    };

    // Send response (with image if provided)
    if let Some(attachment) = image {
        // Download and re-upload the image
        let image_data = attachment.download().await?;
        let attachment = serenity::CreateAttachment::bytes(image_data, &attachment.filename);

        ctx.send(
            poise::CreateReply::default()
                .content(response)
                .attachment(attachment)
        ).await?;
    } else {
        ctx.say(response).await?;
    }

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
