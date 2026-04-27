mod commands;
mod db;
mod images;
mod tasks;
mod util;

use db::gym::models::LoaRequest;

/// Returns true if every member of `role_id` (excluding the bot and the LOA requester)
/// has cast either a ✅ or ❌ reaction on the vote message.
async fn check_all_role_members_voted(
    ctx: &serenity::Context,
    loa: &LoaRequest,
    guild_id: serenity::GuildId,
    role_id: u64,
) -> bool {
    use poise::serenity_prelude::{ChannelId, MessageId, ReactionType, UserId};

    let bot_id = ctx.cache.current_user().id;
    let requester_id = UserId::new(loa.user_id);
    let role_id = serenity::RoleId::new(role_id);

    // Collect members who have the role (from cache)
    let role_members: Vec<UserId> = match ctx.cache.guild(guild_id) {
        Some(guild) => guild
            .members
            .values()
            .filter(|m| m.roles.contains(&role_id) && m.user.id != bot_id && m.user.id != requester_id)
            .map(|m| m.user.id)
            .collect(),
        None => return false,
    };

    if role_members.is_empty() {
        return false;
    }

    let channel = ChannelId::new(loa.vote_channel_id);
    let msg_id = match loa.vote_message_id {
        Some(id) => MessageId::new(id),
        None => return false,
    };

    // Fetch voters (up to 100 each — sufficient for typical Discord servers)
    let yes_users = ctx.http
        .get_reaction_users(channel, msg_id, &ReactionType::Unicode("✅".to_string()), 100, None)
        .await
        .unwrap_or_default();
    let no_users = ctx.http
        .get_reaction_users(channel, msg_id, &ReactionType::Unicode("❌".to_string()), 100, None)
        .await
        .unwrap_or_default();

    let voted: std::collections::HashSet<u64> = yes_users.iter().chain(no_users.iter())
        .map(|u| u.id.get())
        .collect();

    role_members.iter().all(|uid| voted.contains(&uid.get()))
}

use db::gym::queries;
use poise::serenity_prelude as serenity;
use rusqlite;
use std::sync::Arc;

/// User data shared across all commands
pub struct Data {
    pub db: db::Database,
}

impl std::fmt::Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data").field("db", &"Database").finish()
    }
}

