//! Logic for the `home` command.
//!
//! This module provides commands for managing user-specific declarative
//! configuration, including dotfiles and user-level package installations.

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::Colorize;
use zoi_core::utils::is_zoios;
use zoi_system::home::{apply_home_config, load_home_lua};

/// The root home management command.
#[derive(Parser, Debug)]
pub struct HomeCommand {
    /// The specific home subcommand to execute.
    #[command(subcommand)]
    pub command: HomeSubcommands,
}

/// Available home subcommands.
#[derive(Subcommand, Debug)]
pub enum HomeSubcommands {
    /// Apply a declarative user configuration from home.lua
    Apply {
        /// Path to the home configuration file
        #[arg(short, long)]
        file: Option<String>,
    },
}

/// Run the home management command.
///
/// # Errors
///
/// Returns an error if not on a `ZoiOS` system, if the home configuration cannot be
/// loaded or applied, or if user packages fail to install.
pub fn run(args: HomeCommand) -> Result<()> {
    if !is_zoios() {
        return Err(anyhow!(
            "'zoi home' features are only available on ZoiOS systems."
        ));
    }

    match args.command {
        HomeSubcommands::Apply { file } => {
            let config_path = if let Some(f) = file {
                f
            } else {
                let mut p = crate::pkg::utils::get_user_home()
                    .ok_or_else(|| anyhow!("Could not determine user home directory."))?;
                p.push(".config/zoi/home.lua");
                p.to_string_lossy().to_string()
            };

            println!("Reading user configuration from {}...", config_path.cyan());
            let config = load_home_lua(&config_path)?;

            // Install user packages
            if !config.packages.is_empty() {
                println!(
                    "{} Installing {} user packages...",
                    "::".bold().blue(),
                    config.packages.len().to_string().cyan()
                );

                let project_config = zoi_project::config::ProjectConfig {
                    name: "home".to_string(),
                    registries: std::collections::HashMap::new(),
                    packages: Vec::new(),
                    pkgs: config.packages.clone(),
                    pkgs_v2: config.packages_v2.clone(),
                    config: zoi_project::config::ProjectLocalConfig::default(),
                    commands: Vec::new(),
                    environments: Vec::new(),
                    shell: Some(zoi_project::config::ShellSpec::default()),
                };

                crate::cmd::install::run(
                    &config.packages,
                    None,
                    false,
                    false,
                    true, // yes
                    Some(crate::cli::InstallScope::User),
                    false,
                    false,
                    false,
                    None,
                    false,
                    None,
                    false,
                    false,
                    false,
                    false,
                    3,
                    false,
                    false,
                    Some(project_config),
                )?;
            }

            // Apply dotfiles and env
            apply_home_config(&config)?;
            println!("{}", "User environment applied successfully.".green());
        }
    }

    Ok(())
}
