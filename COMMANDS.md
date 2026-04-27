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
- **Next steps**: Add users with `/gym user add`, then `/gym start`

#### `/gym start`
Begin tracking workouts.

- **Permissions**: Administrator
- **What it does**:
  - Creates the first weekly period (Sunday to Sunday)
  - Auto-creates **Szn 1** and links the first period to it
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
| `amount` | integer | 1–100 | Default workouts per week |

- **Permissions**: Administrator
- **Note**: Only affects newly added users; existing users keep their current goal

#### `/gym config rollover <hour>`
Set the UTC hour on Sunday when the weekly rollover fires.

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `hour` | integer | 0–23 | UTC hour (e.g. 12 = Sunday noon UTC) |

- **Permissions**: Administrator
- **Note**: Takes effect on the **next** period created; does not change the current period's end time

#### `/gym period_info`
Show current period dates and time remaining until rollover.

#### `/gym set_period_end <end_time>`
Override when the current period ends.

| Parameter | Type | Description |
|-----------|------|-------------|
| `end_time` | string | RFC3339 datetime (e.g. `2024-01-08T00:00:00+00:00`) or `now` |

- **Permissions**: Administrator

---

### User Management

#### `/gym user add <user>`
Add a user to the gym tracker.

| Parameter | Type | Description |
|-----------|------|-------------|
| `user` | @mention | User to add |

- **Permissions**: Administrator
- **What it does**:
  - Registers user for tracking
  - Sets their goal to the guild default
  - Initializes their totals to zero

#### `/gym user remove <user>`
Remove a user from the tracker.

| Parameter | Type | Description |
|-----------|------|-------------|
| `user` | @mention | User to remove |

- **Permissions**: Administrator
- **Note**: Historical log data is preserved

#### `/gym user list`
Show all tracked users.

#### `/gym user import <user> <json>`
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

#### `/gym user set_type_total <user> <activity_type> <count>`
Manually set a user's all-time count for a specific type.

| Parameter | Type | Description |
|-----------|------|-------------|
| `user` | @mention | Target user |
| `activity_type` | string | Activity type (autocomplete) |
| `count` | integer | Total count to set |

- **Permissions**: Administrator

#### `/gym user set_goal_stats <user> <achieved> <missed>`
Manually set a user's goal statistics.

| Parameter | Type | Description |
|-----------|------|-------------|
| `user` | @mention | Target user |
| `achieved` | integer | Goals achieved count |
| `missed` | integer | Goals missed count |

- **Permissions**: Administrator

---

### Activity Types

#### `/gym add_type <group> <name>`
Add a new activity type, assigned to a group.

| Parameter | Type | Constraints | Description |
|-----------|------|-------------|-------------|
| `group` | string | autocomplete | Activity group to assign this type to |
| `name` | string | max 32 chars | Activity name |

- **Permissions**: Administrator
- **Normalization**: Converted to lowercase, spaces become underscores
- **Example**: `/gym add_type lift "Olympic Lifting"` → `olympic_lifting` in the `lift` group

#### `/gym remove_type <name>`
Remove an activity type.

| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | string | Type to remove (autocomplete) |

- **Permissions**: Administrator
- **Note**: Existing logs with this type are preserved

#### `/gym list_types`
Show all configured activity types, grouped by their activity group.

---

### Activity Groups

Groups let you bundle types together for group-level goals (e.g. a "gym" group containing push/pull/legs).

#### `/gym group create <name>`
Create a new activity group.

| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | string | Group name |

- **Permissions**: Administrator

#### `/gym group delete <name>`
Delete a group (does not affect logs or types).

- **Permissions**: Administrator

#### `/gym group list`
Show all groups and the types assigned to each.

#### `/gym group assign <group> <type>`
Assign an activity type to a group.

| Parameter | Type | Description |
|-----------|------|-------------|
| `group` | string | Group name (autocomplete) |
| `type` | string | Activity type (autocomplete) |

- **Permissions**: Administrator
- **Note**: Each type can only belong to one group

#### `/gym group unassign <type>`
Remove a type from its group.

| Parameter | Type | Description |
|-----------|------|-------------|
| `type` | string | Activity type (autocomplete) |

- **Permissions**: Administrator

---

### Logging Workouts

#### `/gym log <group> <activity_type> [user2] [user3] [image]`
Log a workout.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `group` | string | Yes | Activity group (autocomplete) |
| `activity_type` | string | Yes | Type of workout (autocomplete, filtered by group) |
| `user2` | @mention | No | Additional user to log for |
| `user3` | @mention | No | Another additional user |
| `image` | attachment | No | Proof photo |

- **Who can use**: Any tracked user
- **Examples**:
  - `/gym log lift push` — log push workout for yourself
  - `/gym log cardio run @friend1 @friend2` — log for yourself and 2 others
  - `/gym log lift legs [attach image]` — log with a gym photo (shown as [📷](url))

#### `/gym log_past <group> <activity_type> <weeks_ago>`
Retroactively log a workout for a past period.

| Parameter | Type | Description |
|-----------|------|-------------|
| `group` | string | Activity group (autocomplete) |
| `activity_type` | string | Type (autocomplete, filtered by group) |
| `weeks_ago` | integer | 1–4 weeks back |

---

### Goals

Goals are always **additive AND constraints**:
- `total_goal` is the floor — you must always hit it
- Type goals and group goals are extra requirements on top
- Example: `total=5 + push≥3 + cardio≥1` means 5+ workouts, at least 3 push and at least 1 cardio

#### `/gym goal total <count>`
Set your total weekly goal.

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `count` | integer | 1–100 | Workouts per week |

