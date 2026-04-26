// Gym tracker commands
mod setup;
mod users;
mod types;
mod log;
mod goals;
mod stats;

use crate::{Data, Error};

pub type Context<'a> = poise::Context<'a, Data, Error>;

/// Parent command for all gym tracker commands
#[poise::command(
    slash_command,
    subcommands(
        "setup::setup",
        "setup::start",
        "setup::stop",
        "setup::info",
        "setup::config",
        "users::add_user",
        "users::remove_user",
        "users::list_users",
        "users::import_user",
        "users::set_type_total",
        "users::set_goal_stats",
        "types::add_type",
        "types::remove_type",
        "types::list_types",
        "log::log",
        "goals::goal",
        "stats::status",
        "stats::summary",
        "stats::totals",
        "stats::history",
    ),
    subcommand_required
)]
pub async fn gym(_ctx: Context<'_>) -> Result<(), Error> {
    // Parent command - subcommands handle actual work
    Ok(())
}
