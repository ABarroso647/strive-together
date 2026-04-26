// Gym tracker database queries
// All table names prefixed with gym_

use super::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

// ============================================================================
// Guild Config
// ============================================================================

pub fn get_guild_config(conn: &Connection, guild_id: u64) -> Result<Option<GuildConfig>, rusqlite::Error> {
    conn.query_row(
        "SELECT guild_id, channel_id, default_goal, started FROM gym_guild_config WHERE guild_id = ?",
        [guild_id],
        |row| {
            Ok(GuildConfig {
                guild_id: row.get(0)?,
                channel_id: row.get(1)?,
                default_goal: row.get(2)?,
                started: row.get::<_, i32>(3)? != 0,
            })
        },
    )
    .optional()
}

pub fn insert_guild_config(conn: &Connection, guild_id: u64, channel_id: u64) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_guild_config (guild_id, channel_id) VALUES (?, ?)",
        [guild_id, channel_id],
    )?;
    Ok(())
}

pub fn update_guild_started(conn: &Connection, guild_id: u64, started: bool) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_guild_config SET started = ? WHERE guild_id = ?",
        params![started as i32, guild_id],
    )?;
    Ok(())
}

pub fn update_default_goal(conn: &Connection, guild_id: u64, goal: i32) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_guild_config SET default_goal = ? WHERE guild_id = ?",
        params![goal, guild_id],
    )?;
    Ok(())
}

// ============================================================================
// Activity Types
// ============================================================================

pub fn get_activity_types(conn: &Connection, guild_id: u64) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT name FROM gym_activity_types WHERE guild_id = ? ORDER BY name")?;
    let types = stmt
        .query_map([guild_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(types)
}

pub fn insert_activity_type(conn: &Connection, guild_id: u64, name: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO gym_activity_types (guild_id, name) VALUES (?, ?)",
        params![guild_id, name],
    )?;
    Ok(())
}

pub fn delete_activity_type(conn: &Connection, guild_id: u64, name: &str) -> Result<bool, rusqlite::Error> {
    let rows = conn.execute(
        "DELETE FROM gym_activity_types WHERE guild_id = ? AND name = ?",
        params![guild_id, name],
    )?;
    Ok(rows > 0)
}

pub fn activity_type_exists(conn: &Connection, guild_id: u64, name: &str) -> Result<bool, rusqlite::Error> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM gym_activity_types WHERE guild_id = ? AND name = ?",
        params![guild_id, name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

// ============================================================================
// Users
// ============================================================================

pub fn get_users(conn: &Connection, guild_id: u64) -> Result<Vec<u64>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT user_id FROM gym_users WHERE guild_id = ?")?;
    let users = stmt
        .query_map([guild_id], |row| row.get(0))?
        .collect::<Result<Vec<u64>, _>>()?;
    Ok(users)
}

pub fn user_exists(conn: &Connection, guild_id: u64, user_id: u64) -> Result<bool, rusqlite::Error> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM gym_users WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn insert_user(conn: &Connection, guild_id: u64, user_id: u64, default_goal: i32) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO gym_users (guild_id, user_id) VALUES (?, ?)",
        params![guild_id, user_id],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO gym_user_goal_config (guild_id, user_id, total_goal) VALUES (?, ?, ?)",
        params![guild_id, user_id, default_goal],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO gym_user_totals (guild_id, user_id) VALUES (?, ?)",
        params![guild_id, user_id],
    )?;
    Ok(())
}

pub fn delete_user(conn: &Connection, guild_id: u64, user_id: u64) -> Result<bool, rusqlite::Error> {
    let rows = conn.execute(
        "DELETE FROM gym_users WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
    )?;
    // Also clean up related tables
    conn.execute(
        "DELETE FROM gym_user_goal_config WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
    )?;
    conn.execute(
        "DELETE FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
    )?;
    Ok(rows > 0)
}

// ============================================================================
// Periods
// ============================================================================

