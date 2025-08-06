use super::{config::ProjectConfig, executor};
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use std::error::Error;
use std::process::Command;

pub fn setup(env_alias: Option<&str>, config: &ProjectConfig) -> Result<(), Box<dyn Error>> {
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

    for cmd_str in &env_to_setup.run {
        executor::run_shell_command(cmd_str)?;
    }

    Ok(())
}

fn check_packages(config: &ProjectConfig) -> Result<(), Box<dyn Error>> {
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
