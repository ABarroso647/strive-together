mod commands;
mod db;
mod images;
mod tasks;
mod util;

use poise::serenity_prelude as serenity;
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
            on_error: |error| {
                Box::pin(async move {
                    match error {
                        poise::FrameworkError::Command { error, ctx, .. } => {
                            tracing::error!("Command error: {}", error);
                            let _ = ctx.say(format!("An error occurred: {}", error)).await;
                        }
                        poise::FrameworkError::ArgumentParse { error, ctx, .. } => {
                            let _ = ctx.say(format!("Invalid argument: {}", error)).await;
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

                // Register commands globally (or use guild_id for testing)
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                tracing::info!("Commands registered!");

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
