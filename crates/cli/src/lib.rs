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

pub use zoi_core::cache;
pub use zoi_core::config;
pub use zoi_core::hash;
pub use zoi_core::lock;
pub use zoi_core::offline;
pub use zoi_core::pgp;
pub use zoi_core::pin;
pub use zoi_core::pkgdir;
pub use zoi_core::recorder;
pub use zoi_core::sysroot;
pub use zoi_core::types::{self, Scope};
pub use zoi_core::upgrade;
pub use zoi_hooks as hooks;
pub use zoi_lua as lua;
pub use zoi_purl as purl;
pub use zoi_resolver as resolve;
#[cfg(target_os = "linux")]
pub use zoi_sandbox as sandbox;
pub use zoi_telemetry as telemetry;

pub use pkg::local;
pub use pkg::mini_resolve;

/// Options for installing packages from source.
#[derive(Debug, Clone, Default)]
pub struct SourceInstallOptions {
    /// The repository to install from.
    pub repo: Option<String>,
    /// Whether to force the installation.
    pub force: bool,
    /// Whether to install all optional dependencies.
    pub all_optional: bool,
    /// Whether to skip confirmation prompts.
    pub yes: bool,
    /// Override the installation scope.
    pub scope_override: Option<Scope>,
    /// Whether to save the installation to the project file.
    pub save: bool,
    /// The build type to use.
    pub build_type: Option<String>,
    /// Whether to perform a dry run.
    pub dry_run: bool,
    /// Whether to build the package.
    pub build: bool,
    /// Whether to use the lockfile exactly (frozen).
    pub frozen: bool,
}

/// Converts a core `Scope` to a CLI `InstallScope`.
fn to_install_scope(scope: Scope) -> cli::InstallScope {
    match scope {
        Scope::User => cli::InstallScope::User,
        Scope::System => cli::InstallScope::System,
        Scope::Project => cli::InstallScope::Project,
    }
}

/// Installs one or more packages from source strings (PURLs or names).
///
/// # Errors
///
/// Returns an error if:
/// - The plugin manager fails to initialize.
/// - The installation process fails.
pub fn install_sources(sources: &[String], options: &SourceInstallOptions) -> Result<()> {
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
        None,
    )
}

/// Uninstalls a package by name.
///
/// # Errors
///
/// Returns an error if the uninstallation process fails.
pub fn uninstall_package(package_name: &str, scope_override: Option<Scope>) -> Result<()> {
    zoi_uninstall::run(package_name, scope_override, false, false, false).map(|_| ())
}
