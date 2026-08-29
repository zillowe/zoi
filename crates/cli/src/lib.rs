//! Zoi CLI library.
//!
//! This crate provides the command-line interface logic for Zoi, including
//! command parsing, execution, and utility functions for interacting with Zoi.

/// Command-line argument parsing and definitions.
pub mod cli;
/// Implementation of CLI commands.
pub mod cmd;
/// Package-related CLI logic.
pub mod pkg;
/// Project management.
pub use zoi_project as project;
/// CLI-specific utility functions.
pub mod utils;

use anyhow::Result;
pub use pkg::{local, mini_resolve};
pub use zoi_common::SourceInstallOptions;
pub use zoi_core::types::{self, Scope};
pub use zoi_core::{
    cache, config, hash, lock, offline, pgp, pin, pkgdir, recorder, sysroot,
    upgrade
};
pub use zoi_hooks as hooks;
pub use zoi_lua as lua;
pub use zoi_purl as purl;
pub use zoi_resolver as resolve;
#[cfg(target_os = "linux")]
pub use zoi_sandbox as sandbox;
pub use zoi_telemetry as telemetry;

/// Converts a core `Scope` to a CLI `InstallScope`.
fn to_install_scope(scope: Scope) -> cli::InstallScope {
    match scope {
        Scope::User => cli::InstallScope::User,
        Scope::System => cli::InstallScope::System,
        Scope::Project => cli::InstallScope::Project
    }
}

/// Installs one or more packages from source strings (PURLs or names).
///
/// # Errors
///
/// Returns an error if:
/// - The plugin manager fails to initialize.
/// - The installation process fails.
pub fn install_sources(
    sources: &[String],
    options: &SourceInstallOptions
) -> Result<()> {
    let _lock = lock::acquire_lock()?;
    let plugin_manager = if crate::pkg::utils::is_mini_mode() {
        None
    } else {
        let pm = pkg::plugin::PluginManager::new()?;
        let _ = pm.load_all(options.yes);
        Some(pm)
    };

    let pm_ptr = plugin_manager.as_ref();

    cmd::install::run(
        sources,
        options.repo.clone(),
        options.force,
        options.all_optional,
        options.yes,
        options.scope_override.map(to_install_scope),
        false,
        false,
        options.save,
        false,
        false,
        options.build_type.as_deref(),
        options.dry_run,
        pm_ptr,
        options.build,
        options.frozen,
        false,
        false,
        3,
        false,
        false,
        None
    )
}

/// Uninstalls a package by name.
///
/// # Errors
///
/// Returns an error if the uninstallation process fails.
pub fn uninstall_package(
    package_name: &str,
    scope_override: Option<Scope>
) -> Result<()> {
    let _lock = lock::acquire_lock()?;
    zoi_uninstall::run(package_name, scope_override, false, false, false)
        .map(|_| ())
}
