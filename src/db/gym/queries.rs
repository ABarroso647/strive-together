// Gym tracker database queries
// All table names prefixed with gym_

use super::models::*;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;

// ============================================================================
// Guild Config
// ============================================================================

pub fn get_guild_config(
    conn: &Connection,
    guild_id: u64,
) -> Result<Option<GuildConfig>, rusqlite::Error> {
    conn.query_row(
        "SELECT guild_id, channel_id, default_goal, started, COALESCE(rollover_hour, 12) FROM gym_guild_config WHERE guild_id = ?",
        [guild_id],
        |row| {
            Ok(GuildConfig {
                guild_id: row.get(0)?,
                channel_id: row.get(1)?,
                default_goal: row.get(2)?,
                started: row.get::<_, i32>(3)? != 0,
                rollover_hour: row.get::<_, u32>(4)?,
            })
        },
    )
    .optional()
}

pub fn insert_guild_config(
    conn: &Connection,
    guild_id: u64,
    channel_id: u64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_guild_config (guild_id, channel_id) VALUES (?, ?)",
        [guild_id, channel_id],
    )?;
    Ok(())
}

pub fn update_guild_started(
    conn: &Connection,
    guild_id: u64,
    started: bool,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_guild_config SET started = ? WHERE guild_id = ?",
        params![started as i32, guild_id],
    )?;
    Ok(())
}

pub fn update_default_goal(
    conn: &Connection,
    guild_id: u64,
    goal: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_guild_config SET default_goal = ? WHERE guild_id = ?",
        params![goal, guild_id],
    )?;
    Ok(())
}

/// Update the total_goal for every user who is still "on the default" —
/// meaning their goal equals `old_goal` and they have no type or group customizations.
/// Returns the number of users updated.
pub fn update_default_goal_for_users(
    conn: &Connection,
    guild_id: u64,
    old_goal: i32,
    new_goal: i32,
) -> Result<usize, rusqlite::Error> {
    let rows = conn.execute(
        "UPDATE gym_user_goal_config
         SET total_goal = ?
         WHERE guild_id = ?
           AND total_goal = ?
           AND NOT EXISTS (
               SELECT 1 FROM gym_user_type_goals
               WHERE gym_user_type_goals.guild_id = gym_user_goal_config.guild_id
                 AND gym_user_type_goals.user_id = gym_user_goal_config.user_id
           )
           AND NOT EXISTS (
               SELECT 1 FROM gym_user_group_goals
               WHERE gym_user_group_goals.guild_id = gym_user_goal_config.guild_id
                 AND gym_user_group_goals.user_id = gym_user_goal_config.user_id
           )",
        params![new_goal, guild_id, old_goal],
    )?;
    Ok(rows)
}

pub fn update_rollover_hour(
    conn: &Connection,
    guild_id: u64,
    hour: u32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_guild_config SET rollover_hour = ? WHERE guild_id = ?",
        params![hour, guild_id],
    )?;
    Ok(())
}

// ============================================================================
// Activity Types
// ============================================================================

