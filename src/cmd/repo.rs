use crate::pkg::config;
use clap::{Parser, Subcommand};
use colored::*;
use comfy_table::{presets::UTF8_FULL, Table};
use std::collections::HashSet;

#[derive(Parser)]
pub struct RepoCommand {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a repository to the configuration
    Add {
        /// The name of the repository to add
        repo_name: Option<String>,
    },
    /// Remove a repository from the configuration
    #[command(alias = "rm")]
    Remove {
        /// The name of the repository to remove
        repo_name: String,
    },
    /// List active or all available repositories
    List {
        /// Use 'all' to list all available repositories
        #[arg(value_name = "all")]
        all: Option<String>,
    },
}

pub fn run(args: RepoCommand) {
    match args.command {
        Commands::Add { repo_name } => {
            if let Some(name) = repo_name {
                if let Err(e) = config::add_repo(&name) {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                } else {
                    println!("Repository '{}' added successfully.", name.green());
                }
            } else if let Err(e) = config::interactive_add_repo() {
                eprintln!("{}: {}", "Error".red().bold(), e);
            }
        }
        Commands::Remove { repo_name } => {
            if let Err(e) = config::remove_repo(&repo_name) {
                eprintln!("{}: {}", "Error".red().bold(), e);
            } else {
                println!("Repository '{}' removed successfully.", repo_name.green());
            }
        }
        Commands::List { all } => {
            if all.is_some() {
                if let Err(e) = run_list_all() {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                }
            } else if let Err(e) = run_list_active() {
                eprintln!("{}: {}", "Error".red().bold(), e);
            }
        }
    }
}

fn run_list_active() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::read_config()?;
    if config.repos.is_empty() {
        println!("No active repositories.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Active Repositories"]);
    for repo in config.repos {
        table.add_row(vec![repo]);
    }
    println!("{table}");
    Ok(())
}

fn run_list_all() -> Result<(), Box<dyn std::error::Error>> {
    let active_repos = config::read_config()?
        .repos
        .into_iter()
        .collect::<HashSet<_>>();
    let all_repos = config::get_all_repos()?;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Status", "Repository"]);

    for repo in all_repos {
        let status = if active_repos.contains(&repo) {
            "Added"
        } else {
            ""
        };
        table.add_row(vec![status.to_string(), repo]);
    }
    println!("{table}");
    Ok(())
}
