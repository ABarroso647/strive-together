# Contributing Guide

This document explains the codebase architecture and how to contribute new features.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Directory Structure](#directory-structure)
- [How It All Fits Together](#how-it-all-fits-together)
- [Adding a New Command](#adding-a-new-command)
- [Adding a New Tracker](#adding-a-new-tracker)
- [Database Patterns](#database-patterns)
- [Code Style](#code-style)

---

## Architecture Overview

The bot is built with:
- **[Poise](https://github.com/serenity-rs/poise)** - Discord bot framework (built on Serenity)
- **[Serenity](https://github.com/serenity-rs/serenity)** - Discord API library
- **[Rusqlite](https://github.com/rusqlite/rusqlite)** - SQLite database
- **[Resvg](https://github.com/RazrFalcon/resvg)** - SVG to PNG rendering
- **[Tokio](https://tokio.rs/)** - Async runtime

### Data Flow

```
User types /gym log push
        ↓
Discord sends interaction to bot
        ↓
Poise routes to src/commands/gym/log.rs
        ↓
Command handler calls src/db/gym/queries.rs
        ↓
Database updated, response sent back
```

### Key Concepts

1. **Commands** are async functions decorated with `#[poise::command]`
2. **Context** (`ctx`) gives access to Discord API and shared data
3. **Data** struct holds the database connection (shared across all commands)
4. **MutexGuard** - Database access must be scoped to avoid async issues

---

## Directory Structure

```
src/
├── main.rs                 # Entry point
│
├── commands/               # Discord slash commands
│   ├── mod.rs              # Exports commands() function
│   └── gym/                # Gym tracker commands
│       ├── mod.rs          # Defines /gym parent command
│       ├── setup.rs        # /gym setup, start, stop, info, config
│       ├── users.rs        # /gym add_user, remove_user, list_users, etc.
│       ├── types.rs        # /gym add_type, remove_type, list_types
│       ├── log.rs          # /gym log
│       ├── goals.rs        # /gym goal total/by_type/view
│       └── stats.rs        # /gym status, summary, totals, history
│
├── db/                     # Database layer
│   ├── mod.rs              # Database struct with Mutex<Connection>
│   └── gym/
│       ├── mod.rs          # Module exports
│       ├── schema.rs       # CREATE TABLE statements
│       ├── models.rs       # Rust structs for DB rows
│       └── queries.rs      # All SQL operations as functions
│
├── images/                 # Image generation
│   ├── mod.rs              # Shared SVG→PNG rendering
│   └── gym/
│       ├── summary.rs      # Weekly summary table
│       └── totals.rs       # Leaderboard image
│
├── tasks/                  # Background jobs
│   ├── mod.rs              # Task registration
│   └── gym/
│       └── weekly_check.rs # Hourly check for week rollover
│
└── util/
    ├── mod.rs
    └── time.rs             # Week boundary calculations
```

---

## How It All Fits Together

### main.rs

Sets up the Discord client and registers everything:

```rust
// 1. Load config from .env
let token = std::env::var("DISCORD_TOKEN")?;

// 2. Initialize database
let database = db::Database::new(&db_path)?;
database.init_schema()?;  // Runs all CREATE TABLE statements

// 3. Create Poise framework with commands
let framework = poise::Framework::builder()
    .options(poise::FrameworkOptions {
        commands: commands::commands(),  // Returns Vec of all commands
        ...
    })
    .setup(|ctx, _ready, framework| {
        // Register slash commands with Discord
        poise::builtins::register_globally(ctx, &framework.options().commands).await?;
        
        // Start background tasks
        tasks::start_gym_weekly_check(http, data);
        
        Ok(Data { db: database })
    })
    .build();

// 4. Connect and run
client.start().await?;
```

### Command Structure

Each command file follows this pattern:

```rust
use super::Context;           // Type alias for poise::Context
use crate::db::gym::queries;  // Database operations
use crate::Error;             // Box<dyn Error + Send + Sync>

/// Command description shown in Discord
#[poise::command(slash_command, guild_only)]
pub async fn my_command(
    ctx: Context<'_>,
    #[description = "Parameter description"] param: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    
    // Database operations must be in a block (MutexGuard can't cross await)
    let result = {
        let db = &ctx.data().db;
        let conn = db.conn();  // Returns MutexGuard
        queries::some_query(&conn, guild_id)?
    };  // MutexGuard dropped here
    
    // Now safe to await
    ctx.say(format!("Result: {}", result)).await?;
    Ok(())
}
```

### Database Pattern

**Important**: The `MutexGuard` from `db.conn()` cannot be held across `.await` points. Always scope database operations:

```rust
// CORRECT ✓
let data = {
    let conn = db.conn();
    queries::get_something(&conn)?
};  // conn dropped
ctx.say("done").await?;

// WRONG ✗
let conn = db.conn();
let data = queries::get_something(&conn)?;
ctx.say("done").await?;  // Error: MutexGuard not Send
```

---

## Adding a New Command

### Example: Adding `/gym streak` to show current streak

1. **Add the function to an existing file** (or create new file):

```rust
// src/commands/gym/stats.rs

/// Show your current workout streak
#[poise::command(slash_command, guild_only)]
pub async fn streak(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?.get();
    let user_id = ctx.author().id.get();

    let streak = {
        let conn = ctx.data().db.conn();
        // Add query to queries.rs first
        queries::get_user_streak(&conn, guild_id, user_id)?
    };

    ctx.say(format!("Your current streak: {} days", streak)).await?;
    Ok(())
}
```

2. **Register it in the parent command** (`src/commands/gym/mod.rs`):

```rust
#[poise::command(
    slash_command,
    subcommands(
        // ... existing commands ...
        "stats::streak",  // Add this line
    ),
    subcommand_required
)]
pub async fn gym(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}
```

3. **Add any needed database queries** (`src/db/gym/queries.rs`):

```rust
pub fn get_user_streak(conn: &Connection, guild_id: u64, user_id: u64) -> Result<i32, rusqlite::Error> {
    // Implementation
}
```

4. **Test it**:
```bash
cargo run
# Then in Discord: /gym streak
```

---

## Adding a New Tracker

To add a calorie tracker (or any new tracker):

### 1. Create the command module

```
src/commands/calories/
├── mod.rs          # Parent /calories command
├── setup.rs        # /calories setup, start, stop
├── log.rs          # /calories log <amount>
└── stats.rs        # /calories status, summary
```

`src/commands/calories/mod.rs`:
```rust
mod setup;
mod log;
mod stats;

use crate::{Data, Error};
pub type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(
    slash_command,
    subcommands(
        "setup::setup",
        "setup::start",
        "log::log",
        "stats::status",
    ),
    subcommand_required
)]
pub async fn calories(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}
```

### 2. Create the database module

```
src/db/calories/
├── mod.rs
├── schema.rs       # calories_* tables
├── models.rs
└── queries.rs
```

`src/db/calories/schema.rs`:
```rust
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS calories_guild_config (
    guild_id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL,
    default_goal INTEGER NOT NULL DEFAULT 2000,
    started INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS calories_users (
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    PRIMARY KEY (guild_id, user_id)
);

CREATE TABLE IF NOT EXISTS calories_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    amount INTEGER NOT NULL,
    logged_at TEXT NOT NULL
);
"#;
```

### 3. Register everything

`src/commands/mod.rs`:
```rust
pub mod gym;
pub mod calories;  // Add this

pub fn commands() -> Vec<poise::Command<Data, Error>> {
    vec![
        gym::gym(),
        calories::calories(),  // Add this
    ]
}
```

`src/db/mod.rs`:
```rust
pub mod gym;
pub mod calories;  // Add this

impl Database {
    pub fn init_schema(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(gym::schema::SCHEMA)?;
        conn.execute_batch(calories::schema::SCHEMA)?;  // Add this
        Ok(())
    }
}
```

### 4. (Optional) Add images and background tasks

Follow the same pattern in `src/images/calories/` and `src/tasks/calories/`.

---

## Database Patterns

### Table Naming
All tables for a tracker are prefixed: `gym_users`, `gym_logs`, `calories_users`, etc.

### Common Operations

```rust
// Check if something exists
pub fn user_exists(conn: &Connection, guild_id: u64, user_id: u64) -> Result<bool, rusqlite::Error> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM gym_users WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

// Get optional row
pub fn get_config(conn: &Connection, guild_id: u64) -> Result<Option<Config>, rusqlite::Error> {
    conn.query_row(
        "SELECT ... FROM gym_guild_config WHERE guild_id = ?",
        [guild_id],
        |row| Ok(Config { ... }),
    )
    .optional()  // Returns Ok(None) instead of error if not found
}

// Insert or update
pub fn upsert_setting(conn: &Connection, ...) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO table (...) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![...],
    )?;
    Ok(())
}
```

---

## Code Style

### Error Handling
- Use `?` for propagating errors
- Return user-friendly error messages: `return Err("Gym tracker not set up.".into())`
- Log internal errors with `tracing::error!`

### Naming
- Commands: `snake_case` (Discord converts to kebab-case)
- Database tables: `tracker_table_name` (e.g., `gym_users`)
- Functions: `snake_case`
- Structs: `PascalCase`

### Comments
- Add doc comments (`///`) to public functions
- Command descriptions show in Discord's UI
- Parameter descriptions show when users type the command

### Testing Locally
```bash
# Quick check (no compile)
cargo check

# Run with debug logs
RUST_LOG=gym_tracker_bot=debug cargo run

# Build release
cargo build --release
```

---

## Questions?

Open an issue or check the existing code for patterns to follow!
