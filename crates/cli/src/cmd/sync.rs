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
    scope: Option<SetupScope>
) -> Result<()> {
    println!("{} Syncing package databases...", "::".bold().blue());

    if force {
        println!(
            "{} Force sync enabled, removing existing databases...",
            "::".bold().yellow()
        );
    }

    let pkg_scope = match scope {
        Some(SetupScope::User) => Some(crate::pkg::types::Scope::User),
        Some(SetupScope::System) => Some(crate::pkg::types::Scope::System),
        None => None
    };

    pkg::sync::run(verbose, fallback, no_pm, force, pkg_scope)?;

    println!("{}", "Sync complete.".green());
    Ok(())
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
    frozen: bool
) -> Result<()> {
    if frozen {
        crate::pkg::frozen::set_frozen(true);
    }
    println!(
        "{} Syncing project-local package databases...",
        "::".bold().blue()
    );

    pkg::sync::run_local(verbose, fallback, force, frozen)?;

    println!("{}", "Local sync complete.".green());
    Ok(())
}

/// Print a warning for non-official (third-party) registries.
fn warn_third_party(name: &str, registry_type: &str) {
    if registry_type != "official" {
        eprintln!(
            "{} {} is a third-party registry. Use with caution and verify its \
             authenticity before installing packages.",
            "!".yellow().bold(),
            name.cyan()
        );
    }
}

/// Set the default registry by handle or URL.
///
/// When a built-in registry handle is given, the registry is resolved from the
/// pre-defined YAML and its metadata is used. A local directory or an HTTP URL
/// is also accepted. The `default` keyword resolves to the built-in `set: true`
/// registry.
///
/// Only one built-in registry may be marked as the "set" (default) registry at
/// a time.
///
/// # Errors
///
/// Returns an error if the configuration cannot be updated or the one-set rule
/// is violated.
pub fn set_registry(url_or_handle: &str) -> Result<()> {
    // Resolve the "default" keyword to the built-in `set: true` registry.
    let handle_or_url_storage;
    let handle_or_url = if url_or_handle == "default" {
        handle_or_url_storage = pkg::builtin_registries::get_set()?
            .map_or_else(|| "zoidberg".to_string(), |r| r.handle);
        &handle_or_url_storage
    } else {
        url_or_handle
    };

    // --- resolve built-in if possible ---
    let builtin = pkg::builtin_registries::get(handle_or_url);

    // Print the third-party warning for the resolved registry
    if let Some(ref reg) = builtin {
        warn_third_party(&reg.name, &reg.registry_type);
    }

    // The config helper (set_default_registry) handles writing the user config;
    // if the handle matches a built-in registry it populates the metadata
    // fields automatically.
    pkg::config::set_default_registry(handle_or_url)?;

    // Determine what to display to the user
    let (display_handle, display_url) = match builtin {
        Some(ref reg) => (reg.handle.as_str(), reg.git.as_str()),
        None => (handle_or_url, handle_or_url)
    };

    println!("Default registry set to: {}", display_handle.cyan());
    if !display_url.is_empty() && display_url != display_handle {
        println!("  Git: {display_url}");
    }
    println!("The new registry will be used the next time you run 'zoi sync'");
    Ok(())
}

