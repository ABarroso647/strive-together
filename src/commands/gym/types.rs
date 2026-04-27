use super::Context;
use crate::db::gym::queries;
use crate::Error;
use poise::serenity_prelude as serenity;

/// Add an activity type
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn add_type(
    ctx: Context<'_>,
    #[description = "Name of the activity type (lowercase, no spaces)"] name: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    // Normalize the name: lowercase, trim, replace spaces with underscores
    let name = name.trim().to_lowercase().replace(' ', "_");

    if name.is_empty() {
        return Err("Activity type name cannot be empty.".into());
    }

    if name.len() > 32 {
        return Err("Activity type name must be 32 characters or less.".into());
    }

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up. Run `/gym setup` first.".into());
        }

        // Check if type already exists
        if queries::activity_type_exists(&conn, guild_id, &name)? {
            format!("Activity type **{}** already exists.", name)
        } else {
            queries::insert_activity_type(&conn, guild_id, &name)?;

            // Initialize this type for all existing users
            let users = queries::get_users(&conn, guild_id)?;
            for user_id in users {
                queries::set_user_type_total(&conn, guild_id, user_id, &name, 0)?;
            }

            format!("Added activity type **{}**.", name)
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// Remove an activity type
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn remove_type(
    ctx: Context<'_>,
    #[description = "Name of the activity type to remove"]
    #[autocomplete = "autocomplete_activity_type"]
    name: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let name = name.trim().to_lowercase();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        // Try to delete the type
        if queries::delete_activity_type(&conn, guild_id, &name)? {
            format!(
                "Removed activity type **{}**.\n\
                Note: Existing logs with this type are preserved but won't appear in new summaries.",
                name
            )
        } else {
            format!("Activity type **{}** does not exist.", name)
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// List all activity types
#[poise::command(slash_command, guild_only)]
pub async fn list_types(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let types = {
        let db = &ctx.data().db;
        let conn = db.conn();

        // Check if tracker is set up
        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        queries::get_activity_types(&conn, guild_id)?
    };

    if types.is_empty() {
        ctx.say("No activity types configured. Add some with `/gym add_type`.").await?;
    } else {
        let embed = serenity::CreateEmbed::new()
            .title("Activity Types")
            .description(types.join(", "))
            .field("Count", types.len().to_string(), true)
            .color(0x00aaff);

        ctx.send(poise::CreateReply::default().embed(embed)).await?;
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
