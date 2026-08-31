//! Command for adding and installing packages to a project or global
//! configuration.

use std::collections::HashMap;

use anyhow::Result;
use colored::Colorize;

use crate::pkg::{config, types};

/// Runs the 'use' command.
///
/// Adds the specified packages to either the global or project configuration
/// and triggers an installation.
///
/// # Errors
///
/// Returns an error if:
/// - A package name cannot be parsed.
/// - Configuration update fails.
/// - The installation process fails.
/// - In project mode, 'zoi.lua' is missing.
pub fn run(packages: &[String], global: bool) -> Result<()> {
    if global {
        run_global(packages)
    } else {
        run_project(packages)
    }
}

/// Updates the global configuration with the specified packages and installs
/// them.
fn run_global(packages: &[String]) -> Result<()> {
    println!(
        "{} Adding packages to global configuration...",
        "::".bold().blue()
    );

    let mut versions_to_add = HashMap::new();
    for pkg_spec in packages {
        let request = crate::pkg::resolve::parse_source_string(pkg_spec)?;
        let version =
            request.version_spec.unwrap_or_else(|| "latest".to_string());
        versions_to_add.insert(request.name, version);
    }

    config::update_global_versions(versions_to_add)?;

    println!("{} Installing global packages...", "::".bold().blue());
    let options = crate::SourceInstallOptions {
        scope_override: Some(types::Scope::User),
        yes: true,
        ..Default::default()
    };

    crate::install_sources(packages, &options)?;

    println!("\n{}", "Global packages updated and installed.".green());
    Ok(())
}

/// Installs project packages, persisting them to 'zoi.yaml' when the project
/// uses a declarative config, or instructing the user for scriptable 'zoi.lua'
/// configs whose automatic saving is not supported.
fn run_project(packages: &[String]) -> Result<()> {
    if std::path::Path::new("zoi.lua").exists() {
        println!(
            "{} Project uses zoi.lua. Automatic saving is not supported for \
             Lua configurations.",
            "Note:".yellow().bold()
        );
        println!(
            "   Please add the following to your packages() block in zoi.lua:"
        );
        for pkg in packages {
            println!("   - \"{pkg}\"");
        }
    } else {
        crate::project::config::add_packages_to_config(packages)?;
    }

    println!("{} Installing project packages...", "::".bold().blue());
    let options = crate::SourceInstallOptions {
        scope_override: Some(types::Scope::Project),
        yes: true,
        ..Default::default()
    };

    crate::install_sources(packages, &options)?;

    println!("\n{}", "Project packages updated and installed.".green());
    Ok(())
}
