//! # Zoi: The Universal Package Manager & Environment Setup Tool
//!
//! This crate provides the core functionality of Zoi as a library, allowing other
//! Rust applications to leverage its package management and environment setup
//! capabilities.
//!
//! For user documentation please visit [Zoi's Docs](https://zillowe.qzz.io/docs/zds/zoi), for the library documentation using this or
//! [Zoi's Lib Docs](https://zillowe.qzz.io/docs/zds/zoi/lib) is fine.
//!
//! ## Getting Started
//!
//! To use Zoi as a library, add it using `cargo` or as a dependency in your `Cargo.toml`:
//!
//! ```sh
//! cargo add zoi-rs
//! ```
//!
//! ```toml
//! [dependencies]
//! zoi = { version = "1" } // Replace with the latest version
//! ```
//!
//! ## Example: Install a package
//!
//! ```no_run
//! use zoi::{install_package_with_options, Scope};
//! use std::path::Path;
//! use anyhow::Result;
//!
//! fn main() -> Result<()> {
//!     let archive_path = Path::new("path/to/your/package-1.0.0-linux-amd64.pkg.tar.zst");
//!     let options = zoi::PackageInstallOptions {
//!         scope_override: Some(Scope::User),
//!         registry_handle: "local".to_string(),
//!         yes: true,
//!         ..Default::default()
//!     };
//!
//!     let installed_files = install_package_with_options(archive_path, &options)?;
//!
//!     println!("Package installed successfully. {} files were installed.", installed_files.len());
//!
//!     Ok(())
//! }
//! ```

pub mod cli;
pub mod cmd;
pub mod pkg;
pub mod project;
pub mod utils;

use anyhow::Result;
pub use pkg::types::{self, Scope};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BuildOptions<'a> {
    pub build_type: Option<&'a str>,
    pub platforms: Vec<String>,
    pub sign_key: Option<String>,
    pub install_deps: bool,
    pub method: &'a str,
    pub image: Option<&'a str>,
    pub version_override: Option<&'a str>,
}

impl<'a> Default for BuildOptions<'a> {
    fn default() -> Self {
        Self {
            build_type: None,
            platforms: vec![utils::get_platform().unwrap_or_else(|_| "linux-amd64".to_string())],
            sign_key: None,
            install_deps: true,
            method: "native",
            image: None,
            version_override: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PackageInstallOptions {
    pub scope_override: Option<Scope>,
    pub registry_handle: String,
    pub yes: bool,
    pub sub_packages: Option<Vec<String>>,
    pub link_bins: bool,
}

impl Default for PackageInstallOptions {
    fn default() -> Self {
        Self {
            scope_override: Some(Scope::User),
            registry_handle: "local".to_string(),
            yes: true,
            sub_packages: None,
            link_bins: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SourceInstallOptions {
    pub repo: Option<String>,
    pub force: bool,
    pub all_optional: bool,
    pub yes: bool,
    pub scope_override: Option<Scope>,
    pub save: bool,
    pub build_type: Option<String>,
    pub dry_run: bool,
    pub build: bool,
    pub frozen_lockfile: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DependencyResolutionOptions {
    pub scope_override: Option<Scope>,
    pub force: bool,
    pub yes: bool,
    pub all_optional: bool,
    pub build_type: Option<String>,
    pub quiet: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub package: types::Package,
    pub version: String,
    pub sharable_manifest: Option<types::SharableInstallManifest>,
    pub source_path: PathBuf,
    pub registry_handle: Option<String>,
    pub git_sha: Option<String>,
}

#[derive(Debug)]
pub struct DependencyResolution {
    pub graph: pkg::install::resolver::DependencyGraph,
    pub non_zoi_dependencies: Vec<String>,
}

fn to_install_scope(scope: Scope) -> cli::InstallScope {
    match scope {
        Scope::User => cli::InstallScope::User,
        Scope::System => cli::InstallScope::System,
        Scope::Project => cli::InstallScope::Project,
    }
}

pub fn build_with_options(package_file: &Path, options: &BuildOptions<'_>) -> Result<()> {
    pkg::package::build::run(
        package_file,
        options.build_type,
        &options.platforms,
        options.sign_key.clone(),
        None,
        options.version_override,
        None,
        false,
        options.install_deps,
        options.method,
        options.image,
    )
}

pub fn install_package_with_options(
    package_file: &Path,
    options: &PackageInstallOptions,
) -> Result<Vec<String>> {
    pkg::package::install::run(
        package_file,
        options.scope_override,
        &options.registry_handle,
        None,
        options.yes,
        options.sub_packages.clone(),
        options.link_bins,
        None,
    )
}

pub fn install_sources(sources: &[String], options: &SourceInstallOptions) -> Result<()> {
    let plugin_manager = pkg::plugin::PluginManager::new()?;
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
        options.build_type.clone(),
        options.dry_run,
        &plugin_manager,
        options.build,
        options.frozen_lockfile,
        false,
        false,
        3,
        false,
    )
}

pub fn resolve_package(source: &str, yes: bool) -> Result<ResolvedPackage> {
    let (package, version, sharable_manifest, source_path, registry_handle, git_sha) =
        pkg::resolve::resolve_package_and_version(source, true, yes)?;
    Ok(ResolvedPackage {
        package,
        version,
        sharable_manifest,
        source_path,
        registry_handle,
        git_sha,
    })
}

pub fn resolve_dependency_graph(
    sources: &[String],
    options: &DependencyResolutionOptions,
) -> Result<DependencyResolution> {
    let (graph, non_zoi_dependencies) = pkg::install::resolver::resolve_dependency_graph(
        sources,
        options.scope_override,
        options.force,
        options.yes,
        options.all_optional,
        options.build_type.as_deref(),
        options.quiet,
    )?;
    Ok(DependencyResolution {
        graph,
        non_zoi_dependencies,
    })
}

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
///     build(package_file, Some("source"), &platforms, None, true, "native", None, None)?;
///     println!("Package built successfully!");
///     Ok(())
/// }
/// ```
pub fn build(
    package_file: &Path,
    build_type: Option<&str>,
    platforms: &[String],
    sign_key: Option<String>,
    install_deps: bool,
    method: &str,
    image: Option<&str>,
    version_override: Option<&str>,
) -> Result<()> {
    let options = BuildOptions {
        build_type,
        platforms: platforms.to_vec(),
        sign_key,
        install_deps,
        method,
        image,
        version_override,
    };
    build_with_options(package_file, &options)
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
/// * `yes`: Automatically answer "yes" to any confirmation prompts (e.g. file conflicts).
/// * `sub_packages`: For split packages, optionally specify which sub-packages to install.
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
///     install_package(archive_path, Some(Scope::User), "local", true, None)?;
///     println!("Package installed!");
///     Ok(())
/// }
/// ```
pub fn install_package(
    package_file: &Path,
    scope_override: Option<Scope>,
    registry_handle: &str,
    yes: bool,
    sub_packages: Option<Vec<String>>,
) -> Result<Vec<String>> {
    let options = PackageInstallOptions {
        scope_override,
        registry_handle: registry_handle.to_string(),
        yes,
        sub_packages,
        link_bins: true,
    };
    install_package_with_options(package_file, &options)
}

/// Uninstalls a Zoi package.
///
/// This function removes a package's files from the Zoi store and unlinks its binaries.
///
/// # Arguments
///
/// * `package_name`: The package identifier to uninstall. Use an explicit source
///   like `#handle@repo/name[:sub]@version` when multiple installed packages
///   share the same name.
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
    pkg::uninstall::run(package_name, scope_override, false).map(|_| ())
}
