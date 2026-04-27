mod commands;
mod db;
mod images;
mod tasks;
mod util;

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
                                if emoji == "🔥" {
                                    if let Some(user_id) = add_reaction.user_id {
                                        // Skip the bot's own reaction
                                        if user_id == ctx.cache.current_user().id { return Ok(()); }
                                        let message_id = add_reaction.message_id.get();
                                        let conn = data.db.conn();
                                        // Only record if this message is a tracked log post
                                        if queries::get_log_message_guild(&conn, message_id)?.is_some() {
                                            let now_str = crate::util::time::format_datetime(&chrono::Utc::now());
                                            queries::upsert_log_reaction(&conn, message_id, user_id.get(), &now_str)?;
                                            tracing::debug!("🔥 reaction recorded: message={} user={}", message_id, user_id);
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
        | serenity::GatewayIntents::MESSAGE_CONTENT
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