pub fn get_current_period(conn: &Connection, guild_id: u64) -> Result<Option<Period>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, guild_id, season_id, week_number, start_time, end_time, is_current
         FROM gym_periods WHERE guild_id = ? AND is_current = 1",
        [guild_id],
        |row| {
            Ok(Period {
                id: row.get(0)?,
                guild_id: row.get(1)?,
                season_id: row.get(2)?,
                week_number: row.get(3)?,
                start_time: row.get(4)?,
                end_time: row.get(5)?,
                is_current: row.get::<_, i32>(6)? != 0,
            })
        },
    )
    .optional()
}

pub fn insert_period(
    conn: &Connection,
    guild_id: u64,
    start_time: &str,
    end_time: &str,
) -> Result<i64, rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_periods (guild_id, start_time, end_time, is_current) VALUES (?, ?, ?, 1)",
        params![guild_id, start_time, end_time],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn close_current_period(conn: &Connection, guild_id: u64) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_periods SET is_current = 0 WHERE guild_id = ? AND is_current = 1",
        [guild_id],
    )?;
    Ok(())
}

// ============================================================================
// Logs
// ============================================================================

pub fn insert_log(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    period_id: i64,
    activity_type: &str,
    logged_at: &str,
) -> Result<i64, rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_logs (guild_id, user_id, period_id, activity_type, logged_at) VALUES (?, ?, ?, ?, ?)",
        params![guild_id, user_id, period_id, activity_type, logged_at],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_user_period_count(conn: &Connection, period_id: i64, user_id: u64) -> Result<i32, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM gym_logs WHERE period_id = ? AND user_id = ?",
        params![period_id, user_id],
        |row| row.get(0),
    )
}

pub fn get_user_period_type_counts(
    conn: &Connection,
    period_id: i64,
    user_id: u64,
) -> Result<HashMap<String, i32>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT activity_type, COUNT(*) FROM gym_logs WHERE period_id = ? AND user_id = ? GROUP BY activity_type",
    )?;
    let mut counts = HashMap::new();
    let rows = stmt.query_map(params![period_id, user_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
    })?;
    for row in rows {
        let (activity_type, count) = row?;
        counts.insert(activity_type, count);
    }
    Ok(counts)
}

// ============================================================================
// User Goals
// ============================================================================

pub fn get_user_goal_config(conn: &Connection, guild_id: u64, user_id: u64) -> Result<Option<UserGoalConfig>, rusqlite::Error> {
    conn.query_row(
        "SELECT guild_id, user_id, goal_mode, total_goal FROM gym_user_goal_config WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
        |row| {
            Ok(UserGoalConfig {
                guild_id: row.get(0)?,
                user_id: row.get(1)?,
                goal_mode: GoalMode::from_str(&row.get::<_, String>(2)?),
                total_goal: row.get(3)?,
            })
        },
    )
    .optional()
}

pub fn update_user_total_goal(conn: &Connection, guild_id: u64, user_id: u64, goal: i32) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_user_goal_config SET goal_mode = 'total', total_goal = ? WHERE guild_id = ? AND user_id = ?",
        params![goal, guild_id, user_id],
    )?;
    Ok(())
}

pub fn set_user_type_goal(conn: &Connection, guild_id: u64, user_id: u64, activity_type: &str, goal: i32) -> Result<(), rusqlite::Error> {
    // Set mode to by_type
    conn.execute(
        "UPDATE gym_user_goal_config SET goal_mode = 'by_type' WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
    )?;
    // Upsert the type goal
    conn.execute(
        "INSERT INTO gym_user_type_goals (guild_id, user_id, activity_type, goal) VALUES (?, ?, ?, ?)
         ON CONFLICT(guild_id, user_id, activity_type) DO UPDATE SET goal = excluded.goal",
        params![guild_id, user_id, activity_type, goal],
    )?;
    Ok(())
}

// ============================================================================
// User Totals
// ============================================================================

pub fn get_user_totals(conn: &Connection, guild_id: u64, user_id: u64) -> Result<Option<UserTotals>, rusqlite::Error> {
    conn.query_row(
        "SELECT guild_id, user_id, total_count, achieved_goals, missed_goals FROM gym_user_totals WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
        |row| {
            Ok(UserTotals {
                guild_id: row.get(0)?,
                user_id: row.get(1)?,
                total_count: row.get(2)?,
                achieved_goals: row.get(3)?,
                missed_goals: row.get(4)?,
            })
        },
    )
    .optional()
}

