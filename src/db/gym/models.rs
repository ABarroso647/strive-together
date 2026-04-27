// Gym tracker data models


#[derive(Debug, Clone)]
pub struct GuildConfig {
    pub guild_id: u64,
    pub channel_id: u64,
    pub default_goal: i32,
    pub started: bool,
    /// Hour of day (0–23 UTC) on Sunday when the week rolls over
    pub rollover_hour: u32,
}

#[derive(Debug, Clone)]
pub struct Period {
    pub id: i64,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Clone)]
pub struct UserGoalConfig {
    pub total_goal: i32,
}

#[derive(Debug, Clone)]
pub struct Season {
    pub id: i64,
    pub name: String,       // "Szn 1", "Szn 2", …
    pub start_time: String,
    pub end_time: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct LoaRequest {
    pub id: i64,
    pub guild_id: u64,
    pub user_id: u64,
    pub weeks: i64,
    pub vote_message_id: Option<u64>,
    pub vote_channel_id: u64,
    pub loa_start: Option<String>,
    pub loa_end: Option<String>,
    pub mention_role_id: Option<u64>,
}
