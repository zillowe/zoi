//! Command for managing environment variables and shell integration.

use anyhow::Result;
use clap_complete::Shell;
use colored::*;
use zoi_project::{config, environment};

/// Runs the 'env' command.
///
/// Sets up the environment for a project or exports shell configuration
/// for environment variables.
pub fn run(env_alias: Option<String>, export_shell: Option<Shell>) -> Result<()> {
    let config = config::load()?;
    if let Some(shell) = export_shell {
        environment::export_shell(env_alias.as_deref(), &config, shell)?;
    } else {
        environment::setup(env_alias.as_deref(), &config)?;
        println!("\n{}", "Environment setup complete.".green());
    }
    Ok(())
}