pub fn get_activity_types(
    conn: &Connection,
    guild_id: u64,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT name FROM gym_activity_types WHERE guild_id = ? ORDER BY name")?;
    let types = stmt
        .query_map([guild_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(types)
}

pub fn insert_activity_type(
    conn: &Connection,
    guild_id: u64,
    name: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO gym_activity_types (guild_id, name) VALUES (?, ?)",
        params![guild_id, name],
    )?;
    Ok(())
}

pub fn delete_activity_type(
    conn: &Connection,
    guild_id: u64,
    name: &str,
) -> Result<bool, rusqlite::Error> {
    let rows = conn.execute(
        "DELETE FROM gym_activity_types WHERE guild_id = ? AND name = ?",
        params![guild_id, name],
    )?;
    Ok(rows > 0)
}

pub fn activity_type_exists(
    conn: &Connection,
    guild_id: u64,
    name: &str,
) -> Result<bool, rusqlite::Error> {
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

pub fn user_exists(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
) -> Result<bool, rusqlite::Error> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM gym_users WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn insert_user(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    default_goal: i32,
) -> Result<(), rusqlite::Error> {
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

pub fn delete_user(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
) -> Result<bool, rusqlite::Error> {
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

pub fn get_current_period(
    conn: &Connection,
    guild_id: u64,
) -> Result<Option<Period>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, start_time, end_time FROM gym_periods WHERE guild_id = ? AND is_current = 1",
        [guild_id],
        |row| {
            Ok(Period {
                id: row.get(0)?,
                start_time: row.get(1)?,
                end_time: row.get(2)?,
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

pub fn get_user_period_count(
    conn: &Connection,
    period_id: i64,
    user_id: u64,
) -> Result<i32, rusqlite::Error> {
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

pub fn get_user_goal_config(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
) -> Result<Option<UserGoalConfig>, rusqlite::Error> {
    conn.query_row(
        "SELECT total_goal FROM gym_user_goal_config WHERE guild_id = ? AND user_id = ?",
        params![guild_id, user_id],
        |row| {
            Ok(UserGoalConfig {
                total_goal: row.get(0)?,
            })
        },
    )
    .optional()
}

/// A user's weekly total goal. Errors if the user has no goal set up.
pub fn get_user_total_goal(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
) -> Result<i32, crate::Error> {
    let gc = get_user_goal_config(conn, guild_id, user_id)?
        .ok_or_else(|| format!("<@{}> has no goal set up.", user_id))?;
    Ok(gc.total_goal)
}

/// Whether a user met all their goals (total, per-type, per-group) for a period, from its logs.
pub fn evaluate_goal_met(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    period_id: i64,
) -> Result<bool, crate::Error> {
    let total_goal = get_user_total_goal(conn, guild_id, user_id)?;
    let total_count = get_user_period_count(conn, period_id, user_id)?;
    let type_counts = get_user_period_type_counts(conn, period_id, user_id)?;
    let type_group_map = get_all_type_groups(conn, guild_id)?;

    // 1. Total minimum always required
    if total_count < total_goal {
        return Ok(false);
    }

    // 2. Any per-type minimums (additive — all must be met)
    let type_goals = get_user_type_goals(conn, guild_id, user_id)?;
    for (t, g) in &type_goals {
        if type_counts.get(t).copied().unwrap_or(0) < *g {
            return Ok(false);
        }
    }

    // 3. Any per-group minimums (additive — all must be met)
    let group_goals = get_user_group_goals(conn, guild_id, user_id)?;
    for (grp, goal) in &group_goals {
        let group_total: i32 = type_counts
            .iter()
            .filter(|(t, _)| type_group_map.get(*t).map(|g| g == grp).unwrap_or(false))
            .map(|(_, c)| c)
            .sum();
        if group_total < *goal {
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn update_user_total_goal(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    goal: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_user_goal_config SET total_goal = ? WHERE guild_id = ? AND user_id = ?",
        params![goal, guild_id, user_id],
    )?;
    Ok(())
}

pub fn set_user_type_goal(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    activity_type: &str,
    goal: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_user_type_goals (guild_id, user_id, activity_type, goal) VALUES (?, ?, ?, ?)
         ON CONFLICT(guild_id, user_id, activity_type) DO UPDATE SET goal = excluded.goal",
        params![guild_id, user_id, activity_type, goal],
    )?;
    Ok(())
}

pub fn get_user_type_goals(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
) -> Result<Vec<(String, i32)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT activity_type, goal FROM gym_user_type_goals WHERE guild_id = ? AND user_id = ? ORDER BY activity_type",
    )?;
    let goals = stmt
        .query_map(params![guild_id, user_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<(String, i32)>, _>>()?;
    Ok(goals)
}

// ============================================================================
// User Totals
// ============================================================================

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

pub fn set_user_type_total(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    activity_type: &str,
    count: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_user_type_totals (guild_id, user_id, activity_type, count) VALUES (?, ?, ?, ?)
         ON CONFLICT(guild_id, user_id, activity_type) DO UPDATE SET count = excluded.count",
        params![guild_id, user_id, activity_type, count],
    )?;
    Ok(())
}

pub fn set_user_goal_stats(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    achieved: i32,
    missed: i32,
) -> Result<(), rusqlite::Error> {
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
        "SELECT guild_id, channel_id, default_goal, started, COALESCE(rollover_hour, 12) FROM gym_guild_config WHERE started = 1"
    )?;
    let guilds = stmt
        .query_map([], |row| {
            Ok(GuildConfig {
                guild_id: row.get(0)?,
                channel_id: row.get(1)?,
                default_goal: row.get(2)?,
                started: row.get::<_, i32>(3)? != 0,
                rollover_hour: row.get::<_, u32>(4)?,
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
    loa_exempt: bool,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_period_results (period_id, user_id, total_count, goal_met, loa_exempt) VALUES (?, ?, ?, ?, ?)",
        params![period_id, user_id, total_count, goal_met as i32, loa_exempt as i32],
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

// ============================================================================
// Activity Groups
// ============================================================================

pub fn get_activity_groups(
    conn: &Connection,
    guild_id: u64,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT group_name FROM gym_activity_groups WHERE guild_id = ? ORDER BY group_name",
    )?;
    let groups = stmt
        .query_map([guild_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(groups)
}

pub fn group_exists(
    conn: &Connection,
    guild_id: u64,
    group_name: &str,
) -> Result<bool, rusqlite::Error> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM gym_activity_groups WHERE guild_id = ? AND group_name = ?",
        params![guild_id, group_name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn insert_activity_group(
    conn: &Connection,
    guild_id: u64,
    group_name: &str,
) -> Result<bool, rusqlite::Error> {
    let rows = conn.execute(
        "INSERT OR IGNORE INTO gym_activity_groups (guild_id, group_name) VALUES (?, ?)",
        params![guild_id, group_name],
    )?;
    Ok(rows > 0)
}

pub fn delete_activity_group(
    conn: &Connection,
    guild_id: u64,
    group_name: &str,
) -> Result<bool, rusqlite::Error> {
    let rows = conn.execute(
        "DELETE FROM gym_activity_groups WHERE guild_id = ? AND group_name = ?",
        params![guild_id, group_name],
    )?;
    Ok(rows > 0)
}

/// Assign an activity type to a group (replaces any previous assignment)
pub fn assign_type_to_group(
    conn: &Connection,
    guild_id: u64,
    activity_type: &str,
    group_name: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_type_group_map (guild_id, activity_type, group_name) VALUES (?, ?, ?)
         ON CONFLICT(guild_id, activity_type) DO UPDATE SET group_name = excluded.group_name",
        params![guild_id, activity_type, group_name],
    )?;
    Ok(())
}

/// Remove a type from its group
pub fn unassign_type_from_group(
    conn: &Connection,
    guild_id: u64,
    activity_type: &str,
) -> Result<bool, rusqlite::Error> {
    let rows = conn.execute(
        "DELETE FROM gym_type_group_map WHERE guild_id = ? AND activity_type = ?",
        params![guild_id, activity_type],
    )?;
    Ok(rows > 0)
}

/// Get the group a type belongs to, if any
pub fn get_type_group(
    conn: &Connection,
    guild_id: u64,
    activity_type: &str,
) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT group_name FROM gym_type_group_map WHERE guild_id = ? AND activity_type = ?",
        params![guild_id, activity_type],
        |row| row.get(0),
    )
    .optional()
}

/// Get all types assigned to a group
pub fn get_group_types(
    conn: &Connection,
    guild_id: u64,
    group_name: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT activity_type FROM gym_type_group_map WHERE guild_id = ? AND group_name = ? ORDER BY activity_type"
    )?;
    let types = stmt
        .query_map(params![guild_id, group_name], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(types)
}

/// Get all type→group assignments for a guild
pub fn get_all_type_groups(
    conn: &Connection,
    guild_id: u64,
) -> Result<HashMap<String, String>, rusqlite::Error> {
    let mut stmt = conn
        .prepare("SELECT activity_type, group_name FROM gym_type_group_map WHERE guild_id = ?")?;
    let mut map = HashMap::new();
    let rows = stmt.query_map([guild_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (t, g) = row?;
        map.insert(t, g);
    }
    Ok(map)
}

// ============================================================================
// User Group Goals
// ============================================================================

pub fn set_user_group_goal(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    group_name: &str,
    goal: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_user_group_goals (guild_id, user_id, group_name, goal) VALUES (?, ?, ?, ?)
         ON CONFLICT(guild_id, user_id, group_name) DO UPDATE SET goal = excluded.goal",
        params![guild_id, user_id, group_name, goal],
    )?;
    Ok(())
}

pub fn get_user_group_goals(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
) -> Result<Vec<(String, i32)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT group_name, goal FROM gym_user_group_goals WHERE guild_id = ? AND user_id = ? ORDER BY group_name"
    )?;
    let goals = stmt
        .query_map(params![guild_id, user_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<(String, i32)>, _>>()?;
    Ok(goals)
}

// ============================================================================
// Completed Periods (for history)
// ============================================================================

/// Get the last `limit` completed (non-current) periods for a guild, ordered oldest→newest
pub fn get_completed_periods(
    conn: &Connection,
    guild_id: u64,
    limit: usize,
) -> Result<Vec<Period>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, start_time, end_time FROM gym_periods
         WHERE guild_id = ? AND is_current = 0
         ORDER BY start_time DESC LIMIT ?",
    )?;
    let mut periods: Vec<Period> = stmt
        .query_map(params![guild_id, limit as i64], |row| {
            Ok(Period {
                id: row.get(0)?,
                start_time: row.get(1)?,
                end_time: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    periods.reverse();
    Ok(periods)
}

// ============================================================================
// Season / All-completed-periods stats
// ============================================================================

/// Aggregate user stats across completed periods for a guild.
/// Pass season_id=Some(id) to scope to a season, or None for all-time.
pub fn get_season_user_stats(
    conn: &Connection,
    guild_id: u64,
    season_id: Option<i64>,
) -> Result<Vec<(u64, i32, i32, i32)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT pr.user_id,
                COALESCE(SUM(pr.total_count), 0),
                COALESCE(SUM(CASE WHEN pr.goal_met = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN pr.goal_met = 0 THEN 1 ELSE 0 END), 0)
         FROM gym_period_results pr
         JOIN gym_periods p ON pr.period_id = p.id
         WHERE p.guild_id = ? AND p.is_current = 0 AND (? IS NULL OR p.season_id = ?)
         GROUP BY pr.user_id
         ORDER BY 2 DESC",
    )?;
    let rows = stmt
        .query_map(params![guild_id, season_id, season_id], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Aggregate per-activity counts across completed periods for a guild.
/// Pass season_id=Some(id) to scope to a season, or None for all-time.
pub fn get_season_type_stats(
    conn: &Connection,
    guild_id: u64,
    season_id: Option<i64>,
) -> Result<HashMap<u64, HashMap<String, i32>>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT ptc.user_id, ptc.activity_type, COALESCE(SUM(ptc.count), 0)
         FROM gym_period_type_counts ptc
         JOIN gym_periods p ON ptc.period_id = p.id
         WHERE p.guild_id = ? AND p.is_current = 0 AND (? IS NULL OR p.season_id = ?)
         GROUP BY ptc.user_id, ptc.activity_type",
    )?;
    let mut map: HashMap<u64, HashMap<String, i32>> = HashMap::new();
    let rows = stmt.query_map(params![guild_id, season_id, season_id], |row| {
        Ok((
            row.get::<_, u64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i32>(2)?,
        ))
    })?;
    for row in rows {
        let (uid, t, c) = row?;
        map.entry(uid).or_default().insert(t, c);
    }
    Ok(map)
}

/// How many completed periods exist for a guild (or within a season).
pub fn get_completed_period_count(
    conn: &Connection,
    guild_id: u64,
    season_id: Option<i64>,
) -> Result<i32, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM gym_periods WHERE guild_id = ? AND is_current = 0 AND (? IS NULL OR season_id = ?)",
        params![guild_id, season_id, season_id],
        |row| row.get(0),
    )
}

// ============================================================================
// Seasons
// ============================================================================

fn row_to_season(row: &rusqlite::Row<'_>) -> Result<Season, rusqlite::Error> {
    Ok(Season {
        id: row.get(0)?,
        name: row.get(1)?,
        start_time: row.get(2)?,
        end_time: row.get(3)?,
        is_current: row.get::<_, i32>(4)? != 0,
    })
}

pub fn get_current_season(
    conn: &Connection,
    guild_id: u64,
) -> Result<Option<Season>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, name, start_time, end_time, is_current
         FROM gym_seasons WHERE guild_id = ? AND is_current = 1",
        [guild_id],
        row_to_season,
    )
    .optional()
}

pub fn get_all_seasons(conn: &Connection, guild_id: u64) -> Result<Vec<Season>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, start_time, end_time, is_current
         FROM gym_seasons WHERE guild_id = ? ORDER BY id ASC",
    )?;
    stmt.query_map([guild_id], row_to_season)?
        .collect::<Result<Vec<_>, _>>()
}

pub fn count_seasons(conn: &Connection, guild_id: u64) -> Result<i32, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM gym_seasons WHERE guild_id = ?",
        [guild_id],
        |row| row.get(0),
    )
}

/// Create a new season. Returns the new season's id.
pub fn insert_season(
    conn: &Connection,
    guild_id: u64,
    name: &str,
    start_time: &str,
) -> Result<i64, rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_seasons (guild_id, name, start_time, is_current) VALUES (?, ?, ?, 1)",
        params![guild_id, name, start_time],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Close the current season (sets end_time and is_current=0).
pub fn close_current_season(
    conn: &Connection,
    guild_id: u64,
    end_time: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_seasons SET is_current = 0, end_time = ? WHERE guild_id = ? AND is_current = 1",
        params![end_time, guild_id],
    )?;
    Ok(())
}

/// Tag a period with a season (used at rollover and when starting a season).
pub fn set_period_season(
    conn: &Connection,
    period_id: i64,
    season_id: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_periods SET season_id = ? WHERE id = ?",
        params![season_id, period_id],
    )?;
    Ok(())
}

/// Tag all periods for a guild that have no season yet (retroactive Szn 1 assignment).
pub fn tag_unassigned_periods(
    conn: &Connection,
    guild_id: u64,
    season_id: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_periods SET season_id = ? WHERE guild_id = ? AND season_id IS NULL",
        params![season_id, guild_id],
    )?;
    Ok(())
}

