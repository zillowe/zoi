use colored::*;
use std::error::Error;
use std::process::Command;

pub fn run_shell_command(command_str: &str) -> Result<(), Box<dyn Error>> {
    println!("> {}", command_str.cyan());
    let status = Command::new("bash").arg("-c").arg(command_str).status()?;

    if !status.success() {
        return Err(format!("Command failed with exit code {status}").into());
    }
    Ok(())
}
