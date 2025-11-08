use crate::pkg::config;
use anyhow::Result;
use anyhow::anyhow;
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
    #[command(alias = "a")]
    Add {
        /// The name of the repository to add or a git URL to clone
        repo_or_url: Option<String>,
    },
    /// Remove a repository from the active configuration
    #[command(alias = "rm")]
    Remove { repo_name: String },
    /// List repositories (active by default); use `list all` to show all
    #[command(alias = "ls")]
    List {
        #[command(subcommand)]
        which: Option<ListSub>,
    },
    /// Manage cloned git repositories
    #[command(subcommand)]
    Git(GitCommand),
}

pub fn run(args: RepoCommand) -> Result<()> {
    let yes = args.yes;
    match args.command {
        Commands::Add { repo_or_url } => {
            if let Some(val) = repo_or_url {
                if val.starts_with("http://")
                    || val.starts_with("https://")
                    || val.ends_with(".git")
                {
                    config::clone_git_repo(&val)?;
                } else {
                    config::add_repo(&val)?;
                    println!("Repository '{}' added successfully.", val.green());
                }
            } else if yes {
                return Err(anyhow!(
                    "A repository name or URL is required when using --yes."
                ));
            } else {
                config::interactive_add_repo()?;
            }
        }
        Commands::Remove { repo_name } => {
            config::remove_repo(&repo_name)?;
            println!("Repository '{}' removed successfully.", repo_name.green());
        }
        Commands::List { which } => match which {
            None => run_list_active()?,
            Some(ListSub::All) => run_list_all()?,
        },
        Commands::Git(cmd) => match cmd {
            GitCommand::List => run_list_git_only()?,
            GitCommand::Rm { repo_name } => config::remove_git_repo(&repo_name)?,
        },
    }
    Ok(())
}

fn run_list_active() -> Result<()> {
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

fn run_list_all() -> Result<()> {
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
        let status = if active_repos.contains(&repo.to_lowercase()) {
            "Added"
        } else {
            ""
        };
        table.add_row(vec![status.to_string(), repo]);
    }
    println!("{table}");
    Ok(())
}

#[derive(Subcommand)]
enum ListSub {
    /// Show all available repositories (active + discovered)
    All,
}

#[derive(Subcommand)]
enum GitCommand {
    /// Show only cloned git repositories (~/.zoi/pkgs/git)
    #[command(alias = "ls")]
    List,
    /// Remove a cloned git repository directory (~/.zoi/pkgs/git/<repo-name>)
    Rm { repo_name: String },
}

fn run_list_git_only() -> Result<()> {
    let repos = config::list_git_repos()?;
    if repos.is_empty() {
        println!("No cloned git repositories.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Cloned Git Repositories (~/.zoi/pkgs/git)"]);
    for repo in repos {
        table.add_row(vec![repo]);
    }
    println!("{table}");
    Ok(())
}