/// Get all period results for a given period
pub fn get_period_results(
    conn: &Connection,
    period_id: i64,
) -> Result<Vec<(u64, i32, bool, bool)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT user_id, total_count, goal_met, COALESCE(loa_exempt, 0) FROM gym_period_results WHERE period_id = ?"
    )?;
    let rows = stmt.query_map([period_id], |row| {
        Ok((
            row.get::<_, u64>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, i32>(2)? != 0,
            row.get::<_, i32>(3)? != 0,
        ))
    })?;
    rows.collect()
}

// ============================================================================
// Retroactive logging support
// ============================================================================

/// Increment total_count in gym_period_results for a past period.
/// Inserts with goal_met=0 if the row doesn't exist yet (e.g. user added after the period closed).
pub fn increment_period_result_count(
    conn: &Connection,
    period_id: i64,
    user_id: u64,
    delta: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_period_results (period_id, user_id, total_count, goal_met) VALUES (?, ?, ?, 0)
         ON CONFLICT(period_id, user_id) DO UPDATE SET total_count = total_count + excluded.total_count",
        params![period_id, user_id, delta],
    )?;
    Ok(())
}

/// Increment (or insert) a per-type count in gym_period_type_counts for a past period.
pub fn increment_period_type_count_upsert(
    conn: &Connection,
    period_id: i64,
    user_id: u64,
    activity_type: &str,
    delta: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_period_type_counts (period_id, user_id, activity_type, count) VALUES (?, ?, ?, ?)
         ON CONFLICT(period_id, user_id, activity_type) DO UPDATE SET count = count + excluded.count",
        params![period_id, user_id, activity_type, delta],
    )?;
    Ok(())
}

