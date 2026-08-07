//! Running project-specific commands and aliases.

use anyhow::Result;
use zoi_project::{config, runner};

/// Runs a project command or alias.
///
/// This loads the project configuration and executes the requested command or alias
/// with the provided arguments.
pub fn run(cmd_alias: Option<String>, args: Vec<String>) -> Result<()> {
    let config = config::load()?;
    runner::run(cmd_alias.as_deref(), &args, &config)
}
