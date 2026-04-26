// Command modules - one per tracker type
pub mod gym;

use crate::{Data, Error};

/// Register all commands
pub fn commands() -> Vec<poise::Command<Data, Error>> {
    vec![
        gym::gym(),
        // Future: calories::calories(),
    ]
}
