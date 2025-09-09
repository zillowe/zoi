//! Zoi: The Universal Package Manager & Environment Setup Tool.
//!
//! This crate provides the core functionality of Zoi as a library, allowing other
//! Rust applications to leverage its package management and environment setup
//! capabilities.
//!
//! # Key Features
//!
//! - **Package Management:** Install, update, uninstall, and manage packages from
//!   various sources.
//! - **Dependency Resolution:** Automatically handle complex dependency graphs.
//! - **Environment Setup:** Configure project environments and run tasks.
//! - **Extensibility:** Use extensions to add new repositories and functionality.
//!
//! # Getting Started
//!
//! To use Zoi as a library, add it as a dependency in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! zoi = { version = "5.0.0-beta" }
//! ```
//!
//! # Examples
//!
//! ## Installing a package
//!
//! ```no_run
//! use zoi::{install, InstallMode, types::InstallReason};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let source = "hello";
//!     let mode = InstallMode::PreferBinary;
//!     let force = false;
//!     let reason = InstallReason::Direct;
//!     let non_interactive = true;
//!
//!     zoi::install(source, mode, force, reason, non_interactive)?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Listing installed packages
//!
//! ```no_run
//! use zoi::get_installed_packages;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let installed_packages = get_installed_packages()?;
//!     for pkg in installed_packages {
//!         println!("- {}@{}", pkg.name, pkg.version);
//!     }
//!     Ok(())
//! }
//! ```

// Make all modules public for library usage
pub mod cli;
pub mod cmd;
pub mod pkg;
pub mod project;
pub mod utils;

// Re-export key types and functions for easier use.
pub use pkg::install::InstallMode;
pub use pkg::pin::PinnedPackage;
pub use pkg::types::{
    self, Author, Checksums, Config, Dependencies, InstallManifest, InstallReason,
    InstallationMethod, Maintainer, Package, PackageType, Scope,
};
pub use pkg::update::UpdateResult;

use std::collections::HashSet;
use std::error::Error;

// --- Core Package Management Functions ---

/// Installs a Zoi package.
///
/// This is the main entry point for using Zoi's installation logic as a library.
///
/// # Arguments
///
/// * `source` - The package source identifier (e.g. "package-name", "@repo/package-name", "http://...", "/path/to/file.pkg.lua").
/// * `mode` - The installation mode to use.
/// * `force` - If true, forces re-installation even if the package is already installed.
/// * `reason` - The reason for installation (direct user action or as a dependency).
/// * `non_interactive` - If true, automatically confirms any prompts (answers "yes").
///
/// # Returns
///
/// `Ok(())` on success, or an error if installation fails.
pub fn install(
    source: &str,
    mode: InstallMode,
    force: bool,
    reason: InstallReason,
    non_interactive: bool,
) -> Result<(), Box<dyn Error>> {
    // Internally, run_installation uses this to track dependencies and avoid cycles.
    // For a top-level install call, we start with an empty set.
    let mut processed_deps = HashSet::new();
    pkg::install::run_installation(
        source,
        mode,
        force,
        reason,
        non_interactive,
        false, // all_optional
        &mut processed_deps,
        None, // scope_override
    )
}

/// Uninstalls a Zoi package.
///
/// # Arguments
///
/// * `package_name` - The name of the package to uninstall.
///
/// # Returns
///
/// `Ok(())` on success, or an error if uninstallation fails.
pub fn uninstall(package_name: &str) -> Result<(), Box<dyn Error>> {
    pkg::uninstall::run(package_name)
}

/// Updates a Zoi package if a new version is available.
///
/// This function checks the installed version of a package against the latest
/// available version. If they differ, it performs an update.
///
/// # Arguments
///
/// * `source` - The package source identifier (e.g. "package-name").
/// * `non_interactive` - If true, automatically confirms any prompts (answers "yes").
///
/// # Returns
///
/// A `Result` containing an `UpdateResult` enum on success, or an error if the
/// update fails.
pub fn update(source: &str, non_interactive: bool) -> Result<UpdateResult, Box<dyn Error>> {
    pkg::update::run(source, non_interactive)
}

/// Upgrades the Zoi binary itself to the latest version.
///
/// # Arguments
///
/// * `force` - If true, forces the upgrade even if the version is the same.
///
/// # Returns
///
/// `Ok(())` on success, or an error if the upgrade fails.
pub fn upgrade(force: bool) -> Result<(), Box<dyn Error>> {
    // These are the default values from the CLI for a standard upgrade.
    let branch = "Production";
    let status = "Beta";
    let number = env!("CARGO_PKG_VERSION");
    let full = false;
    let tag = None;
    let custom_branch = None;
    pkg::upgrade::run(branch, status, number, full, force, tag, custom_branch)
}

