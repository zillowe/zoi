use super::{config, executor};
use anyhow::{Result, anyhow};
use clap_complete::Shell;
use colored::Colorize;
use dialoguer::{Select, theme::ColorfulTheme};
use std::collections::HashMap;
use std::io::{self, Write};
use std::process::Stdio;
use zoi_core::utils;

/// Sets up a project environment by running its specified commands.
///
/// If `env_alias` is provided, it tries to find and setup that specific environment.
/// Otherwise, it prompts the user to choose an environment interactively.
///
/// # Errors
///
/// Returns an error if no environments are defined, the specified environment is not found,
/// if package checks fail, or if executing setup commands fails.
pub fn setup(env_alias: Option<&str>, config: &config::ProjectConfig) -> Result<()> {
    if config.environments.is_empty() {
        return Err(anyhow!("No environments defined in zoi.yaml"));
    }

    let env_to_setup = if let Some(alias) = env_alias {
        config
            .environments
            .iter()
            .find(|e| e.cmd == alias)
            .ok_or_else(|| anyhow!("Environment '{alias}' not found in zoi.yaml"))?
            .clone()
    } else {
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
            .ok_or(anyhow!("No environment chosen."))?;

        config
            .environments
            .get(selection)
            .cloned()
            .ok_or_else(|| anyhow!("Invalid selection"))?
    };

    println!(
        "\n{} Setting up environment: {}...",
        "::".bold().blue(),
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
                anyhow!(
                    "No commands found for platform '{platform}' and no default specified"
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

/// Checks if all required packages defined in the configuration are present on the system.
fn check_packages(config: &config::ProjectConfig) -> Result<()> {
    if config.packages.is_empty() {
        return Ok(());
    }
    println!("\nChecking required packages...");
    let mut all_ok = true;
    for package in &config.packages {
        print!("- Checking for '{}': ", package.name.cyan());
        let _ = io::stdout().flush();

        let status = executor::get_shell_command(&package.check)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if status.success() {
            println!("{}", "OK".green());
        } else {
            println!("{}", "MISSING".red());
            all_ok = false;
        }
    }
    if !all_ok {
        return Err(anyhow!("One or more required packages are missing."));
    }
    Ok(())
}

/// Exports environment variables to the shell.
///
/// This prints shell-specific commands (e.g. `export` for Bash/Zsh) that can be
/// evaluated to set up the environment variables for the current project.
///
/// # Errors
///
/// Returns an error if the specified environment is not found or if there is
/// an issue determining the current platform or directory.
pub fn export_shell(
    env_alias: Option<&str>,
    config: &config::ProjectConfig,
    shell: Shell,
) -> Result<()> {
    let platform = utils::get_platform()?;
    let mut env_vars = HashMap::new();

    if let Some(alias) = env_alias {
        let env_spec = config
            .environments
            .iter()
            .find(|e| e.cmd == alias)
            .ok_or_else(|| anyhow!("Environment '{alias}' not found"))?;

        let extra_env = match &env_spec.env {
            config::PlatformOrEnvMap::EnvMap(m) => m.clone(),
            config::PlatformOrEnvMap::Platform(p) => p
                .get(&platform)
                .or_else(|| p.get("default"))
                .cloned()
                .unwrap_or_default(),
        };
        env_vars.extend(extra_env);
    }

    if config.config.local {
        let bin_dir = std::env::current_dir()?
            .join(".zoi")
            .join("pkgs")
            .join("bin");
        if bin_dir.exists() {
            let mut path = bin_dir.to_string_lossy().to_string();
            if let Ok(old_path) = std::env::var("PATH") {
                path = format!(
                    "{}{}{}",
                    path,
                    if cfg!(windows) { ";" } else { ":" },
                    old_path
                );
            }
            env_vars.insert("PATH".to_string(), path);
        }
    }

    for (k, v) in env_vars {
        match shell {
            Shell::Bash | Shell::Zsh => {
                println!("export {k}=\"{v}\"");
            }
            Shell::Fish => {
                println!("set -gx {k} \"{v}\"");
            }
            Shell::PowerShell => {
                println!("$env:{k} = \"{v}\"");
            }
            Shell::Elvish => {
                println!("set E:{k} = \"{v}\"");
            }
            _ => {}
        }
    }

    Ok(())
}
