//! Running project-specific commands and aliases.

use anyhow::Result;
use zoi_project::{config, runner};

/// Runs a project command or alias.
///
/// This loads the project configuration and executes the requested command or
/// alias with the provided arguments.
///
/// # Errors
///
/// Returns an error if the project configuration cannot be loaded or if the
/// command execution fails.
pub fn run(cmd_alias: Option<&str,>, args: &[String],) -> Result<(),> {
    let config = config::load()?;
    runner::run(cmd_alias, args, &config,)
}
