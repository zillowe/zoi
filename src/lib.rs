//! # Zoi: The Universal Package Manager & Environment Setup Tool
//!
//! This crate provides the core functionality of Zoi as a library, allowing other
//! Rust applications to leverage its package management and environment setup
//! capabilities.
//!
//! ## Getting Started
//!
//! To use Zoi as a library, add it as a dependency in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! zoi = { version = "1.0.0" } // Replace with the desired version
//! ```
//!
//! ## Example: Install a package
//!
//! ```no_run
//! use zoi::{install_package, Scope};
//! use std::path::Path;
//! use anyhow::Result;
//!
//! fn main() -> Result<()> {
//!     let archive_path = Path::new("path/to/your/package-1.0.0-linux-amd64.pkg.tar.zst");
//!     let scope = Some(Scope::User);
//!     let registry_handle = "local";
//!
//!     let installed_files = install_package(archive_path, scope, registry_handle)?;
//!
//!     println!("Package installed successfully. {} files were installed.", installed_files.len());
//!
//!     Ok(())
//! }
//! ```
//!
//! For more detailed documentation, see the [Zoi Documentation Hub](https://zillowe.qzz.io/docs/zds/zoi).
//! For more library examples, see the [Library Examples page](https://zillowe.qzz.io/docs/zds/zoi/lib/examples).

pub mod cli;
pub mod cmd;
pub mod pkg;
pub mod project;
pub mod utils;

use anyhow::Result;
pub use pkg::types::{self, Scope};
use std::path::Path;

/// Builds a Zoi package from a local `.pkg.lua` file.
///
/// This function reads a package definition, runs the build process, and creates
/// a distributable `.pkg.tar.zst` archive.
///
/// # Arguments
///
/// * `package_file`: Path to the `.pkg.lua` file.
/// * `build_type`: The type of package to build (e.g. "source", "pre-compiled").
/// * `platforms`: A slice of platform strings to build for (e.g. `["linux-amd64"]`).
/// * `sign_key`: An optional PGP key name or fingerprint to sign the package.
///
/// # Errors
///
/// Returns an error if the build process fails, if the package file cannot be read,
/// or if the specified build type is not supported by the package.
///
/// # Examples
///
/// ```no_run
/// use zoi::build;
/// use std::path::Path;
/// use anyhow::Result;
///
/// fn main() -> Result<()> {
///     let package_file = Path::new("my-package.pkg.lua");
///     let platforms = vec!["linux-amd64".to_string()];
///     build(package_file, "source", &platforms, None)?;
///     println!("Package built successfully!");
///     Ok(())
/// }
/// ```
pub fn build(
    package_file: &Path,
    build_type: &str,
    platforms: &[String],
    sign_key: Option<String>,
) -> Result<()> {
    pkg::package::build::run(package_file, build_type, platforms, sign_key, None, None)
}

/// Installs a Zoi package from a local package archive.
///
/// This function unpacks a `.pkg.tar.zst` archive and installs its contents
/// into the appropriate Zoi store, linking any binaries.
///
/// # Arguments
///
/// * `package_file`: Path to the local package archive.
/// * `scope_override`: Optionally override the installation scope (`User`, `System`, `Project`).
/// * `registry_handle`: The handle of the registry this package belongs to (e.g. "zoidberg", or "local").
///
/// # Returns
///
/// A `Result` containing a `Vec<String>` of all the file paths that were installed.
///
/// # Errors
///
/// Returns an error if the installation fails, such as if the archive is invalid
/// or if there are file system permission issues.
///
/// # Examples
///
/// ```no_run
/// use zoi::{install_package, Scope};
/// use std::path::Path;
/// use anyhow::Result;
///
/// fn main() -> Result<()> {
///     let archive_path = Path::new("my-package-1.0.0-linux-amd64.pkg.tar.zst");
///     install_package(archive_path, Some(Scope::User), "local")?;
///     println!("Package installed!");
///     Ok(())
/// }
/// ```
pub fn install_package(
    package_file: &Path,
    scope_override: Option<Scope>,
    registry_handle: &str,
) -> Result<Vec<String>> {
    pkg::package::install::run(package_file, scope_override, registry_handle, None)
}

/// Uninstalls a Zoi package.
///
/// This function removes a package's files from the Zoi store and unlinks its binaries.
///
/// # Arguments
///
/// * `package_name`: The name of the package to uninstall.
/// * `scope_override`: Optionally specify the scope to uninstall from. If `None`, Zoi
///   will search for the package across all scopes.
///
/// # Errors
///
/// Returns an error if the package is not found or if the uninstallation process fails.
///
/// # Examples
///
/// ```no_run
/// use zoi::{uninstall_package, Scope};
/// use anyhow::Result;
///
/// fn main() -> Result<()> {
///     uninstall_package("my-package", Some(Scope::User))?;
///     println!("Package uninstalled!");
///     Ok(())
/// }
/// ```
pub fn uninstall_package(package_name: &str, scope_override: Option<Scope>) -> Result<()> {
    pkg::uninstall::run(package_name, scope_override).map(|_| ())
}
