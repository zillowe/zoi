//! Command for managing environment variables and shell integration.

use anyhow::Result;
use clap_complete::Shell;
use colored::Colorize;
use zoi_project::{config, environment};

/// Runs the 'env' command.
///
/// Sets up the environment for a project or exports shell configuration
/// for environment variables.
///
/// # Errors
///
/// Returns an error if the project configuration cannot be loaded or if
/// environment setup/export fails.
pub fn run(
    env_alias: Option<&str,>,
    export_shell: Option<Shell,>,
) -> Result<(),> {
    let config = config::load()?;
    if let Some(shell,) = export_shell {
        environment::export_shell(env_alias, &config, shell,)?;
    } else {
        environment::setup(env_alias, &config,)?;
        println!("\n{}", "Environment setup complete.".green());
    }
    Ok((),)
}
