# Strive Together Bot

A Discord bot for tracking fitness goals with weekly accountability. Currently supports gym workout tracking, with an extensible architecture for future trackers (calories, etc.).

## Documentation

- **[Getting Started](GETTING_STARTED.md)** - Set up and run the bot locally
- **[Commands](COMMANDS.md)** - Full command reference
- **[Contributing](CONTRIBUTING.md)** - Understand the codebase and add features

## Features

### Gym Tracker
- **Workout Logging**: Log workouts with different activity types
- **Flexible Goals**: Set total weekly goals or per-activity-type goals
- **Weekly Summaries**: Auto-generated image summaries at week end
- **Leaderboards**: All-time totals with rankings
- **Multi-user Support**: Track multiple users per server
- **History**: View week-by-week progress

## Commands

All gym tracker commands are under the `/gym` parent command:

### Admin Commands (require ADMINISTRATOR)
- `/gym setup` - Initialize the gym tracker in a channel
- `/gym start` - Start tracking (creates first weekly period)
- `/gym stop` - Stop tracking
- `/gym info` - Show current tracker configuration
- `/gym config goal <amount>` - Set default goal for new users
- `/gym add_user @user` - Add a user to the tracker
- `/gym remove_user @user` - Remove a user from the tracker
- `/gym add_type <name>` - Add an activity type
- `/gym remove_type <name>` - Remove an activity type
- `/gym import_user @user <json>` - Import user data from JSON
- `/gym set_type_total @user <type> <count>` - Set a user's type total
- `/gym set_goal_stats @user <achieved> <missed>` - Set goal statistics

### User Commands
- `/gym log <type> [user2] [user3] [image]` - Log a workout
- `/gym goal total <count>` - Set your total weekly goal
- `/gym goal by_type <type> <count>` - Set a per-type goal
- `/gym goal view` - View your goal settings
- `/gym status` - Show your current week progress
- `/gym summary` - Show weekly summary image for all users
- `/gym totals` - Show all-time leaderboard image
- `/gym history [@user]` - Show week-by-week history
- `/gym list_users` - List tracked users
- `/gym list_types` - List activity types

## Architecture

The bot is designed to support multiple trackers:

```
src/
├── main.rs              # Entry point
├── commands/
│   ├── mod.rs           # Command registration
│   └── gym/             # Gym tracker commands
│       ├── mod.rs
│       ├── setup.rs
│       ├── users.rs
│       ├── types.rs
│       ├── log.rs
│       ├── goals.rs
│       └── stats.rs
├── db/
│   ├── mod.rs           # Database wrapper
│   └── gym/             # Gym tracker database
│       ├── mod.rs
│       ├── schema.rs    # Tables prefixed with gym_
│       ├── models.rs
│       └── queries.rs
├── images/
│   ├── mod.rs           # Shared rendering utilities
│   └── gym/             # Gym tracker images
│       ├── mod.rs
│       ├── summary.rs
│       └── totals.rs
├── tasks/
│   ├── mod.rs           # Task registration
│   └── gym/             # Gym tracker background tasks
│       └── weekly_check.rs
└── util/
    ├── mod.rs
    └── time.rs          # Shared time utilities
```

To add a new tracker (e.g., calories):
1. Create `src/commands/calories/` with commands
2. Create `src/db/calories/` with schema (tables prefixed with `calories_`)
3. Create `src/images/calories/` for any image generation
4. Create `src/tasks/calories/` for background tasks
5. Register in respective `mod.rs` files

## Setup

### Prerequisites
- Docker and Docker Compose
- Discord Bot Token

### Discord Bot Setup
1. Go to [Discord Developer Portal](https://discord.com/developers/applications)
2. Create a new application
3. Go to "Bot" and create a bot
4. Enable these Privileged Gateway Intents:
   - Server Members Intent
   - Message Content Intent
5. Copy the bot token
6. Go to "OAuth2" > "URL Generator"
7. Select scopes: `bot`, `applications.commands`
8. Select permissions: `Send Messages`, `Attach Files`, `Use Slash Commands`
9. Use the generated URL to invite the bot to your server

### Deployment

1. Clone this repository
2. Copy `.env.example` to `.env` and add your Discord token:
   ```bash
   cp .env.example .env
   # Edit .env and add your DISCORD_TOKEN
   ```

3. Build and run with Docker Compose:
   ```bash
   docker compose up -d
   ```

4. Check logs:
   ```bash
   docker compose logs -f
   ```

### Local Development

1. Install Rust (1.80+)
2. Copy `.env.example` to `.env` and configure
3. Run:
   ```bash
   cargo run
   ```

## Data Persistence

The SQLite database is stored in `./data/gym_tracker.db`. The data directory is mounted as a volume in Docker to persist data across container restarts.

All gym tracker tables are prefixed with `gym_` to allow multiple trackers to coexist.

## Activity Types

Default activity types include:
- push, pull, legs, chest, shoulders, back
- cardio, upper, lower, full_body
- arms, core, hiit, yoga, stretching
- swimming, cycling, running, walking, sports

Admins can add or remove types with `/gym add_type` and `/gym remove_type`.

## Weekly Cycle

- Tracking periods run Sunday to Sunday
- At week end, the bot automatically:
  - Saves period results
  - Updates all-time totals
  - Posts a summary image to the configured channel
  - Creates a new period

## License

MIT