/// Add a new registry by handle or URL.
///
/// When a built-in registry handle is given (e.g. `docker`), the registry is
/// resolved from the pre-defined YAML and its metadata is used. A local
/// directory or an HTTP URL is also accepted. The `default` keyword resolves to
/// the built-in `set: true` registry.
///
/// # Errors
///
/// Returns an error if the directory path is invalid or if the configuration
/// cannot be updated.
pub fn add_registry(handle_or_url: &str) -> Result<()> {
    // Resolve the "default" keyword to the built-in `set: true` registry.
    let resolved_storage;
    let handle_or_url = if handle_or_url == "default" {
        resolved_storage = pkg::builtin_registries::get_set()?
            .map_or_else(|| "zoidberg".to_string(), |r| r.handle);
        &resolved_storage
    } else {
        handle_or_url
    };

    // Check for local directories first
    let path = std::path::Path::new(handle_or_url);
    if path.is_dir() {
        let final_url =
            std::fs::canonicalize(path)?.to_string_lossy().to_string();
        pkg::config::add_added_registry(&final_url)?;
        let url_cyan = final_url.cyan();
        println!("Registry '{url_cyan}' added.");
        println!("It will be synced on the next 'zoi sync' run.");
        return Ok(());
    }

    // Try resolving as a built-in handle
    if let Some(reg) = pkg::builtin_registries::get(handle_or_url) {
        warn_third_party(&reg.name, &reg.registry_type);

        pkg::config::add_added_registry(handle_or_url)?;
        println!("Registry {} added.", reg.handle.cyan());
        println!("  Name: {}", reg.name);
        println!("  Git: {}", reg.git);
        println!("It will be synced on the next 'zoi sync' run.");
        return Ok(());
    }

    // Fallback: treat as a raw URL
    pkg::config::add_added_registry(handle_or_url)?;
    let url_cyan = handle_or_url.cyan();
    println!("Registry '{url_cyan}' added.");
    println!("It will be synced on the next 'zoi sync' run.");
    Ok(())
}

/// Remove a registry by its handle or URL.
///
/// # Errors
///
/// Returns an error if the registry cannot be removed from the configuration.
pub fn remove_registry(handle: &str) -> Result<()> {
    pkg::config::remove_added_registry(handle)?;
    let handle_cyan = handle.cyan();
    println!("Registry '{handle_cyan}' removed.");
    Ok(())
}

/// List all configured registries and show which built-in registries are
/// available but not yet added.
///
/// Output format:
///
/// ```text
/// :: Configured Registries
/// [Set] zoidberg: Zoidberg
///       - Official Zoi packages repository
///       - Git: https://gitlab.com/zillowe/zillwen/zusty/zoidberg
/// [Added] registry-handle: Registry Name
///         - Registry description
///         - Git: https://github/registry/database
///
/// :: Available Registries
/// [Unadded] registry-handle: Registry Name
///           - Registry description
///           - Git: https://github/registry/database
/// ```
///
/// # Errors
///
/// Returns an error if the configuration cannot be read.
pub fn list_registries() -> Result<()> {
    let config = pkg::config::read_config()?;
    let all_builtins = pkg::builtin_registries::load_all();

    // Collect handles that are already configured (set or added)
    let mut configured_handles: Vec<&str> = Vec::new();

    println!("{} Configured Registries", "::".bold().blue());

    // --- set registry (default_registry) ---
    if let Some(ref default) = config.default_registry {
        let handle = &default.handle;
        let display_name = default.name.as_deref().unwrap_or(handle);
        let display_desc = default.description.as_deref();

        configured_handles.push(handle.as_str());

        let handle_str = if handle.is_empty() {
            "<not synced>".italic().to_string()
        } else {
            handle.cyan().to_string()
        };
        println!("[Set] {handle_str}: {}", display_name.white());
        if let Some(desc) = display_desc {
            println!("      - {desc}");
        }
        println!("      - Git: {}", default.url);
    } else {
        println!("[Set] {} not set", "<none>".dimmed());
    }

    // --- added registries ---
    for reg in &config.added_registries {
        let handle = &reg.handle;
        let display_name = reg.name.as_deref().unwrap_or(handle);
        let display_desc = reg.description.as_deref();

        configured_handles.push(handle.as_str());

        let handle_str = if handle.is_empty() {
            "<not synced>".italic().to_string()
        } else {
            handle.cyan().to_string()
        };
        println!();
        println!("[Added] {handle_str}: {}", display_name.white());
        if let Some(desc) = display_desc {
            println!("          - {desc}");
        }
        println!("          - Git: {}", reg.url);
    }

    // --- available but unadded builtins ---
    let unadded: Vec<_> = all_builtins
        .iter()
        .filter(|r| !configured_handles.contains(&r.handle.as_str()))
        .collect();

    if !unadded.is_empty() {
        println!();
        println!("{} Available Registries", "::".bold().blue());
        for reg in unadded {
            let handle_str = reg.handle.cyan().to_string();
            println!("[Unadded] {handle_str}: {}", reg.name.white());
            println!("          - {}", reg.description);
            println!("          - Git: {}", reg.git);
        }
    }

    Ok(())
}
