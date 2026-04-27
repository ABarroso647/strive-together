// Gym tracker commands
mod debug;
mod groups;
mod loa;
mod season;
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
        "setup::period_info",
        "setup::set_period_end",
        "debug::force_rollover",
        "debug::force_register",
        "users::user",
        "types::types_cmd",
        "groups::group",
        "season::season",
        "log::log",
        "log::log_past",
        "goals::goal",
        "loa::loa",
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