/// Error type used throughout the bot
pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("gym_tracker_bot=debug".parse().unwrap())
                .add_directive("poise=info".parse().unwrap())
                .add_directive("serenity=info".parse().unwrap()),
        )
        .init();

    // Load environment variables
    if let Err(e) = dotenvy::dotenv() {
        tracing::warn!("No .env file found: {}", e);
    }

    // Get Discord token
    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set");

    // Get database path (default to ./data/gym_tracker.db)
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "data/gym_tracker.db".to_string());

    // Ensure data directory exists
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent).expect("Failed to create data directory");
    }

    // Initialize database
    let database = db::Database::new(&db_path).expect("Failed to open database");
    database.init_schema().expect("Failed to initialize database schema");
    tracing::info!("Database initialized at {}", db_path);

    // Create shared data for background tasks
    let shared_data = Arc::new(Data { db: database });
    let framework_data = shared_data.clone();

    // Create framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::commands(),
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    match event {
                        serenity::FullEvent::ReactionAdd { add_reaction } => {
                            if let serenity::ReactionType::Unicode(emoji) = &add_reaction.emoji {
                                let message_id = add_reaction.message_id.get();
                                let bot_id = ctx.cache.current_user().id;

                                if emoji == "🔥" {
                                    if let Some(user_id) = add_reaction.user_id {
                                        if user_id == bot_id { return Ok(()); }
                                        let conn = data.db.conn();
                                        if queries::get_log_message_guild(&conn, message_id)?.is_some() {
                                            let now_str = crate::util::time::format_datetime(&chrono::Utc::now());
                                            queries::upsert_log_reaction(&conn, message_id, user_id.get(), &now_str)?;
                                            tracing::debug!("🔥 reaction recorded: message={} user={}", message_id, user_id);
                                        }
                                    }
                                } else if emoji == "✅" || emoji == "❌" {
                                    if let Some(user_id) = add_reaction.user_id {
                                        if user_id == bot_id { return Ok(()); }

                                        // Check if this is a pending LOA vote message
                                        let loa = {
                                            let conn = data.db.conn();
                                            queries::get_loa_by_vote_message(&conn, message_id)?
                                        };

                                        if let Some(loa) = loa {
                                            // Requester cannot vote on their own LOA — remove the reaction
                                            if user_id.get() == loa.user_id {
                                                let channel = serenity::ChannelId::new(loa.vote_channel_id);
                                                let _ = ctx.http.delete_reaction(
                                                    channel,
                                                    add_reaction.message_id,
                                                    user_id,
                                                    &add_reaction.emoji,
                                                ).await;
                                                tracing::debug!(
                                                    "Removed LOA self-vote: loa_id={} user={}",
                                                    loa.id, user_id
                                                );
                                                return Ok(());
                                            }

                                            // If a mention role was set, auto-close once all role members have voted
                                            if let Some(role_id) = loa.mention_role_id {
                                                if let Some(guild_id) = add_reaction.guild_id {
                                                    let all_voted = check_all_role_members_voted(
                                                        ctx, &loa, guild_id, role_id,
                                                    ).await;
                                                    if all_voted {
                                                        tracing::info!(
                                                            "All role members voted on LOA id={} — resolving early",
                                                            loa.id
                                                        );
                                                        if let Err(e) = tasks::gym::resolve_loa_vote(
                                                            &ctx.http, data, &loa,
                                                        ).await {
                                                            tracing::error!(
                                                                "Early LOA resolve failed id={}: {}",
                                                                loa.id, e
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        serenity::FullEvent::ReactionRemove { removed_reaction } => {
                            if let serenity::ReactionType::Unicode(emoji) = &removed_reaction.emoji {
                                if emoji == "🔥" {
                                    if let Some(user_id) = removed_reaction.user_id {
                                        let message_id = removed_reaction.message_id.get();
                                        let conn = data.db.conn();
                                        if queries::get_log_message_guild(&conn, message_id)?.is_some() {
                                            queries::remove_log_reaction(&conn, message_id, user_id.get())?;
                                            tracing::debug!("🔥 reaction removed: message={} user={}", message_id, user_id);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    Ok(())
                })
            },
            on_error: |error| {
                Box::pin(async move {
                    match error {
                        poise::FrameworkError::Command { error, ctx, .. } => {
                            let is_internal = error.downcast_ref::<rusqlite::Error>().is_some();
                            let msg = if is_internal {
                                tracing::error!(
                                    "Internal error in /{}: {:?}",
                                    ctx.command().qualified_name,
                                    error
                                );
                                "Something went wrong on our end. Please try again, or let an admin know if it keeps happening.".to_string()
                            } else {
                                tracing::debug!(
                                    "Command user error in /{}: {}",
                                    ctx.command().qualified_name,
                                    error
                                );
                                error.to_string()
                            };
                            let _ = ctx.send(
                                poise::CreateReply::default().content(msg).ephemeral(true)
                            ).await;
                        }
                        poise::FrameworkError::ArgumentParse { error, ctx, .. } => {
                            let _ = ctx.send(
                                poise::CreateReply::default()
                                    .content(format!("Invalid argument: {}", error))
                                    .ephemeral(true)
                            ).await;
                        }
                        other => {
                            tracing::error!("Framework error: {:?}", other);
                        }
                    }
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            let data = framework_data;
            let http = ctx.http.clone();
            Box::pin(async move {
                tracing::info!("Bot is ready! Registering commands...");

                // Register commands — instantly to dev guild, or globally in production
                if std::env::var("ENVIRONMENT").as_deref() == Ok("development") {
                    let guild_id = serenity::GuildId::new(541468644782243875);
                    poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id).await?;
                    tracing::info!("Commands registered to dev guild!");
                } else {
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                    tracing::info!("Commands registered globally!");
                }

                // Start background tasks for each tracker
                tasks::start_gym_weekly_check(http, data.clone());
                tracing::info!("Background tasks started!");

                // Create new Data instance for framework
                let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "data/gym_tracker.db".to_string());
                let database = db::Database::new(&db_path).expect("Failed to open database");

                Ok(Data { db: database })
            })
        })
        .build();

    // Create client
    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::GUILD_MEMBERS;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .expect("Failed to create client");

    // Start the bot
    tracing::info!("Starting bot...");

    if let Err(e) = client.start().await {
        tracing::error!("Client error: {}", e);
    }
}
