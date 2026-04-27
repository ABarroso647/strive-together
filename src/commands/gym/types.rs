use super::Context;
use crate::db::gym::queries;
use crate::Error;
use poise::serenity_prelude as serenity;

/// Manage activity types
#[poise::command(slash_command, guild_only, rename = "type", subcommands("add_type", "remove_type", "list_types"))]
pub async fn types_cmd(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Add an activity type and assign it to a group
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "add")]
pub async fn add_type(
    ctx: Context<'_>,
    #[description = "Name of the activity type (lowercase, no spaces)"] name: String,
    #[description = "Group to assign this type to"]
    #[autocomplete = "autocomplete_group"]
    group: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let name = name.trim().to_lowercase().replace(' ', "_");
    let group = group.trim().to_lowercase();

    if name.is_empty() {
        return Err("Activity type name cannot be empty.".into());
    }
    if name.len() > 32 {
        return Err("Activity type name must be 32 characters or less.".into());
    }

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up. Run `/gym setup` first.".into());
        }

        if !queries::group_exists(&conn, guild_id, &group)? {
            let groups = queries::get_activity_groups(&conn, guild_id)?;
            if groups.is_empty() {
                return Err("No groups exist yet. Create one with `/gym group create <name>` first.".into());
            }
            return Err(format!(
                "Group '{}' doesn't exist. Available: {}",
                group,
                groups.join(", ")
            ).into());
        }

        if queries::activity_type_exists(&conn, guild_id, &name)? {
            return Err(format!("Activity type **{}** already exists.", name).into());
        }

        queries::insert_activity_type(&conn, guild_id, &name)?;
        queries::assign_type_to_group(&conn, guild_id, &name, &group)?;

        let users = queries::get_users(&conn, guild_id)?;
        for user_id in users {
            queries::set_user_type_total(&conn, guild_id, user_id, &name, 0)?;
        }

        tracing::info!("guild={} user={} cmd=type_add name={} group={}", guild_id, ctx.author().id.get(), name, group);
        format!("Added activity type **{}** in group **{}**.", name, group)
    };

    ctx.say(response).await?;
    Ok(())
}

/// Remove an activity type
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "remove")]
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
            tracing::info!("guild={} user={} cmd=type_remove name={}", guild_id, ctx.author().id.get(), name);
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

/// List all activity types grouped by their group
#[poise::command(slash_command, guild_only, rename = "list")]
pub async fn list_types(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let (groups, type_group_map, all_types) = {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        let groups = queries::get_activity_groups(&conn, guild_id)?;
        let type_group_map = queries::get_all_type_groups(&conn, guild_id)?;
        let all_types = queries::get_activity_types(&conn, guild_id)?;
        (groups, type_group_map, all_types)
    };

    if all_types.is_empty() {
        ctx.say("No activity types configured. Add some with `/gym type add`.").await?;
        return Ok(());
    }

    let mut embed = serenity::CreateEmbed::new()
        .title("Activity Types")
        .color(0x00aaff);

    for group_name in &groups {
        let members: Vec<&str> = all_types.iter()
            .filter(|t| type_group_map.get(*t).map(|g| g == group_name).unwrap_or(false))
            .map(|t| t.as_str())
            .collect();
        if !members.is_empty() {
            embed = embed.field(group_name, members.join(", "), false);
        }
    }

    let unassigned: Vec<&str> = all_types.iter()
        .filter(|t| !type_group_map.contains_key(*t))
        .map(|t| t.as_str())
        .collect();
    if !unassigned.is_empty() {
        embed = embed.field("unassigned", unassigned.join(", "), false);
    }

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

async fn autocomplete_group<'a>(ctx: Context<'a>, partial: &'a str) -> Vec<String> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.get(),
        None => return vec![],
    };
    let db = &ctx.data().db;
    let conn = db.conn();
    queries::get_activity_groups(&conn, guild_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|g| g.to_lowercase().contains(&partial.to_lowercase()))
        .take(25)
        .collect()
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
