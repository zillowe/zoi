use super::{config, executor};
use crate::utils;
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use std::error::Error;

pub fn run(
    cmd_alias: Option<&str>,
    args: &[String],
    config: &config::ProjectConfig,
) -> Result<(), Box<dyn Error>> {
    if config.commands.is_empty() {
        return Err("No commands defined in zoi.yaml".into());
    }

    let command_to_run = match cmd_alias {
        Some(alias) => config
            .commands
            .iter()
            .find(|c| c.cmd == alias)
            .ok_or_else(|| format!("Command alias '{}' not found in zoi.yaml", alias))?
            .clone(),
        None => {
            if !args.is_empty() {
                return Err("Cannot pass arguments when in interactive mode.".into());
            }
            let selections: Vec<&str> = config.commands.iter().map(|c| c.cmd.as_str()).collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose a command to run")
                .items(&selections)
                .default(0)
                .interact_opt()?
                .ok_or("No command chosen.")?;

            config.commands[selection].clone()
        }
    };

    let platform = utils::get_platform()?;

    let run_cmd = match &command_to_run.run {
        config::PlatformOrString::String(s) => s.clone(),
        config::PlatformOrString::Platform(p) => p
            .get(&platform)
            .or_else(|| p.get("default"))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "No command found for platform '{}' and no default specified",
                    platform
                )
            })?,
    };

    let env_vars = match &command_to_run.env {
        config::PlatformOrEnvMap::EnvMap(m) => m.clone(),
        config::PlatformOrEnvMap::Platform(p) => p
            .get(&platform)
            .or_else(|| p.get("default"))
            .cloned()
            .unwrap_or_default(),
    };

    println!("--- Running command: {} ---", command_to_run.cmd.bold());
    let mut full_command = run_cmd;
    if !args.is_empty() {
        full_command.push(' ');
        full_command.push_str(&args.join(" "));
    }
    executor::run_shell_command(&full_command, &env_vars)
}
