use anyhow::{Result, anyhow};
use colored::*;
use std::collections::HashMap;
use std::process::Command;

/// Runs a shell command with the provided environment variables.
pub fn run_shell_command(command_str: &str, envs: &HashMap<String, String>) -> Result<()> {
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
