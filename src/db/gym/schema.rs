// Gym tracker database schema
// All tables prefixed with gym_ to allow multiple trackers

pub const SCHEMA: &str = r#"
-- Guild configuration for gym tracker
CREATE TABLE IF NOT EXISTS gym_guild_config (
    guild_id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL,
    default_goal INTEGER NOT NULL DEFAULT 5,
    started INTEGER NOT NULL DEFAULT 0,
    rollover_hour INTEGER NOT NULL DEFAULT 12
);

-- Seasons (for grouping periods, future feature)
CREATE TABLE IF NOT EXISTS gym_seasons (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    start_time TEXT NOT NULL,
    end_time TEXT,
    is_current INTEGER NOT NULL DEFAULT 1
);

-- Activity types per guild
CREATE TABLE IF NOT EXISTS gym_activity_types (
    guild_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (guild_id, name)
);

-- Users in the tracker
CREATE TABLE IF NOT EXISTS gym_users (
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    PRIMARY KEY (guild_id, user_id)
);

-- User goal configuration
CREATE TABLE IF NOT EXISTS gym_user_goal_config (
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    goal_mode TEXT NOT NULL DEFAULT 'total',
    total_goal INTEGER NOT NULL DEFAULT 5,
    PRIMARY KEY (guild_id, user_id)
);

-- User type-specific goals (for by_type mode)
CREATE TABLE IF NOT EXISTS gym_user_type_goals (
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    activity_type TEXT NOT NULL,
    goal INTEGER NOT NULL,
    PRIMARY KEY (guild_id, user_id, activity_type)
);

-- Weekly periods
CREATE TABLE IF NOT EXISTS gym_periods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    season_id INTEGER,
    week_number INTEGER,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    is_current INTEGER NOT NULL DEFAULT 0
);

-- Individual workout logs
CREATE TABLE IF NOT EXISTS gym_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    period_id INTEGER NOT NULL,
    activity_type TEXT NOT NULL,
    logged_at TEXT NOT NULL
);

-- Period results (archived after period ends)
CREATE TABLE IF NOT EXISTS gym_period_results (
    period_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    total_count INTEGER NOT NULL DEFAULT 0,
    goal_met INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (period_id, user_id)
);

-- Period type counts (archived)
CREATE TABLE IF NOT EXISTS gym_period_type_counts (
    period_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    activity_type TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (period_id, user_id, activity_type)
);

-- All-time user totals
CREATE TABLE IF NOT EXISTS gym_user_totals (
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    total_count INTEGER NOT NULL DEFAULT 0,
    achieved_goals INTEGER NOT NULL DEFAULT 0,
    missed_goals INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (guild_id, user_id)
);

-- All-time user type totals
CREATE TABLE IF NOT EXISTS gym_user_type_totals (
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    activity_type TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (guild_id, user_id, activity_type)
);

-- Activity groups (e.g. "gym", "cardio", "wellness")
CREATE TABLE IF NOT EXISTS gym_activity_groups (
    guild_id INTEGER NOT NULL,
    group_name TEXT NOT NULL,
    PRIMARY KEY (guild_id, group_name)
);

-- Maps each activity type to a group (one group per type)
CREATE TABLE IF NOT EXISTS gym_type_group_map (
    guild_id INTEGER NOT NULL,
    activity_type TEXT NOT NULL,
    group_name TEXT NOT NULL,
    PRIMARY KEY (guild_id, activity_type),
    FOREIGN KEY (guild_id, group_name) REFERENCES gym_activity_groups(guild_id, group_name) ON DELETE CASCADE
);

-- User group-based goals (for by_group mode)
CREATE TABLE IF NOT EXISTS gym_user_group_goals (
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    group_name TEXT NOT NULL,
    goal INTEGER NOT NULL,
    PRIMARY KEY (guild_id, user_id, group_name)
);

-- Discord message IDs for log posts (for reactions + end-of-season highlights)
CREATE TABLE IF NOT EXISTS gym_log_messages (
    message_id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    period_id INTEGER NOT NULL
);

