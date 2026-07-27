use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::*;
use zoi_core::utils::is_zoios;
use zoi_system::home::{apply_home_config, load_home_lua};

#[derive(Parser, Debug)]
pub struct HomeCommand {
    #[command(subcommand)]
    pub command: HomeSubcommands,
}

#[derive(Subcommand, Debug)]
pub enum HomeSubcommands {
    /// Apply a declarative user configuration from home.lua
    Apply {
        /// Path to the home configuration file
        #[arg(short, long)]
        file: Option<String>,
    },
}

pub fn run(args: HomeCommand) -> Result<()> {
    if !is_zoios() {
        return Err(anyhow!(
            "'zoi home' features are only available on ZoiOS systems."
        ));
    }

    match args.command {
        HomeSubcommands::Apply { file } => {
            let config_path = file.unwrap_or_else(|| {
                let mut p = home::home_dir().unwrap();
                p.push(".config/zoi/home.lua");
                p.to_string_lossy().to_string()
            });

            println!("Reading user configuration from {}...", config_path.cyan());
            let config = load_home_lua(&config_path)?;

            // Install user packages
            if !config.packages.is_empty() {
                println!(
                    "{} Installing {} user packages...",
                    "::".bold().blue(),
                    config.packages.len().to_string().cyan()
                );
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
                )?;
            }

            // Apply dotfiles and env
            apply_home_config(&config)?;
            println!("{}", "User environment applied successfully.".green());
        }
    }

    Ok(())
}