/// Set the stored goal result for a user in a closed period.
pub fn set_period_goal_met(
    conn: &Connection,
    period_id: i64,
    user_id: u64,
    goal_met: bool,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_period_results SET goal_met = ? WHERE period_id = ? AND user_id = ?",
        params![goal_met as i32, period_id, user_id],
    )?;
    Ok(())
}

/// Re-check a closed period's stored goal result against its logs; if wrong, fix it and move
/// one count between achieved/missed. LOA weeks are skipped. Returns true if it changed.
pub fn reevaluate_period_goal(
    conn: &Connection,
    guild_id: u64,
    period_id: i64,
    user_id: u64,
    stored_goal_met: bool,
    loa_exempt: bool,
) -> Result<bool, crate::Error> {
    if loa_exempt {
        return Ok(false);
    }
    let goal_met = evaluate_goal_met(conn, guild_id, user_id, period_id)?;
    if goal_met == stored_goal_met {
        return Ok(false);
    }
    set_period_goal_met(conn, period_id, user_id, goal_met)?;
    let (achieved_delta, missed_delta) = if goal_met { (1, -1) } else { (-1, 1) };
    update_user_totals(conn, guild_id, user_id, 0, achieved_delta, missed_delta)?;
    Ok(true)
}

/// Log a workout into a closed period, updating its archived results, all-time totals,
/// and goal result.
pub fn backfill_log(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    period_id: i64,
    activity_type: &str,
    logged_at: &str,
) -> Result<(), crate::Error> {
    let stored = get_period_results(conn, period_id)?
        .into_iter()
        .find(|(uid, ..)| *uid == user_id)
        .map(|(_, _, goal_met, loa_exempt)| (goal_met, loa_exempt));

    insert_log(conn, guild_id, user_id, period_id, activity_type, logged_at)?;
    increment_period_type_count_upsert(conn, period_id, user_id, activity_type, 1)?;
    increment_period_result_count(conn, period_id, user_id, 1)?;
    update_user_totals(conn, guild_id, user_id, 1, 0, 0)?;
    increment_user_type_total(conn, guild_id, user_id, activity_type, 1)?;

    let (goal_met, loa_exempt) = match stored {
        Some(result) => result,
        None => {
            // New result row starts as a miss; count it like rollover would have
            update_user_totals(conn, guild_id, user_id, 0, 0, 1)?;
            (false, false)
        }
    };
    reevaluate_period_goal(conn, guild_id, period_id, user_id, goal_met, loa_exempt)?;
    Ok(())
}

