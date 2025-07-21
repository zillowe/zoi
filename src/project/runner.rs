use super::{config::ProjectConfig, executor};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Select};
use std::error::Error;

pub fn run(cmd_alias: Option<&str>, config: &ProjectConfig) -> Result<(), Box<dyn Error>> {
    if config.commands.is_empty() {
        return Err("No commands defined in zoi.yaml".into());
    }

    let command_to_run = match cmd_alias {
        Some(alias) => config
            .commands
            .iter()
            .find(|c| c.cmd == alias)
            .ok_or_else(|| format!("Command alias '{alias}' not found in zoi.yaml"))?
            .clone(),
        None => {
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

    println!("\n--- Running command: {} ---", command_to_run.cmd.bold());
    executor::run_shell_command(&command_to_run.run)
}
