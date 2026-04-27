# Command Reference

Complete reference for all bot commands.

---

## Gym Tracker (`/gym`)

All gym tracker commands are subcommands of `/gym`.

### Setup & Configuration

#### `/gym setup`
Initialize the gym tracker in the current channel.

- **Permissions**: Administrator
- **What it does**:
  - Creates guild configuration
  - Sets the current channel as the announcement channel
  - Adds default activity types (push, pull, legs, cardio, etc.)
- **Next steps**: Add users with `/gym add_user`, then `/gym start`

#### `/gym start`
Begin tracking workouts.

- **Permissions**: Administrator
- **What it does**:
  - Creates the first weekly period (Sunday to Sunday)
  - Enables workout logging
- **Prerequisite**: Must run `/gym setup` first

#### `/gym stop`
Pause tracking.

- **Permissions**: Administrator
- **What it does**:
  - Disables workout logging
  - Preserves all data
- **Resume**: Run `/gym start` again

#### `/gym info`
Display current configuration.

- **Shows**:
  - Announcement channel
  - Tracking status (active/stopped)
  - Default goal
  - Current period dates
  - Number of users and activity types

#### `/gym config goal <amount>`
Set the default weekly goal for new users.

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `amount` | integer | 1-100 | Default workouts per week |

- **Permissions**: Administrator
- **Note**: Only affects newly added users

---

### User Management

#### `/gym add_user <user>`
Add a user to the gym tracker.

| Parameter | Type | Description |
|-----------|------|-------------|
| `user` | @mention | User to add |

- **Permissions**: Administrator
- **What it does**:
  - Registers user for tracking
  - Sets their goal to the guild default
  - Initializes their totals to zero

#### `/gym remove_user <user>`
Remove a user from the tracker.

| Parameter | Type | Description |
|-----------|------|-------------|
| `user` | @mention | User to remove |

- **Permissions**: Administrator
- **Warning**: Does not delete historical log data

#### `/gym list_users`
Show all tracked users.

- **Output**: Embed with user mentions and total count

#### `/gym import_user <user> <json>`
Import historical data for a user.

| Parameter | Type | Description |
|-----------|------|-------------|
| `user` | @mention | User to import for |
| `json` | string | JSON data |

- **Permissions**: Administrator
- **JSON format**:
```json
{
  "total_count": 150,
  "achieved_goals": 20,
  "missed_goals": 5,
  "type_totals": {
    "push": 40,
    "pull": 35,
    "cardio": 30
  }
}
```

#### `/gym set_type_total <user> <activity_type> <count>`
Manually set a user's all-time count for a specific type.

| Parameter | Type | Description |
|-----------|------|-------------|
| `user` | @mention | Target user |
| `activity_type` | string | Activity type (autocomplete) |
| `count` | integer | Total count to set |

- **Permissions**: Administrator

#### `/gym set_goal_stats <user> <achieved> <missed>`
Manually set a user's goal statistics.

| Parameter | Type | Description |
|-----------|------|-------------|
| `user` | @mention | Target user |
| `achieved` | integer | Goals achieved count |
| `missed` | integer | Goals missed count |

- **Permissions**: Administrator

---

### Activity Types

#### `/gym add_type <name>`
Add a new activity type.

| Parameter | Type | Constraints | Description |
|-----------|------|-------------|-------------|
| `name` | string | max 32 chars, no spaces | Activity name |

- **Permissions**: Administrator
- **Normalization**: Converted to lowercase, spaces become underscores
- **Example**: `/gym add_type "Olympic Lifting"` → `olympic_lifting`

#### `/gym remove_type <name>`
Remove an activity type.

| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | string | Type to remove (autocomplete) |

- **Permissions**: Administrator
- **Note**: Existing logs with this type are preserved

#### `/gym list_types`
Show all configured activity types.

- **Default types**: push, pull, legs, chest, shoulders, back, cardio, upper, lower, full_body, arms, core, hiit, yoga, stretching, swimming, cycling, running, walking, sports

---

### Logging Workouts

#### `/gym log <activity_type> [user2] [user3] [image]`
Log a workout.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `activity_type` | string | Yes | Type of workout (autocomplete) |
| `user2` | @mention | No | Additional user to log for |
| `user3` | @mention | No | Another additional user |
| `image` | attachment | No | Proof photo |

- **Who can use**: Any tracked user
- **Examples**:
  - `/gym log push` - Log push workout for yourself
  - `/gym log cardio @friend1 @friend2` - Log for yourself and 2 friends
  - `/gym log legs [attach image]` - Log with a gym photo

---

### Goals

#### `/gym goal total <count>`
Set your weekly goal to a total number of workouts.

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `count` | integer | 1-100 | Workouts per week |

- **Mode**: Switches you to "total" mode
- **Example**: Goal of 5 means any 5 workouts meet the goal

#### `/gym goal by_type <activity_type> <count>`
Set a goal for a specific activity type.

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `activity_type` | string | - | Type (autocomplete) |
| `count` | integer | 1-50 | Required count |

- **Mode**: Switches you to "by_type" mode
- **Note**: In by_type mode, you must meet ALL type goals
- **Example**: 
  - `/gym goal by_type push 2`
  - `/gym goal by_type cardio 3`
  - Now you need 2 push AND 3 cardio to meet your goal

#### `/gym goal view`
Show your current goal settings.

- **Shows**: Mode (total/by_type), goal values

---

### Statistics

#### `/gym status`
Show your progress for the current week.

- **Shows**:
  - Current period dates
  - Total workouts logged
  - Goal progress (with checkmark if met)
  - Breakdown by activity type

#### `/gym summary`
Generate a weekly summary image for all users.

- **Output**: PNG image showing:
  - All users in a table
  - Per-type breakdown columns
  - Total and goal status
  - Green/orange color coding

#### `/gym totals`
Generate the all-time leaderboard image.

- **Output**: PNG image showing:
  - Users ranked by total workouts
  - Medals for top 3
  - Per-type totals
  - Goals achieved/missed

#### `/gym history [user]`
Show week-by-week history.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `user` | @mention | No | User to show (defaults to you) |

- **Shows**: Last 10 completed weeks with:
  - Week start date
  - Total workouts
  - Goal met (✅) or missed (❌)

---

## Automatic Features

### Weekly Rollover
Every hour, the bot checks if the current week has ended. When it does:

1. Saves period results for all users
2. Updates all-time totals and goal statistics
3. Posts a summary image to the configured channel
4. Creates a new period for the next week

### Week Boundaries
- Weeks run **Sunday 00:00 UTC to Sunday 00:00 UTC**
- The first period starts when you run `/gym start`

---

## Permission Summary

| Command | Permission Required |
|---------|---------------------|
| `/gym setup` | Administrator |
| `/gym start` | Administrator |
| `/gym stop` | Administrator |
| `/gym config goal` | Administrator |
| `/gym add_user` | Administrator |
| `/gym remove_user` | Administrator |
| `/gym add_type` | Administrator |
| `/gym remove_type` | Administrator |
| `/gym import_user` | Administrator |
| `/gym set_type_total` | Administrator |
| `/gym set_goal_stats` | Administrator |
| `/gym info` | Anyone |
| `/gym list_users` | Anyone |
| `/gym list_types` | Anyone |
| `/gym log` | Tracked users |
| `/gym goal *` | Tracked users |
| `/gym status` | Tracked users |
| `/gym summary` | Anyone |
| `/gym totals` | Anyone |
| `/gym history` | Anyone |