// ============================================================================
// History type breakdown queries
// ============================================================================

/// Get per-type counts for ALL users in a period.
/// Returns HashMap<user_id, Vec<(type, count)>> sorted by count desc per user.
pub fn get_all_period_type_counts(
    conn: &Connection,
    period_id: i64,
) -> Result<HashMap<u64, Vec<(String, i32)>>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT user_id, activity_type, count FROM gym_period_type_counts
         WHERE period_id = ? ORDER BY count DESC",
    )?;
    let mut map: HashMap<u64, Vec<(String, i32)>> = HashMap::new();
    let rows = stmt.query_map([period_id], |row| {
        Ok((
            row.get::<_, u64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i32>(2)?,
        ))
    })?;
    for row in rows {
        let (uid, t, c) = row?;
        map.entry(uid).or_default().push((t, c));
    }
    Ok(map)
}

/// Get ALL completed periods in a season (no limit), oldest→newest.
/// Deduplicates by (start_time, end_time) — keeps the highest-id period per date range.
pub fn get_all_completed_periods_in_season(
    conn: &Connection,
    guild_id: u64,
    season_id: i64,
) -> Result<Vec<Period>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, start_time, end_time FROM gym_periods
         WHERE guild_id = ? AND is_current = 0 AND season_id = ?
           AND id IN (
               SELECT MAX(id) FROM gym_periods
               WHERE guild_id = ? AND is_current = 0 AND season_id = ?
               GROUP BY start_time, end_time
           )
         ORDER BY start_time ASC",
    )?;
    stmt.query_map(params![guild_id, season_id, guild_id, season_id], |row| {
        Ok(Period {
            id: row.get(0)?,
            start_time: row.get(1)?,
            end_time: row.get(2)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()
}

/// Get ALL completed periods for a guild (no limit), oldest→newest.
/// Deduplicates by (start_time, end_time) — keeps the highest-id period per date range.
pub fn get_all_completed_periods(
    conn: &Connection,
    guild_id: u64,
) -> Result<Vec<Period>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, start_time, end_time FROM gym_periods
         WHERE guild_id = ? AND is_current = 0
           AND id IN (
               SELECT MAX(id) FROM gym_periods
               WHERE guild_id = ? AND is_current = 0
               GROUP BY start_time, end_time
           )
         ORDER BY start_time ASC",
    )?;
    stmt.query_map(params![guild_id, guild_id], |row| {
        Ok(Period {
            id: row.get(0)?,
            start_time: row.get(1)?,
            end_time: row.get(2)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()
}

// ============================================================================
// Goal change history
// ============================================================================

/// Record a goal configuration change for a user. `description` is a human-readable
/// summary like "total goal → 5" or "added: push ≥ 3".
pub fn record_goal_change(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    changed_at: &str,
    description: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_goal_history (guild_id, user_id, changed_at, description) VALUES (?, ?, ?, ?)",
        params![guild_id, user_id, changed_at, description],
    )?;
    Ok(())
}

/// Get ALL goal changes for a user, oldest→newest.
pub fn get_all_goal_changes(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT changed_at, description FROM gym_goal_history
         WHERE guild_id = ? AND user_id = ?
         ORDER BY changed_at ASC",
    )?;
    stmt.query_map(params![guild_id, user_id], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?
    .collect::<Result<Vec<_>, _>>()
}

// ============================================================================
// Log message / attachment / reaction tracking (groundwork for end-of-season)
// ============================================================================

/// Store a Discord message ID for a log post.
pub fn insert_log_message(
    conn: &Connection,
    message_id: u64,
    guild_id: u64,
    channel_id: u64,
    period_id: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO gym_log_messages (message_id, guild_id, channel_id, period_id) VALUES (?, ?, ?, ?)",
        params![message_id, guild_id, channel_id, period_id],
    )?;
    Ok(())
}

/// Look up which guild a log message belongs to (for reaction tracking).
pub fn get_log_message_guild(
    conn: &Connection,
    message_id: u64,
) -> Result<Option<u64>, rusqlite::Error> {
    conn.query_row(
        "SELECT guild_id FROM gym_log_messages WHERE message_id = ?",
        [message_id],
        |row| row.get(0),
    )
    .optional()
}

/// Store an image attachment URL from a log command.
pub fn insert_log_attachment(
    conn: &Connection,
    message_id: u64,
    guild_id: u64,
    user_id: u64,
    url: &str,
    filename: &str,
    uploaded_at: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_log_attachments (message_id, guild_id, user_id, url, filename, uploaded_at) VALUES (?, ?, ?, ?, ?, ?)",
        params![message_id, guild_id, user_id, url, filename, uploaded_at],
    )?;
    Ok(())
}

/// Record a 🔥 reaction on a log post (upsert — idempotent).
pub fn upsert_log_reaction(
    conn: &Connection,
    message_id: u64,
    reactor_user_id: u64,
    reacted_at: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO gym_log_reactions (message_id, reactor_user_id, reacted_at) VALUES (?, ?, ?)",
        params![message_id, reactor_user_id, reacted_at],
    )?;
    Ok(())
}

/// Remove a 🔥 reaction (user un-reacted).
pub fn remove_log_reaction(
    conn: &Connection,
    message_id: u64,
    reactor_user_id: u64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM gym_log_reactions WHERE message_id = ? AND reactor_user_id = ?",
        params![message_id, reactor_user_id],
    )?;
    Ok(())
}

