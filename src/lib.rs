//! Zoi: The Universal Package Manager & Environment Setup Tool.
//!
//! This crate provides the core functionality of Zoi as a library, allowing other
//! Rust applications to leverage its package management and environment setup
//! capabilities.
//!
//! If you're looking for the user documentation visit [Zoi Docs](https://zillowe.qzz.io/docs/zds/zoi).
//! For better library docs visit [Zoi Library Docs](https://zillowe.qzz.io/docs/zds/zoi/lib).
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
//! To use Zoi as a library, add it using `cargo` or as a dependency in your `Cargo.toml`:
//!
//! ```sh
//! cargo add zoi-rs
//! ```
//!
//! ```toml
//! [dependencies]
//! zoi = { version = "1.0.0" } # subject to change
//! ```
//!
//! # Examples
//!
//! ## Installing a package
//!
//! ```no_run
//! use zoi::{install, types::InstallReason};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let source = "hello";
//!     let force = false;
//!     let reason = InstallReason::Direct;
//!     let non_interactive = true;
//!
//!     zoi::install(source, force, reason, non_interactive)?;
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

pub mod cli;
pub mod cmd;
pub mod pkg;
pub mod project;
pub mod utils;

pub use pkg::install::InstallMode;
pub use pkg::pin::PinnedPackage;
pub use pkg::types::{
    self, Author, Config, Dependencies, InstallManifest, InstallReason, Maintainer, Package,
    PackageType, Scope,
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
/// * `force` - If true, forces re-installation even if the package is already installed.
/// * `reason` - The reason for installation (direct user action or as a dependency).
/// * `yes` - If true, automatically confirms any prompts (answers "yes").
///
/// # Returns
///
/// `Ok(())` on success, or an error if installation fails.
pub fn install(
    source: &str,
    force: bool,
    reason: InstallReason,
    yes: bool,
) -> Result<(), Box<dyn Error>> {
    // Internally, run_installation uses this to track dependencies and avoid cycles.
    // For a top-level install call, we start with an empty set.
    let mut processed_deps = HashSet::new();
    pkg::install::run_installation(
        source,
        InstallMode::PreferPrebuilt,
        force,
        reason,
        yes,
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
    let (pkg, _, _, _, _) = pkg::resolve::resolve_package_and_version(source)?;
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
pub fn pin(source: &str, version: &str) -> Result<(), Box<dyn Error>> {
    let mut pinned_packages = pkg::pin::get_pinned_packages()?;
    if pinned_packages.iter().any(|p| p.source == source) {
        return Err(format!("Package '{}' is already pinned.", source).into());
    }
    let new_pin = PinnedPackage {
        source: source.to_string(),
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
pub fn unpin(source: &str) -> Result<(), Box<dyn Error>> {
    let mut pinned_packages = pkg::pin::get_pinned_packages()?;
    let initial_len = pinned_packages.len();
    pinned_packages.retain(|p| p.source != source);
    if pinned_packages.len() == initial_len {
        return Err(format!("Package '{}' was not pinned.", source).into());
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
