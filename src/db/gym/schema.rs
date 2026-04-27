// Gym tracker database schema
// All tables prefixed with gym_ to allow multiple trackers

pub const SCHEMA: &str = r#"
-- Guild configuration for gym tracker
CREATE TABLE IF NOT EXISTS gym_guild_config (
    guild_id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL,
    default_goal INTEGER NOT NULL DEFAULT 5,
    started INTEGER NOT NULL DEFAULT 0
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

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_gym_logs_period_user ON gym_logs(period_id, user_id);
CREATE INDEX IF NOT EXISTS idx_gym_logs_guild_user ON gym_logs(guild_id, user_id);
CREATE INDEX IF NOT EXISTS idx_gym_periods_guild_current ON gym_periods(guild_id, is_current);
"#;

pub const DEFAULT_ACTIVITY_TYPES: &[&str] = &[
    "push", "pull", "legs", "chest", "shoulders", "back",
    "cardio", "upper", "lower", "full_body",
    "arms", "core", "hiit", "yoga", "stretching",
    "swimming", "cycling", "running", "walking", "sports",
];