#### `/gym goal by_type <activity_type> <count>`
Add a minimum requirement for a specific activity type.

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `activity_type` | string | — | Type (autocomplete) |
| `count` | integer | 1–50 | Required count |

- **Additive**: stacks with your total goal — both must be met

#### `/gym goal by_group <group> <count>`
Add a minimum requirement for an activity group.

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `group` | string | — | Group name (autocomplete) |
| `count` | integer | 1–50 | Required count |

- **Additive**: stacks with total and type goals

#### `/gym goal view`
Show all your active goal constraints.

- **Shows**: Total goal (required) + any type requirements (AND) + any group requirements (AND)

#### `/gym goal reset`
Reset to server default goal; clears all type and group requirements.

---

### Statistics

#### `/gym status`
Show your progress for the current week.

- **Shows**:
  - Current period dates
  - Total workouts logged
  - Goal progress (✓ if met)
  - Breakdown by activity type

#### `/gym summary`
Generate a weekly summary image for all users.

- **Output**: PNG — vertical cards, one per user, ~500px wide
  - Progress bar (green if goal met, orange if not)
  - Type chip grid (only shows types the user logged)

#### `/gym totals`
Generate the season stats image.

- **Output**: PNG showing season totals for all users
  - Ranked by total workouts (medals for top 3)
  - Per-type totals and goals achieved/missed

#### `/gym history [@user] [season]`
Show week-by-week history.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `user` | @mention | No | Show single-user detailed view (defaults to caller) |
| `season` | string | No | Season to show (autocomplete; defaults to current) |

- **No user specified**: Heatmap grid — all users × all completed periods in the season, each cell shows count + ✓/✗
- **User specified**: Detailed image table — one row per week, with type breakdown and inline goal-change annotations

---

### Leave of Absence

A user can request a leave of absence (LOA) — e.g. for vacation or illness. If approved by the server, their weekly goal tracking is paused for the specified window. They can still log workouts (which count toward all-time totals), but missed-goal weeks during the LOA don't count against them. LOA weeks appear as **LOA** (blue) in history instead of ✓/✗.

#### `/gym loa request <weeks> [start_date] [mention_role]`
Submit an LOA request for a community vote.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `weeks` | integer (1–12) | Yes | How many weeks the leave covers |
| `start_date` | string (YYYY-MM-DD) | No | When the LOA begins; defaults to today |
| `mention_role` | @role | No | Role to @mention in the vote post |

- The bot posts a vote message in the tracker channel with ✅/❌ reactions
- Any server member can vote except the requester (their votes are excluded from the tally)
- Voting closes after **24 hours**
- **Approval**: more ✅ than ❌; or no votes at all (auto-approved)
- **Denial**: more ❌ than ✅
- The LOA window is stored immediately — if `start_date` is in the future, the exemption activates then

---

### Seasons

Seasons group periods together for scoped stats and history.

#### `/gym season new`
End the current season and start the next one.

- **Permissions**: Administrator
- **What it does**:
  - Closes the current season
  - Creates a new season (auto-increments: Szn 1 → Szn 2 → …)
  - Links future periods to the new season

#### `/gym season end`
End the current season without starting a new one.

- **Permissions**: Administrator

#### `/gym season list`
List all seasons with start dates and status.

---

### Admin / Debug

#### `/gym force_rollover`
Manually trigger a weekly rollover immediately.

- **Permissions**: Administrator
- **Use case**: Testing, fixing a missed rollover, or starting a new period early
- **Behavior**: Identical to an automatic rollover — archives results, posts summary image, creates next period

---

## Automatic Features

### Weekly Rollover
On startup (and after each rollover), the bot calculates exactly when the next period ends and sleeps until then. On rollover it:

1. Archives results to `gym_period_results` and `gym_period_type_counts`
2. Updates all-time totals and goal statistics
3. Posts weekly summary image to the configured channel
4. Posts season stats image (if a season is active)
5. Creates the next period

### Rollover Timing
- Default: **Sunday 12:00 UTC**
- Configurable per guild via `/gym config rollover <hour>`
- The bot stores the exact end time in the DB — restarts recalculate sleep from the stored end time, so missed rollovers fire immediately on next startup

### Reaction Tracking
🔥 reactions on log posts are stored in `gym_log_reactions` for use in end-of-season recap features.

---

## Permission Summary

| Command | Permission Required |
|---------|---------------------|
| `/gym setup` | Administrator |
| `/gym start` | Administrator |
| `/gym stop` | Administrator |
| `/gym config goal` | Administrator |
| `/gym config rollover` | Administrator |
| `/gym set_period_end` | Administrator |
| `/gym user add` | Administrator |
| `/gym user remove` | Administrator |
| `/gym add_type` | Administrator |
| `/gym remove_type` | Administrator |
| `/gym group create` | Administrator |
| `/gym group delete` | Administrator |
| `/gym group assign` | Administrator |
| `/gym group unassign` | Administrator |
| `/gym user import` | Administrator |
| `/gym user set_type_total` | Administrator |
| `/gym user set_goal_stats` | Administrator |
| `/gym season new` | Administrator |
| `/gym season end` | Administrator |
| `/gym force_rollover` | Administrator |
| `/gym info` | Anyone |
| `/gym period_info` | Anyone |
| `/gym user list` | Anyone |
| `/gym list_types` | Anyone |
| `/gym group list` | Anyone |
| `/gym season list` | Anyone |
| `/gym log` | Tracked users |
| `/gym log_past` | Tracked users |
| `/gym goal *` | Tracked users |
| `/gym loa request` | Tracked users |
| `/gym status` | Tracked users |
| `/gym summary` | Anyone |
| `/gym totals` | Anyone |
| `/gym history` | Anyone |
