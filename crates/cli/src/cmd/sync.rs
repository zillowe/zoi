//! Logic for the `sync` command.
//!
//! This module provides commands for syncing package databases from registries,
//! and managing the list of configured registries.

use anyhow::Result;
use colored::Colorize;

use crate::cli::SetupScope;
use crate::pkg;

/// Run the sync command to update package databases.
///
/// # Errors
///
/// Returns an error if the sync process fails.
pub fn run(
    verbose: bool,
    fallback: bool,
    no_pm: bool,
    force: bool,
    scope: Option<SetupScope,>,
) -> Result<(),> {
    println!("{} Syncing package databases...", "::".bold().blue());

    if force {
        println!(
            "{} Force sync enabled, removing existing databases...",
            "::".bold().yellow()
        );
    }

    let pkg_scope = match scope {
        Some(SetupScope::User,) => Some(crate::pkg::types::Scope::User,),
        Some(SetupScope::System,) => Some(crate::pkg::types::Scope::System,),
        None => None,
    };

    pkg::sync::run(verbose, fallback, no_pm, force, pkg_scope,)?;

    println!("{}", "Sync complete.".green());
    Ok((),)
}

/// Run a project-local sync command.
///
/// # Errors
///
/// Returns an error if the project-local sync process fails.
pub fn run_local(
    verbose: bool,
    fallback: bool,
    force: bool,
    frozen: bool,
) -> Result<(),> {
    if frozen {
        crate::pkg::frozen::set_frozen(true,);
    }
    println!(
        "{} Syncing project-local package databases...",
        "::".bold().blue()
    );

    pkg::sync::run_local(verbose, fallback, force, frozen,)?;

    println!("{}", "Local sync complete.".green());
    Ok((),)
}

/// Set the default registry URL or use a pre-defined keyword.
///
/// # Errors
///
/// Returns an error if the configuration cannot be updated.
pub fn set_registry(url_or_keyword: &str,) -> Result<(),> {
    let url_storage;
    let url = match url_or_keyword {
        "default" => {
            url_storage = pkg::config::get_default_registry();
            &url_storage
        }
        "gitlab" => "https://gitlab.com/zillowe/zillwen/zusty/zoidberg.git",
        "github" => "https://github.com/zillowe/zoidberg.git",
        "codeberg" => "https://codeberg.org/Zillowe/Zoidberg.git",
        _ => url_or_keyword,
    };

    pkg::config::set_default_registry(url,)?;
    let url_cyan = url.cyan();
    println!("Default registry set to: {url_cyan}");
    println!("The new registry will be used the next time you run 'zoi sync'");
    Ok((),)
}

/// Add a new registry URL to the list of tracked registries.
///
/// # Errors
///
/// Returns an error if the directory path is invalid or if the configuration
/// cannot be updated.
pub fn add_registry(url: &str,) -> Result<(),> {
    let mut final_url = url.to_string();
    let path = std::path::Path::new(url,);
    if path.is_dir() {
        final_url = std::fs::canonicalize(path,)?.to_string_lossy().to_string();
    }

    pkg::config::add_added_registry(&final_url,)?;
    let url_cyan = final_url.cyan();
    println!("Registry '{url_cyan}' added.");
    println!("It will be synced on the next 'zoi sync' run.");
    Ok((),)
}

/// Remove a registry by its handle or URL.
///
/// # Errors
///
/// Returns an error if the registry cannot be removed from the configuration.
pub fn remove_registry(handle: &str,) -> Result<(),> {
    pkg::config::remove_added_registry(handle,)?;
    let handle_cyan = handle.cyan();
    println!("Registry '{handle_cyan}' removed.");
    Ok((),)
}

/// List all configured and tracked registries.
///
/// # Errors
///
/// Returns an error if the configuration cannot be read.
pub fn list_registries() -> Result<(),> {
    let config = crate::pkg::config::read_config()?;
    let db_root = crate::pkg::resolve::get_db_root()?;

    println!("{} Configured Registries", "::".bold().blue());

    if let Some(default,) = config.default_registry {
        let handle = &default.handle;
        let mut desc = String::new();
        if !handle.is_empty() {
            let repo_path = db_root.join(handle,);
            if let Ok(repo_config,) =
                crate::pkg::config::read_repo_config(&repo_path,)
            {
                let repo_desc = &repo_config.description;
                desc = format!(" - {repo_desc}");
            }
        }
        let handle_str = if handle.is_empty() {
            "<not synced>".italic().to_string()
        } else {
            handle.cyan().to_string()
        };
        let url_cyan = default.url.cyan();
        let url = &default.url;
        println!("[Set] {handle_str}: {url}{url_cyan}");
        if !desc.is_empty() {
            let desc_dimmed = desc.dimmed();
            println!("      {desc_dimmed}");
        }
    } else {
        println!("[Set]: <not set>");
    }

    if !config.added_registries.is_empty() {
        println!();
        for reg in config.added_registries {
            let handle = &reg.handle;
            let mut desc = String::new();
            if !handle.is_empty() {
                let repo_path = db_root.join(handle,);
                if let Ok(repo_config,) =
                    crate::pkg::config::read_repo_config(&repo_path,)
                {
                    let repo_desc = &repo_config.description;
                    desc = format!(" - {repo_desc}");
                }
            }
            let handle_str = if handle.is_empty() {
                "<not synced>".italic().to_string()
            } else {
                handle.cyan().to_string()
            };
            let url_cyan = reg.url.cyan();
            let url = &reg.url;
            println!("[Add] {handle_str}: {url}{url_cyan}");
            if !desc.is_empty() {
                let desc_dimmed = desc.dimmed();
                println!("      {desc_dimmed}");
            }
        }
    }
    Ok((),)
}
