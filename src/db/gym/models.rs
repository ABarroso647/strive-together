// Gym tracker data models

#[derive(Debug, Clone)]
pub struct GuildConfig {
    pub guild_id: u64,
    pub channel_id: u64,
    pub default_goal: i32,
    pub started: bool,
}

#[derive(Debug, Clone)]
pub struct Period {
    pub id: i64,
    pub guild_id: u64,
    pub season_id: Option<i64>,
    pub week_number: Option<i32>,
    pub start_time: String,
    pub end_time: String,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct UserGoalConfig {
    pub guild_id: u64,
    pub user_id: u64,
    pub goal_mode: GoalMode,
    pub total_goal: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GoalMode {
    Total,
    ByType,
}

impl GoalMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "by_type" => GoalMode::ByType,
            _ => GoalMode::Total,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            GoalMode::Total => "total",
            GoalMode::ByType => "by_type",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub id: i64,
    pub guild_id: u64,
    pub user_id: u64,
    pub period_id: i64,
    pub activity_type: String,
    pub logged_at: String,
}

#[derive(Debug, Clone)]
pub struct UserTotals {
    pub guild_id: u64,
    pub user_id: u64,
    pub total_count: i32,
    pub achieved_goals: i32,
    pub missed_goals: i32,
}

#[derive(Debug, Clone)]
pub struct PeriodResult {
    pub period_id: i64,
    pub user_id: u64,
    pub total_count: i32,
    pub goal_met: bool,
}