// ============================================================================
// Leave of Absence (LOA)
// ============================================================================

/// Insert a new LOA request. `loa_start` and `loa_end` are stored immediately so the
/// window is known regardless of when the vote resolves.
pub fn insert_loa_request(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    requested_at: &str,
    weeks: i64,
    vote_channel_id: u64,
    vote_ends_at: &str,
    loa_start: &str,
    loa_end: &str,
    mention_role_id: Option<u64>,
) -> Result<i64, rusqlite::Error> {
    conn.execute(
        "INSERT INTO gym_loa_requests (guild_id, user_id, requested_at, weeks, vote_channel_id, vote_ends_at, loa_start, loa_end, mention_role_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![guild_id, user_id, requested_at, weeks, vote_channel_id, vote_ends_at, loa_start, loa_end, mention_role_id],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Return the earliest vote_ends_at across all pending LOA requests (for smart sleep).
pub fn get_earliest_pending_vote_end(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT MIN(vote_ends_at) FROM gym_loa_requests WHERE status = 'pending'",
        [],
        |row| row.get(0),
    )
    .ok()
    .flatten()
}

pub fn set_loa_vote_message(
    conn: &Connection,
    id: i64,
    message_id: u64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_loa_requests SET vote_message_id = ? WHERE id = ?",
        params![message_id, id],
    )?;
    Ok(())
}

pub fn get_pending_loa_for_user(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
) -> Result<Option<LoaRequest>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, guild_id, user_id, weeks, vote_message_id, vote_channel_id, loa_start, loa_end, mention_role_id FROM gym_loa_requests WHERE guild_id = ? AND user_id = ? AND status = 'pending'",
        params![guild_id, user_id],
        |row| Ok(LoaRequest {
            id: row.get(0)?,
            guild_id: row.get(1)?,
            user_id: row.get(2)?,
            weeks: row.get(3)?,
            vote_message_id: row.get(4)?,
            vote_channel_id: row.get(5)?,
            loa_start: row.get(6)?,
            loa_end: row.get(7)?,
            mention_role_id: row.get(8)?,
        }),
    ).optional()
}

pub fn get_expired_pending_loas(
    conn: &Connection,
    now_str: &str,
) -> Result<Vec<LoaRequest>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, guild_id, user_id, weeks, vote_message_id, vote_channel_id, loa_start, loa_end, mention_role_id FROM gym_loa_requests WHERE status = 'pending' AND vote_ends_at <= ?"
    )?;
    let rows = stmt.query_map([now_str], |row| {
        Ok(LoaRequest {
            id: row.get(0)?,
            guild_id: row.get(1)?,
            user_id: row.get(2)?,
            weeks: row.get(3)?,
            vote_message_id: row.get(4)?,
            vote_channel_id: row.get(5)?,
            loa_start: row.get(6)?,
            loa_end: row.get(7)?,
            mention_role_id: row.get(8)?,
        })
    })?;
    rows.collect()
}

