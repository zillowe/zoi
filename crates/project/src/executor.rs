use std::collections::HashMap;
use std::process::Command;

use anyhow::{Result, anyhow};
use colored::Colorize;

/// Runs a shell command with the provided environment variables.
///
/// # Errors
///
/// Returns an error if the command fails to start or if it returns a non-zero
/// exit code.
pub fn run_shell_command<S: ::std::hash::BuildHasher>(
    command_str: &str,
    envs: &HashMap<String, String, S>
) -> Result<()> {
    println!("> {}", command_str.cyan());
    let status = get_shell_command(command_str).envs(envs).status()?;

    if !status.success() {
        return Err(anyhow!("Command failed with exit code {status}"));
    }
    Ok(())
}

/// Returns a `std::process::Command` configured to run a shell command.
///
/// On Windows, it uses `pwsh`. On other platforms, it uses `bash`.
pub fn get_shell_command(command_str: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("pwsh");
        cmd.arg("-Command").arg(command_str);
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(command_str);
        cmd
    }
}