-- Image attachment URLs uploaded alongside log commands
CREATE TABLE IF NOT EXISTS gym_log_attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    filename TEXT NOT NULL,
    uploaded_at TEXT NOT NULL
);

-- 🔥 reactions tracked on log posts
CREATE TABLE IF NOT EXISTS gym_log_reactions (
    message_id INTEGER NOT NULL,
    reactor_user_id INTEGER NOT NULL,
    reacted_at TEXT NOT NULL,
    PRIMARY KEY (message_id, reactor_user_id)
);

-- Goal change audit log (shown in per-user history)
CREATE TABLE IF NOT EXISTS gym_goal_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    changed_at TEXT NOT NULL,
    description TEXT NOT NULL
);

-- Leave of absence requests (community vote)
CREATE TABLE IF NOT EXISTS gym_loa_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    requested_at TEXT NOT NULL,
    weeks INTEGER NOT NULL,
    vote_message_id INTEGER,
    vote_channel_id INTEGER NOT NULL,
    vote_ends_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    loa_start TEXT,
    loa_end TEXT,
    mention_role_id INTEGER
);
CREATE INDEX IF NOT EXISTS idx_gym_loa_requests_status ON gym_loa_requests(status, vote_ends_at);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_gym_logs_period_user ON gym_logs(period_id, user_id);
CREATE INDEX IF NOT EXISTS idx_gym_logs_guild_user ON gym_logs(guild_id, user_id);
CREATE INDEX IF NOT EXISTS idx_gym_periods_guild_current ON gym_periods(guild_id, is_current);
CREATE INDEX IF NOT EXISTS idx_gym_log_messages_guild ON gym_log_messages(guild_id);
CREATE INDEX IF NOT EXISTS idx_gym_goal_history_user ON gym_goal_history(guild_id, user_id, changed_at);
"#;

/// Migrations applied after the main schema — each statement is run independently
/// and errors are silently ignored (handles already-applied migrations on existing DBs).
pub const MIGRATIONS: &[&str] = &[
    "ALTER TABLE gym_guild_config ADD COLUMN rollover_hour INTEGER NOT NULL DEFAULT 12",
    "ALTER TABLE gym_period_results ADD COLUMN loa_exempt INTEGER NOT NULL DEFAULT 0",
    "CREATE TABLE IF NOT EXISTS gym_loa_requests (id INTEGER PRIMARY KEY AUTOINCREMENT, guild_id INTEGER NOT NULL, user_id INTEGER NOT NULL, requested_at TEXT NOT NULL, weeks INTEGER NOT NULL, vote_message_id INTEGER, vote_channel_id INTEGER NOT NULL, vote_ends_at TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending', loa_start TEXT, loa_end TEXT)",
    "ALTER TABLE gym_loa_requests ADD COLUMN mention_role_id INTEGER",
    // Seed default groups for existing guilds
    "INSERT OR IGNORE INTO gym_activity_groups (guild_id, group_name) SELECT guild_id, 'lift' FROM gym_guild_config",
    "INSERT OR IGNORE INTO gym_activity_groups (guild_id, group_name) SELECT guild_id, 'cardio' FROM gym_guild_config",
    "INSERT OR IGNORE INTO gym_type_group_map (guild_id, activity_type, group_name) SELECT guild_id, name, 'lift' FROM gym_activity_types WHERE name IN ('push', 'pull', 'legs', 'upper', 'lower')",
    "INSERT OR IGNORE INTO gym_type_group_map (guild_id, activity_type, group_name) SELECT guild_id, name, 'cardio' FROM gym_activity_types WHERE name IN ('run', 'bike', 'machine_cardio', 'hiit')",
];

pub const DEFAULT_ACTIVITY_TYPES: &[&str] = &[
    "push", "pull", "legs", "upper", "lower",
    "run", "bike", "machine_cardio", "hiit",
];

pub const DEFAULT_ACTIVITY_GROUPS: &[(&str, &[&str])] = &[
    ("lift", &["push", "pull", "legs", "upper", "lower"]),
    ("cardio", &["run", "bike", "machine_cardio", "hiit"]),
];