/// Resolve a pending LOA — just flips the status. loa_start/loa_end were set at request time.
pub fn resolve_loa(conn: &Connection, id: i64, status: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE gym_loa_requests SET status = ? WHERE id = ?",
        params![status, id],
    )?;
    Ok(())
}

/// Returns the active approved LOA for a user if their LOA window overlaps the given period.
pub fn get_active_loa_for_user(
    conn: &Connection,
    guild_id: u64,
    user_id: u64,
    period_start: &str,
    period_end: &str,
) -> Result<Option<LoaRequest>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, guild_id, user_id, weeks, vote_message_id, vote_channel_id, loa_start, loa_end, mention_role_id FROM gym_loa_requests WHERE guild_id = ? AND user_id = ? AND status IN ('approved', 'pending') AND loa_start <= ? AND loa_end >= ?",
        params![guild_id, user_id, period_end, period_start],
        |row| Ok(LoaRequest {
            id: row.get(0)?,
            guild_id: row.get(1)?,
            user_id: row.get(2)?,
            weeks: row.get(3)?,
            vote_message_id: row.get(4)?,
            vote_channel_id: row.get(5)?,
            loa_start: row.get(6)?,
            loa_end: row.get(7)?,
            mention_role_id: row.get(8)?,
        }),
    ).optional()
}

