# Strive Together Bot

A Discord bot for tracking fitness goals with weekly accountability. Currently supports gym workout tracking, with an extensible architecture for future trackers.

## Documentation

- **[Getting Started](GETTING_STARTED.md)** - Set up and run the bot locally
- **[Commands](COMMANDS.md)** - Full command reference
- **[Contributing](CONTRIBUTING.md)** - Understand the codebase and add features

## Features

### Gym Tracker
- **Workout Logging**: Log workouts with activity types, optional image proof, multi-user logging
- **Flexible Goals**: Total weekly goal + optional additive type/group requirements (all must be met)
- **Activity Groups**: Group types (e.g. push/pull/legs → "gym group") and set group-level goals
- **Weekly Summaries**: Auto-generated summary image posted at week rollover
- **Season Stats**: Season-scoped totals image with per-user breakdown
- **History**: Heatmap overview for all users; per-user table with week-by-week breakdown and goal changes
- **Seasons**: Named seasons (Szn 1, Szn 2, …); history is scoped per season
- **Reaction Tracking**: 🔥 reactions on log posts are recorded for end-of-season highlights
- **Image Storage**: Workout image URLs stored for end-of-season recap
- **Leave of Absence**: Community-voted LOA requests pause goal tracking while still counting logs

## Commands

All gym tracker commands are under the `/gym` parent:

### Admin
| Command | Description |
|---|---|
| `/gym setup` | Initialize tracker in this channel |
| `/gym start` | Start tracking — creates first period and Szn 1 |
| `/gym stop` | Pause tracking |
| `/gym info` | Show config, users, current period |
| `/gym config goal <n>` | Set default weekly goal for new users |
| `/gym config rollover <hour>` | Set the UTC hour on Sunday when the week rolls over (0–23) |
| `/gym set_period_end <time\|now>` | Shift when the current week ends |
| `/gym add_user @user` | Add a user |
| `/gym remove_user @user` | Remove a user |
| `/gym list_users` | List tracked users |
| `/gym import_user @user <json>` | Bulk-import historical data |
| `/gym set_type_total @user <type> <n>` | Manually set a type total |
| `/gym set_goal_stats @user <met> <missed>` | Manually set goal stats |
| `/gym add_type <name>` | Add an activity type |
| `/gym remove_type <name>` | Remove an activity type |
| `/gym list_types` | List activity types |
| `/gym group create <name>` | Create an activity group |
| `/gym group delete <name>` | Delete a group |
| `/gym group list` | Show groups and their assigned types |
| `/gym group assign <group> <type>` | Assign a type to a group |
| `/gym group unassign <type>` | Remove a type from its group |
| `/gym season new` | End current season and start the next |
| `/gym season end` | End the current season without starting a new one |
| `/gym season list` | List all seasons |
| `/gym force_rollover` | Manually trigger a weekly rollover (dev/admin) |

### User
| Command | Description |
|---|---|
| `/gym log <type> [user2] [user3] [image]` | Log a workout |
| `/gym log_past <type> <weeks_ago>` | Retroactively log for a past week |
| `/gym status` | Your current week progress embed |
| `/gym summary` | Weekly summary image for all users |
| `/gym totals` | Season stats image |
| `/gym history [@user] [season]` | Heatmap overview or per-user table |
| `/gym goal total <n>` | Set your total weekly goal |
| `/gym goal by_type <type> <n>` | Add a type requirement (additive) |
| `/gym goal by_group <group> <n>` | Add a group requirement (additive) |
| `/gym goal view` | View all your active goal constraints |
| `/gym goal reset` | Reset to server default, clear extra requirements |
| `/gym loa request <weeks> [start_date] [mention_role]` | Request a leave of absence (community vote) |

## Goal System

Goals are always **additive AND constraints**:
- `total_goal` is the floor — you must always hit it
- Type goals and group goals are extra requirements on top
- Example: `total=5 + push≥3 + cardio≥1` means you need 5+ workouts, at least 3 of which are push, at least 1 cardio

## Architecture

```
src/
├── main.rs                    # Entry point, event handler (🔥 reactions)
├── commands/gym/
│   ├── setup.rs               # setup, start, stop, info, config, period_info, set_period_end
│   ├── users.rs               # add/remove/list/import user, set_type_total, set_goal_stats
│   ├── types.rs               # add_type, remove_type, list_types
│   ├── groups.rs              # group create/delete/list/assign/unassign
│   ├── goals.rs               # goal total/by_type/by_group/view/reset
│   ├── log.rs                 # log, log_past
│   ├── stats.rs               # status, summary, totals, history
│   ├── season.rs              # season new/end/list
│   ├── loa.rs                 # loa request (leave of absence)
│   └── debug.rs               # force_rollover
├── db/gym/
│   ├── schema.rs              # All gym_ tables + indexes
│   ├── models.rs              # GuildConfig, Period, UserGoalConfig, Season
│   └── queries.rs             # All DB query functions
├── images/gym/
│   ├── summary.rs             # Weekly per-user card layout (progress bar + type grid)
│   ├── season.rs              # Season stats table image
│   └── history.rs             # Heatmap overview + per-user history table
├── tasks/gym/
│   ├── weekly_check.rs        # Smart-sleep rollover task, posts summary + season images
│   └── loa_check.rs           # Smart-sleep LOA vote resolution task
└── util/
    └── time.rs                # format_datetime, parse_datetime, get_weekly_period_bounds_with_hour
```

## Setup

### Prerequisites
- Docker and Docker Compose, OR Rust 1.80+
- Discord Bot Token with Server Members Intent + Message Content Intent enabled

### Discord Bot Permissions
Scopes: `bot`, `applications.commands`
Permissions: `Send Messages`, `Attach Files`, `Use Slash Commands`, `Add Reactions`, `Read Message History`

### Running

**Option 1 — Docker (recommended)**

Builds from source and runs with dev mode enabled:
```bash
cp .env.example .env
# Fill in DISCORD_TOKEN

docker compose up -d
```

**Option 2 — Pre-built image**

No build step — pulls the latest published image:
```bash
cp .env.example .env
# Fill in DISCORD_TOKEN

docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d
```

**Option 3 — Run locally**
```bash
cp .env.example .env
cargo run
```

`ENVIRONMENT=development` (set in `.env` or the compose file) registers slash commands to your dev guild instantly. Without it, commands register globally and can take up to an hour to propagate.

## Data

SQLite at `./data/gym_tracker.db` (volume-mounted in Docker). All tables prefixed `gym_`.

Key tables: `gym_logs`, `gym_periods`, `gym_period_results`, `gym_period_type_counts`, `gym_seasons`, `gym_user_goal_config`, `gym_user_type_goals`, `gym_user_group_goals`, `gym_goal_history`, `gym_log_messages`, `gym_log_attachments`, `gym_log_reactions`

## Weekly Cycle

Periods run Sunday→Sunday. On startup and after each rollover, the bot calculates exactly when the next period ends and sleeps until then (plus a 30-second buffer). On rollover it:
1. Archives results to `gym_period_results` and `gym_period_type_counts`
2. Updates `gym_user_totals` and `gym_user_type_totals`
3. Posts weekly summary image to the configured channel
4. Posts season stats image (if a season is active)
5. Creates the next period

## License

MIT
