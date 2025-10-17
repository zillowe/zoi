use anyhow::{Result, anyhow};
use colored::*;
use std::collections::HashMap;
use std::process::Command;

pub fn run_shell_command(command_str: &str, envs: &HashMap<String, String>) -> Result<()> {
    println!("> {}", command_str.cyan());
    let status = if cfg!(target_os = "windows") {
        Command::new("pwsh")
            .arg("-Command")
            .arg(command_str)
            .envs(envs)
            .status()?
    } else {
        Command::new("bash")
            .arg("-c")
            .arg(command_str)
            .envs(envs)
            .status()?
    };

    if !status.success() {
        return Err(anyhow!("Command failed with exit code {status}"));
    }
    Ok(())
}
