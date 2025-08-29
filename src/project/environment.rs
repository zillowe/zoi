use super::{config, executor};
use crate::utils;
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use std::error::Error;
use std::process::Command;

pub fn setup(
    env_alias: Option<&str>,
    config: &config::ProjectConfig,
) -> Result<(), Box<dyn Error>> {
    if config.environments.is_empty() {
        return Err("No environments defined in zoi.yaml".into());
    }

    let env_to_setup = match env_alias {
        Some(alias) => config
            .environments
            .iter()
            .find(|e| e.cmd == alias)
            .ok_or_else(|| format!("Environment '{alias}' not found in zoi.yaml"))?
            .clone(),
        None => {
            let selections: Vec<&str> = config
                .environments
                .iter()
                .map(|e| e.name.as_str())
                .collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose an environment to set up")
                .items(&selections)
                .default(0)
                .interact_opt()?
                .ok_or("No environment chosen.")?;

            config.environments[selection].clone()
        }
    };

    println!(
        "\n--- Setting up environment: {} ---",
        env_to_setup.name.bold()
    );

    check_packages(config)?;

    let platform = utils::get_platform()?;

    let run_cmds = match &env_to_setup.run {
        config::PlatformOrStringVec::StringVec(v) => v.clone(),
        config::PlatformOrStringVec::Platform(p) => p
            .get(&platform)
            .or_else(|| p.get("default"))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "No commands found for platform '{}' and no default specified",
                    platform
                )
            })?,
    };

    let env_vars = match &env_to_setup.env {
        config::PlatformOrEnvMap::EnvMap(m) => m.clone(),
        config::PlatformOrEnvMap::Platform(p) => p
            .get(&platform)
            .or_else(|| p.get("default"))
            .cloned()
            .unwrap_or_default(),
    };

    for cmd_str in &run_cmds {
        executor::run_shell_command(cmd_str, &env_vars)?;
    }

    Ok(())
}

fn check_packages(config: &config::ProjectConfig) -> Result<(), Box<dyn Error>> {
    if config.packages.is_empty() {
        return Ok(());
    }
    println!("\nChecking required packages...");
    let mut all_ok = true;
    for package in &config.packages {
        print!("- Checking for '{}': ", package.name.cyan());
        let status = Command::new("bash")
            .arg("-c")
            .arg(&package.check)
            .output()?;
        if status.status.success() {
            println!("{}", "OK".green());
        } else {
            println!("{}", "MISSING".red());
            all_ok = false;
        }
    }
    if !all_ok {
        return Err("One or more required packages are missing.".into());
    }
    Ok(())
}
