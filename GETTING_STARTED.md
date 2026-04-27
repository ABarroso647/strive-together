# Getting Started

This guide walks you through setting up and running the Strive Together bot locally.

## Prerequisites

- **Rust 1.80+** - Install from https://rustup.rs
- **A Discord account** - To create a bot and test server
- **A test Discord server** - Where you have admin permissions

## Step 1: Create a Discord Bot

1. Go to the [Discord Developer Portal](https://discord.com/developers/applications)

2. Click **"New Application"**
   - Name it something like "Strive Together Dev"
   - Click "Create"

3. Go to the **"Bot"** tab (left sidebar)
   - Click **"Reset Token"** and copy the token
   - **Save this token** - you'll need it in Step 2
   - Scroll down to **"Privileged Gateway Intents"**
   - Enable **Server Members Intent** ✓
   - Click "Save Changes"

4. Go to **"OAuth2" → "URL Generator"** (left sidebar)
   - Under "Scopes", check:
     - `bot`
     - `applications.commands`
   - Under "Bot Permissions", check:
     - Send Messages
     - Attach Files
     - Use Slash Commands
     - Add Reactions
     - Read Message History
   - Copy the generated URL at the bottom

5. Open that URL in your browser
   - Select your test server
   - Click "Authorize"

## Step 2: Configure the Bot

```bash
cp .env.example .env
```

Edit `.env` and add your bot token:
```
DISCORD_TOKEN=paste_your_token_here
```

## Step 3: Run the Bot

```bash
# If you just installed Rust, load it into your shell
source ~/.cargo/env

# Run the bot
cargo run
```

First run will take a few minutes to compile dependencies. You should see:
```
INFO gym_tracker_bot: Database initialized at data/gym_tracker.db
INFO gym_tracker_bot: Bot is ready! Registering commands...
INFO gym_tracker_bot: Commands registered to 1 guild(s)
INFO gym_tracker_bot: Background tasks started!
INFO gym_tracker_bot: Starting bot...
```

The bot is now online!

## Step 4: Test the Bot

Open Discord and go to your test server. Try these commands in order:

### Initial Setup (Admin)
```
/gym setup                   → Initializes the tracker in the current channel
/gym user add @YourName      → Adds yourself to the tracker
/gym start                   → Begins the first tracking week
```

### Log Some Workouts
```
/gym log lift push           → Log a push workout
/gym log cardio run          → Log a run
/gym log lift legs           → Log a legs workout
```

### Check Your Progress
```
/gym status             → See your weekly progress
/gym summary            → Generate a summary image
/gym totals             → See the leaderboard
```

### Customize Your Goals
```
/gym goal view              → See your current goal settings
/gym goal total 3           → Change goal to 3 workouts/week
/gym goal by_type push 2    → Require 2 push workouts specifically
/gym goal by_group lift 3   → Require 3 lift workouts specifically
```

## Common Issues

### "DISCORD_TOKEN must be set"
Your `.env` file is missing or doesn't have the token. Make sure:
- `.env` exists in the `discord-bot` directory
- It contains `DISCORD_TOKEN=your_actual_token`
- No quotes around the token

### Commands don't appear in Discord
- Commands register instantly to each guild on startup
- Try restarting the bot
- Make sure the bot has `applications.commands` scope

### "Gym tracker not set up"
Run `/gym setup` in the channel first.

### "You're not in the gym tracker"
An admin needs to run `/gym user add @you` to add you.

### Bot shows online but doesn't respond
- Check terminal for error messages
- Verify the bot has "Send Messages" permission in that channel
- Make sure you're using slash commands (start with `/`)

## Running with Docker

If you prefer Docker:

```bash
# Build and run
docker compose up -d

# View logs
docker compose logs -f

# Stop
docker compose down
```

## Next Steps

- Read [COMMANDS.md](COMMANDS.md) for a full command reference
- Read [CONTRIBUTING.md](CONTRIBUTING.md) to understand the codebase
- Check out the `src/` directory to see how it all works