pub fn get_all_user_totals(conn: &Connection, guild_id: u64) -> Result<Vec<UserTotals>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT guild_id, user_id, total_count, achieved_goals, missed_goals FROM gym_user_totals WHERE guild_id = ? ORDER BY total_count DESC",
    )?;
    let totals = stmt
        .query_map([guild_id], |row| {
            Ok(UserTotals {
                guild_id: row.get(0)?,
                user_id: row.get(1)?,
                total_count: row.get(2)?,
                achieved_goals: row.get(3)?,
                missed_goals: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(totals)
}

pub fn update_user_totals(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    count_delta: i32,
    achieved_delta: i32,
    missed_delta: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_user_totals SET total_count = total_count + ?, achieved_goals = achieved_goals + ?, missed_goals = missed_goals + ? WHERE guild_id = ? AND user_id = ?",
        params![count_delta, achieved_delta, missed_delta, guild_id, user_id],
    )?;
    Ok(())
}

pub fn set_user_type_total(conn: &Connection, guild_id: u64, user_id: u64, activity_type: &str, count: i32) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_user_type_totals (guild_id, user_id, activity_type, count) VALUES (?, ?, ?, ?)
         ON CONFLICT(guild_id, user_id, activity_type) DO UPDATE SET count = excluded.count",
        params![guild_id, user_id, activity_type, count],
    )?;
    Ok(())
}

pub fn get_user_type_totals(conn: &Connection, guild_id: u64, user_id: u64) -> Result<HashMap<String, i32>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT activity_type, count FROM gym_user_type_totals WHERE guild_id = ? AND user_id = ?",
    )?;
    let mut counts = HashMap::new();
    let rows = stmt.query_map(params![guild_id, user_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
    })?;
    for row in rows {
        let (activity_type, count) = row?;
        counts.insert(activity_type, count);
    }
    Ok(counts)
}

pub fn set_user_goal_stats(conn: &Connection, guild_id: u64, user_id: u64, achieved: i32, missed: i32) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_user_totals SET achieved_goals = ?, missed_goals = ? WHERE guild_id = ? AND user_id = ?",
        params![achieved, missed, guild_id, user_id],
    )?;
    Ok(())
}

// ============================================================================
// Started Guilds (for background task)
// ============================================================================

pub fn get_started_guilds(conn: &Connection) -> Result<Vec<GuildConfig>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT guild_id, channel_id, default_goal, started FROM gym_guild_config WHERE started = 1"
    )?;
    let guilds = stmt
        .query_map([], |row| {
            Ok(GuildConfig {
                guild_id: row.get(0)?,
                channel_id: row.get(1)?,
                default_goal: row.get(2)?,
                started: row.get::<_, i32>(3)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(guilds)
}

// ============================================================================
// Period Results
// ============================================================================

pub fn insert_period_result(
    conn: &Connection,
    period_id: i64,
    user_id: u64,
    total_count: i32,
    goal_met: bool,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_period_results (period_id, user_id, total_count, goal_met) VALUES (?, ?, ?, ?)",
        params![period_id, user_id, total_count, goal_met as i32],
    )?;
    Ok(())
}

pub fn insert_period_type_count(
    conn: &Connection,
    period_id: i64,
    user_id: u64,
    activity_type: &str,
    count: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_period_type_counts (period_id, user_id, activity_type, count) VALUES (?, ?, ?, ?)",
        params![period_id, user_id, activity_type, count],
    )?;
    Ok(())
}

pub fn increment_user_type_total(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    activity_type: &str,
    delta: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_user_type_totals (guild_id, user_id, activity_type, count) VALUES (?, ?, ?, ?)
         ON CONFLICT(guild_id, user_id, activity_type) DO UPDATE SET count = count + excluded.count",
        params![guild_id, user_id, activity_type, delta],
    )?;
    Ok(())
}
