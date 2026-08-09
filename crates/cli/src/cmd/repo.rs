use crate::pkg::config;
use anyhow::Result;
use anyhow::anyhow;
use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_FULL};
use std::collections::HashSet;

/// Arguments for the `repo` command.
#[derive(Parser)]
pub struct RepoCommand {
    /// Automatically answer yes to all prompts
    #[arg(
        short = 'y',
        long,
        help = "Automatically answer yes to all prompts",
        global = true
    )]
    yes: bool,
    /// The repository sub-command to run.
    #[command(subcommand)]
    command: Commands,
}

/// Available repository sub-commands.
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
    Remove {
        /// The name of the repository to remove
        repo_name: String,
    },
    /// List repositories (active by default); use `list all` to show all
    #[command(alias = "ls")]
    List {
        /// Which repositories to list
        #[command(subcommand)]
        which: Option<ListSub>,
    },
    /// Manage cloned git repositories
    #[command(subcommand)]
    Git(GitCommand),
}

/// Runs the `repo` command.
///
/// # Errors
///
/// This function returns an error if:
/// - A repository name or URL is missing when running non-interactively with `--yes`.
/// - Adding, cloning, or removing a repository fails.
/// - The configuration file cannot be read or modified.
/// # Errors
///
/// Returns an error if the repository operation fails.
pub fn run(args: RepoCommand) -> Result<()> {
    let yes = args.yes;
    match args.command {
        Commands::Add { repo_or_url } => {
            if let Some(val) = repo_or_url {
                if val.starts_with("http://")
                    || val.starts_with("https://")
                    || std::path::Path::new(&val)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("git"))
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

/// Lists active repositories.
fn run_list_active() -> Result<()> {
    let config = config::read_config()?;
    if config.repos.is_empty() {
        println!("No active repositories.");
        return Ok(());
    }

    println!("{} Active repositories:", "::".bold().blue());
    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec!["Repository"]);
    for repo in config.repos {
        table.add_row(vec![repo]);
    }
    println!("{table}");
    Ok(())
}

/// Lists all available repositories.
fn run_list_all() -> Result<()> {
    let active_repos = config::read_config()?
        .repos
        .into_iter()
        .collect::<HashSet<_>>();
    let all_repos = config::get_all_repos()?;

    println!("{} All available repositories:", "::".bold().blue());
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

/// Options for listing repositories.
#[derive(Subcommand)]
enum ListSub {
    /// Show all available repositories (active + discovered)
    All,
}

/// Available git repository sub-commands.
#[derive(Subcommand)]
enum GitCommand {
    /// Show only cloned git repositories (~/.zoi/pkgs/git)
    #[command(alias = "ls")]
    List,
    /// Remove a cloned git repository directory (~/.zoi/pkgs/git/<repo-name>)
    Rm {
        /// The name of the repository to remove
        repo_name: String,
    },
}

/// Lists only cloned git repositories.
fn run_list_git_only() -> Result<()> {
    let repos = config::list_git_repos()?;
    if repos.is_empty() {
        println!("No cloned git repositories.");
        return Ok(());
    }

    println!(
        "{} Cloned git repositories (~/.zoi/pkgs/git):",
        "::".bold().blue()
    );
    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec!["Repository"]);
    for repo in repos {
        table.add_row(vec![repo]);
    }
    println!("{table}");
    Ok(())
}