/// Synchronizes the local package database with the remote repository.
///
/// # Arguments
///
/// * `verbose` - If true, shows detailed git output.
///
/// # Returns
///
/// `Ok(())` on success, or an error if the sync fails.
pub fn sync(verbose: bool) -> Result<(), Box<dyn Error>> {
    pkg::sync::run(verbose, true, false)
}

/// Removes packages that were installed as dependencies but are no longer needed.
///
/// # Arguments
///
/// * `non_interactive` - If true, automatically confirms any prompts (answers "yes").
///
/// # Returns
///
/// `Ok(())` on success, or an error if the cleanup fails.
pub fn autoremove(non_interactive: bool) -> Result<(), Box<dyn Error>> {
    pkg::autoremove::run(non_interactive)
}

// --- Package Information Functions ---

/// Resolves a package source string to a `Package` struct.
///
/// This function finds the package but does not install it. It's useful for
/// inspecting package metadata.
///
/// # Arguments
///
/// * `source` - The package source identifier.
///
/// # Returns
///
/// A `Result` containing the resolved `Package` on success, or an error.
pub fn resolve_package(source: &str) -> Result<Package, Box<dyn Error>> {
    let (pkg, _, _, _) = pkg::resolve::resolve_package_and_version(source)?;
    Ok(pkg)
}

/// Checks if a package is installed.
///
/// # Arguments
///
/// * `package_name` - The name of the package.
/// * `scope` - The scope (User or System) to check.
///
/// # Returns
///
/// A `Result` containing an `Option<InstallManifest>`. `Some` contains the
/// manifest if installed, `None` otherwise.
pub fn is_package_installed(
    package_name: &str,
    scope: Scope,
) -> Result<Option<InstallManifest>, Box<dyn Error>> {
    pkg::local::is_package_installed(package_name, scope)
}

/// Gets a list of all installed packages.
///
/// # Returns
///
/// A `Result` containing a `Vec<InstallManifest>` of all installed packages.
pub fn get_installed_packages() -> Result<Vec<InstallManifest>, Box<dyn Error>> {
    pkg::local::get_installed_packages()
}

/// Gets a list of all available packages from the synchronized repositories.
///
/// # Returns
///
/// A `Result` containing a `Vec<Package>` of all available packages.
pub fn get_all_available_packages() -> Result<Vec<Package>, Box<dyn Error>> {
    pkg::local::get_all_available_packages()
}

// --- Pinning Functions ---

/// Pins a package to a specific version.
///
/// # Arguments
///
/// * `package_name` - The name of the package to pin.
/// * `version` - The version string to pin the package to.
///
/// # Returns
///
/// `Ok(())` on success, or an error.
pub fn pin(package_name: &str, version: &str) -> Result<(), Box<dyn Error>> {
    let mut pinned_packages = pkg::pin::get_pinned_packages()?;
    if pinned_packages.iter().any(|p| p.name == package_name) {
        return Err(format!("Package '{}' is already pinned.", package_name).into());
    }
    let new_pin = PinnedPackage {
        name: package_name.to_string(),
        version: version.to_string(),
    };
    pinned_packages.push(new_pin);
    pkg::pin::write_pinned_packages(&pinned_packages)?;
    Ok(())
}

/// Unpins a package, allowing it to be updated.
///
/// # Arguments
///
/// * `package_name` - The name of the package to unpin.
///
/// # Returns
///
/// `Ok(())` on success, or an error.
pub fn unpin(package_name: &str) -> Result<(), Box<dyn Error>> {
    let mut pinned_packages = pkg::pin::get_pinned_packages()?;
    let initial_len = pinned_packages.len();
    pinned_packages.retain(|p| p.name != package_name);
    if pinned_packages.len() == initial_len {
        return Err(format!("Package '{}' was not pinned.", package_name).into());
    }
    pkg::pin::write_pinned_packages(&pinned_packages)?;
    Ok(())
}

/// Gets a list of all pinned packages.
///
/// # Returns
///
/// A `Result` containing a `Vec<PinnedPackage>`.
pub fn get_pinned_packages() -> Result<Vec<PinnedPackage>, Box<dyn Error>> {
    Ok(pkg::pin::get_pinned_packages()?)
}
