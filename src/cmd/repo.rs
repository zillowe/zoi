use crate::pkg::config;
use clap::{Parser, Subcommand};
use colored::*;
use comfy_table::{Table, presets::UTF8_FULL};
use std::collections::HashSet;

#[derive(Parser)]
pub struct RepoCommand {
    #[arg(
        short = 'y',
        long,
        help = "Automatically answer yes to all prompts",
        global = true
    )]
    yes: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a repository to the configuration or clone from a git URL
    Add {
        /// The name of the repository to add or a git URL to clone
        repo_or_url: Option<String>,
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
    let yes = args.yes;
    match args.command {
        Commands::Add { repo_or_url } => {
            if let Some(val) = repo_or_url {
                if val.starts_with("http://")
                    || val.starts_with("https://")
                    || val.ends_with(".git")
                {
                    if let Err(e) = config::clone_git_repo(&val) {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                } else {
                    if let Err(e) = config::add_repo(&val) {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    } else {
                        println!("Repository '{}' added successfully.", val.green());
                    }
                }
            } else if !yes {
                if let Err(e) = config::interactive_add_repo() {
                    eprintln!("{}: {}", "Error".red().bold(), e);
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
