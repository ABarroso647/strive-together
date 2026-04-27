use super::Context;
use crate::db::gym::queries;
use crate::Error;

/// Manage activity groups
#[poise::command(
    slash_command,
    guild_only,
    subcommands("group_create", "group_delete", "group_list", "group_assign", "group_unassign")
)]
pub async fn group(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Create a new activity group (admin only)
#[poise::command(
    slash_command,
    guild_only,
    rename = "create",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn group_create(
    ctx: Context<'_>,
    #[description = "Group name (e.g. gym, cardio, wellness)"] name: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let name = name.trim().to_lowercase();

    if name.is_empty() {
        return Err("Group name cannot be empty.".into());
    }

    {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        let created = queries::insert_activity_group(&conn, guild_id, &name)?;
        if !created {
            return Err(format!("Group '{}' already exists.", name).into());
        }
    }

    tracing::info!("guild={} user={} cmd=group_create name={}", guild_id, ctx.author().id.get(), name);
    ctx.say(format!(
        "Created group **{}**.\nAssign activity types to it with `/gym group assign {} <type>`.",
        name, name
    )).await?;
    Ok(())
}

/// Delete an activity group (admin only)
#[poise::command(
    slash_command,
    guild_only,
    rename = "delete",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn group_delete(
    ctx: Context<'_>,
    #[description = "Group name"]
    #[autocomplete = "autocomplete_group"]
    name: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let name = name.trim().to_lowercase();

    {
        let db = &ctx.data().db;
        let conn = db.conn();

        let deleted = queries::delete_activity_group(&conn, guild_id, &name)?;
        if !deleted {
            return Err(format!("Group '{}' doesn't exist.", name).into());
        }
    }

    tracing::info!("guild={} user={} cmd=group_delete name={}", guild_id, ctx.author().id.get(), name);
    ctx.say(format!("Deleted group **{}**. Activity types that were in this group are now unassigned.", name)).await?;
    Ok(())
}

/// List all groups and their assigned activity types
#[poise::command(slash_command, guild_only, rename = "list")]
pub async fn group_list(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();

    let response = {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        let groups = queries::get_activity_groups(&conn, guild_id)?;

        if groups.is_empty() {
            "No groups created yet. Use `/gym group create <name>` to create one.".to_string()
        } else {
            let mut lines = vec!["**Activity Groups:**".to_string()];
            for group_name in &groups {
                let types = queries::get_group_types(&conn, guild_id, group_name)?;
                let types_str = if types.is_empty() {
                    "*(no types assigned)*".to_string()
                } else {
                    types.join(", ")
                };
                lines.push(format!("**{}**: {}", group_name, types_str));
            }

            // Also list unassigned types
            let all_types = queries::get_activity_types(&conn, guild_id)?;
            let type_group_map = queries::get_all_type_groups(&conn, guild_id)?;
            let unassigned: Vec<&str> = all_types.iter()
                .filter(|t| !type_group_map.contains_key(*t))
                .map(|t| t.as_str())
                .collect();

            if !unassigned.is_empty() {
                lines.push(format!("**Unassigned**: {}", unassigned.join(", ")));
            }

            lines.join("\n")
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// Assign an activity type to a group (admin only)
#[poise::command(
    slash_command,
    guild_only,
    rename = "assign",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn group_assign(
    ctx: Context<'_>,
    #[description = "Group name"]
    #[autocomplete = "autocomplete_group"]
    group: String,
    #[description = "Activity type"]
    #[autocomplete = "autocomplete_activity_type"]
    activity_type: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let group = group.trim().to_lowercase();
    let activity_type = activity_type.trim().to_lowercase();

    {
        let db = &ctx.data().db;
        let conn = db.conn();

        if queries::get_guild_config(&conn, guild_id)?.is_none() {
            return Err("Gym tracker not set up.".into());
        }

        if !queries::group_exists(&conn, guild_id, &group)? {
            return Err(format!("Group '{}' doesn't exist. Create it first with `/gym group create`.", group).into());
        }

        if !queries::activity_type_exists(&conn, guild_id, &activity_type)? {
            return Err(format!("Activity type '{}' doesn't exist.", activity_type).into());
        }

        queries::assign_type_to_group(&conn, guild_id, &activity_type, &group)?;
    }

    tracing::info!("guild={} user={} cmd=group_assign type={} group={}", guild_id, ctx.author().id.get(), activity_type, group);
    ctx.say(format!("Assigned **{}** to group **{}**.", activity_type, group)).await?;
    Ok(())
}

/// Remove an activity type from its group (admin only)
#[poise::command(
    slash_command,
    guild_only,
    rename = "unassign",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn group_unassign(
    ctx: Context<'_>,
    #[description = "Activity type"]
    #[autocomplete = "autocomplete_activity_type"]
    activity_type: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let activity_type = activity_type.trim().to_lowercase();

    let current_group = {
        let db = &ctx.data().db;
        let conn = db.conn();

        let group = queries::get_type_group(&conn, guild_id, &activity_type)?;
        if group.is_none() {
            return Err(format!("'{}' is not assigned to any group.", activity_type).into());
        }
        queries::unassign_type_from_group(&conn, guild_id, &activity_type)?;
        group.unwrap()
    };

    tracing::info!("guild={} user={} cmd=group_unassign type={} from_group={}", guild_id, ctx.author().id.get(), activity_type, current_group);
    ctx.say(format!("Removed **{}** from group **{}**.", activity_type, current_group)).await?;
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

async fn autocomplete_activity_type<'a>(ctx: Context<'a>, partial: &'a str) -> Vec<String> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.get(),
        None => return vec![],
    };
    let db = &ctx.data().db;
    let conn = db.conn();
    queries::get_activity_types(&conn, guild_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|t| t.to_lowercase().contains(&partial.to_lowercase()))
        .take(25)
        .collect()
}