/// Look up a pending LOA by its vote message ID (for real-time reaction handling).
pub fn get_loa_by_vote_message(
    conn: &Connection,
    message_id: u64,
) -> Result<Option<LoaRequest>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, guild_id, user_id, weeks, vote_message_id, vote_channel_id, loa_start, loa_end, mention_role_id FROM gym_loa_requests WHERE vote_message_id = ? AND status = 'pending'",
        [message_id],
        |row| Ok(LoaRequest {
            id: row.get(0)?,
            guild_id: row.get(1)?,
            user_id: row.get(2)?,
            weeks: row.get(3)?,
            vote_message_id: row.get(4)?,
            vote_channel_id: row.get(5)?,
            loa_start: row.get(6)?,
            loa_end: row.get(7)?,
            mention_role_id: row.get(8)?,
        }),
    ).optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    const GUILD: u64 = 1;
    const USER: u64 = 10;

    /// In-memory DB with schema + migrations, a set-up guild, and USER tracked with `total_goal`.
    fn setup(total_goal: i32) -> (Database, i64) {
        let db = Database::new(":memory:").unwrap();
        db.init_schema().unwrap();
        let period_id = {
            let conn = db.conn();
            insert_guild_config(&conn, GUILD, 99).unwrap();
            insert_user(&conn, GUILD, USER, total_goal).unwrap();
            insert_period(&conn, GUILD, "2025-06-01T12:00:00Z", "2025-06-08T12:00:00Z").unwrap()
        };
        (db, period_id)
    }

    fn log(conn: &Connection, period_id: i64, activity_type: &str, n: usize) {
        for _ in 0..n {
            insert_log(
                conn,
                GUILD,
                USER,
                period_id,
                activity_type,
                "2025-06-02T00:00:00Z",
            )
            .unwrap();
        }
    }

    #[test]
    fn total_goal_met_only_at_threshold() {
        let (db, period) = setup(3);
        let conn = db.conn();
        log(&conn, period, "push", 2);
        assert!(!evaluate_goal_met(&conn, GUILD, USER, period).unwrap());
        log(&conn, period, "push", 1);
        assert!(evaluate_goal_met(&conn, GUILD, USER, period).unwrap());
    }

    #[test]
    fn type_minimum_required_even_when_total_met() {
        let (db, period) = setup(2);
        let conn = db.conn();
        set_user_type_goal(&conn, GUILD, USER, "push", 2).unwrap();
        log(&conn, period, "pull", 2);
        assert!(!evaluate_goal_met(&conn, GUILD, USER, period).unwrap());
        log(&conn, period, "push", 2);
        assert!(evaluate_goal_met(&conn, GUILD, USER, period).unwrap());
    }

    #[test]
    fn group_minimum_counts_only_types_in_that_group() {
        let (db, period) = setup(2);
        let conn = db.conn();
        insert_activity_group(&conn, GUILD, "lift").unwrap();
        assign_type_to_group(&conn, GUILD, "push", "lift").unwrap();
        set_user_group_goal(&conn, GUILD, USER, "lift", 2).unwrap();
        log(&conn, period, "run", 2);
        assert!(!evaluate_goal_met(&conn, GUILD, USER, period).unwrap());
        log(&conn, period, "push", 2);
        assert!(evaluate_goal_met(&conn, GUILD, USER, period).unwrap());
    }

    #[test]
    fn only_counts_logs_in_the_given_period() {
        let (db, period) = setup(2);
        let conn = db.conn();
        let other =
            insert_period(&conn, GUILD, "2025-06-08T12:00:00Z", "2025-06-15T12:00:00Z").unwrap();
        log(&conn, other, "push", 2);
        assert!(!evaluate_goal_met(&conn, GUILD, USER, period).unwrap());
        assert!(evaluate_goal_met(&conn, GUILD, USER, other).unwrap());
    }

    #[test]
    fn user_without_goal_row_errors() {
        let (db, period) = setup(2);
        let conn = db.conn();
        assert!(evaluate_goal_met(&conn, GUILD, 20, period).is_err());
    }

    #[test]
    fn type_and_group_minimums_must_all_hold() {
        let (db, period) = setup(3);
        let conn = db.conn();
        insert_activity_group(&conn, GUILD, "cardio").unwrap();
        assign_type_to_group(&conn, GUILD, "run", "cardio").unwrap();
        assign_type_to_group(&conn, GUILD, "bike", "cardio").unwrap();
        set_user_type_goal(&conn, GUILD, USER, "push", 1).unwrap();
        set_user_group_goal(&conn, GUILD, USER, "cardio", 2).unwrap();
        // total 3 and push ≥ 1 met, cardio 1/2 → not met
        log(&conn, period, "push", 2);
        log(&conn, period, "run", 1);
        assert!(!evaluate_goal_met(&conn, GUILD, USER, period).unwrap());
        // different type in the same group completes cardio
        log(&conn, period, "bike", 1);
        assert!(evaluate_goal_met(&conn, GUILD, USER, period).unwrap());
    }

    /// (total_count, achieved_goals, missed_goals) from gym_user_totals
    fn totals(conn: &Connection) -> (i32, i32, i32) {
        conn.query_row(
            "SELECT total_count, achieved_goals, missed_goals FROM gym_user_totals WHERE guild_id = ? AND user_id = ?",
            params![GUILD, USER],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    }

    /// Stored (total_count, goal_met, loa_exempt) for USER in a period
    fn result(conn: &Connection, period_id: i64) -> Option<(i32, bool, bool)> {
        get_period_results(conn, period_id)
            .unwrap()
            .into_iter()
            .find(|(uid, ..)| *uid == USER)
            .map(|(_, total, met, loa)| (total, met, loa))
    }

    /// Simulate rollover having closed `period` for USER with `count` logs and a miss.
    fn rolled_over_miss(conn: &Connection, period_id: i64, count: usize, loa_exempt: bool) {
        log(conn, period_id, "push", count);
        insert_period_result(conn, period_id, USER, count as i32, false, loa_exempt).unwrap();
        let missed = if loa_exempt { 0 } else { 1 };
        update_user_totals(conn, GUILD, USER, count as i32, 0, missed).unwrap();
    }

    #[test]
    fn backfill_reaching_goal_flips_miss_to_met() {
        let (db, period) = setup(2);
        let conn = db.conn();
        rolled_over_miss(&conn, period, 1, false);
        backfill_log(&conn, GUILD, USER, period, "push", "t").unwrap();
        assert_eq!(result(&conn, period), Some((2, true, false)));
        assert_eq!(totals(&conn), (2, 1, 0));
    }

    #[test]
    fn backfill_requires_sub_goals_before_flipping() {
        let (db, period) = setup(2);
        let conn = db.conn();
        set_user_type_goal(&conn, GUILD, USER, "run", 1).unwrap();
        rolled_over_miss(&conn, period, 1, false);
        backfill_log(&conn, GUILD, USER, period, "push", "t").unwrap();
        assert_eq!(result(&conn, period), Some((2, false, false)));
        backfill_log(&conn, GUILD, USER, period, "run", "t").unwrap();
        assert_eq!(result(&conn, period), Some((3, true, false)));
        assert_eq!(totals(&conn), (3, 1, 0));
    }

    #[test]
    fn backfill_into_loa_week_leaves_goal_stats_alone() {
        let (db, period) = setup(1);
        let conn = db.conn();
        rolled_over_miss(&conn, period, 0, true);
        backfill_log(&conn, GUILD, USER, period, "push", "t").unwrap();
        assert_eq!(result(&conn, period), Some((1, false, true)));
        assert_eq!(totals(&conn), (1, 0, 0));
    }

    #[test]
    fn backfill_into_week_with_no_result_row_counts_it() {
        let (db, period) = setup(2);
        let conn = db.conn();
        backfill_log(&conn, GUILD, USER, period, "push", "t").unwrap();
        assert_eq!(result(&conn, period), Some((1, false, false)));
        assert_eq!(totals(&conn), (1, 0, 1));
        backfill_log(&conn, GUILD, USER, period, "push", "t").unwrap();
        assert_eq!(result(&conn, period), Some((2, true, false)));
        assert_eq!(totals(&conn), (2, 1, 0));
    }

    #[test]
    fn reevaluate_fixes_wrong_met_and_is_idempotent() {
        let (db, period) = setup(2);
        let conn = db.conn();
        // Stored as met with achieved counted, but only 1 log exists
        log(&conn, period, "push", 1);
        insert_period_result(&conn, period, USER, 1, true, false).unwrap();
        update_user_totals(&conn, GUILD, USER, 1, 1, 0).unwrap();

        assert!(reevaluate_period_goal(&conn, GUILD, period, USER, true, false).unwrap());
        assert_eq!(result(&conn, period), Some((1, false, false)));
        assert_eq!(totals(&conn), (1, 0, 1));

        assert!(!reevaluate_period_goal(&conn, GUILD, period, USER, false, false).unwrap());
        assert_eq!(totals(&conn), (1, 0, 1));
    }
}
