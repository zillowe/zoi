//! # Zoi: The Advanced Package Manager & Environment Orchestrator
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
//! zoi-rs = "1"
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

pub use zoi_cli::cli;
pub use zoi_cli::cmd;
pub use zoi_cli::pkg;
pub use zoi_cli::project;
pub use zoi_core::utils;

use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};
pub use zoi_core::types::{self, Scope};

/// Options for building a package from a `.pkg.lua` definition.
#[derive(Debug, Clone)]
pub struct BuildOptions<'a> {
    /// Build type to use, such as `source` or `pre-compiled`.
    pub build_type: Option<&'a str>,
    /// Target platforms to build for. Use platform strings such as `linux-amd64`.
    pub platforms: Vec<String>,
    /// Optional PGP key name or fingerprint used to sign the output archive.
    pub sign_key: Option<String>,
    /// Whether to install build-time dependencies before building.
    pub install_deps: bool,
    /// Build backend to use. Supported values are `native` and `docker`.
    pub method: &'a str,
    /// Docker image to use when `method` is `docker`.
    pub image: Option<&'a str>,
    /// Optional package version override.
    pub version_override: Option<&'a str>,
    /// Whether to force root ownership (UID/GID 0) in the built archive.
    pub fakeroot: bool,
}

impl<'a> Default for BuildOptions<'a> {
    fn default() -> Self {
        Self {
            build_type: None,
            platforms: vec![
                zoi_core::utils::get_platform().unwrap_or_else(|_| "linux-amd64".to_string()),
            ],
            sign_key: None,
            install_deps: true,
            method: "native",
            image: None,
            version_override: None,
            fakeroot: false,
        }
    }
}

/// Options for installing a local `.pkg.tar.zst` archive.
#[derive(Debug, Clone)]
pub struct PackageInstallOptions {
    /// Optional installation scope override.
    pub scope_override: Option<Scope>,
    /// Registry handle to record for the installed package. Use `local` for local archives.
    pub registry_handle: String,
    /// Automatically answer yes to prompts.
    pub yes: bool,
    /// Optional split-package names to install from the archive.
    pub sub_packages: Option<Vec<String>>,
    /// Whether to create binary links for installed package binaries.
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

/// Options for installing one or more package source strings.
#[derive(Debug, Clone, Default)]
pub struct SourceInstallOptions {
    /// Optional git repository spec for `zoi install --repo`.
    pub repo: Option<String>,
    /// Force reinstalling packages that are already installed.
    pub force: bool,
    /// Accept all optional dependencies.
    pub all_optional: bool,
    /// Automatically answer yes to prompts.
    pub yes: bool,
    /// Optional installation scope override.
    pub scope_override: Option<Scope>,
    /// Save requested packages to the current project's `zoi.yaml`.
    pub save: bool,
    /// Build type to use when building from source.
    pub build_type: Option<String>,
    /// Print the install plan without performing the installation.
    pub dry_run: bool,
    /// Force building from source even when a prebuilt archive is available.
    pub build: bool,
    /// Enforce the current `zoi.lock` exactly for project installs.
    pub frozen_lockfile: bool,
}

/// Options for resolving a dependency graph without installing packages.
#[derive(Debug, Clone, Default)]
pub struct DependencyResolutionOptions {
    /// Optional scope to use when resolving dependencies.
    pub scope_override: Option<Scope>,
    /// Include packages even when they appear to be installed already.
    pub force: bool,
    /// Automatically answer yes to resolver prompts.
    pub yes: bool,
    /// Accept all optional dependencies.
    pub all_optional: bool,
    /// Build type used for selecting typed build dependencies.
    pub build_type: Option<String>,
    /// Suppress non-essential resolver output.
    pub quiet: bool,
}

/// Result of resolving a single package source.
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    /// Parsed package metadata.
    pub package: types::Package,
    /// Resolved package version.
    pub version: String,
    /// Portable manifest information suitable for lockfiles.
    pub sharable_manifest: Option<types::SharableInstallManifest>,
    /// Local path to the resolved package definition.
    pub source_path: PathBuf,
    /// Registry handle, when the source came from a registry.
    pub registry_handle: Option<String>,
    /// Registry repository type (official, community, etc.).
    pub repo_type: Option<String>,
    /// Git commit SHA, when the source came from a git repository.
    pub git_sha: Option<String>,
}

/// Dependency graph resolution result.
#[derive(Debug)]
pub struct DependencyResolution {
    /// Resolved Zoi package graph.
    pub graph: zoi_install::resolver::DependencyGraph,
    /// Dependencies handled by external package managers.
    pub non_zoi_dependencies: Vec<String>,
}

fn to_install_scope(scope: Scope) -> zoi_cli::cli::InstallScope {
    match scope {
        Scope::User => zoi_cli::cli::InstallScope::User,
        Scope::System => zoi_cli::cli::InstallScope::System,
        Scope::Project => zoi_cli::cli::InstallScope::Project,
    }
}

pub fn build_with_options(package_file: &Path, options: &BuildOptions<'_>) -> Result<()> {
    if options.install_deps {
        for platform in &options.platforms {
            let current_platform = if platform == "current" {
                zoi_core::utils::get_platform()?
            } else {
                platform.clone()
            };

            if let Some(dep_strings) = zoi_package::build::get_build_dependencies(
                package_file,
                options.build_type,
                &current_platform,
                options.version_override,
                false,
            )? && !dep_strings.is_empty()
            {
                println!("{} Installing build dependencies...", "::".bold().blue());
                let processed = std::sync::Mutex::new(std::collections::HashSet::new());
                let mut installed = Vec::new();
                for dep_str in dep_strings {
                    let dep = zoi_deps::parse_dependency_string(&dep_str)?;
                    zoi_install::dep_install::install_dependency(
                        &dep,
                        "build",
                        zoi_core::types::Scope::User,
                        true,
                        true,
                        &processed,
                        &mut installed,
                        None,
                    )?;
                }
            }
        }
    }

    zoi_package::build::run(
        package_file,
        options.build_type,
        &options.platforms,
        options.sign_key.clone(),
        None,
        options.version_override,
        None,
        false,
        options.method,
        options.image,
        options.fakeroot,
        options.install_deps,
    )
}

pub fn install_package_with_options(
    package_file: &Path,
    options: &PackageInstallOptions,
) -> Result<Vec<String>> {
    zoi_install::pkg_install::run(
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
    let plugin_manager = if zoi_core::utils::is_mini_mode() {
        None
    } else {
        let pm = zoi_plugins::PluginManager::new()?;
        let _ = pm.load_all(options.yes);
        Some(pm)
    };

    let pm_ptr = plugin_manager.as_ref();

    zoi_cli::cmd::install::run(
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
        pm_ptr,
        options.build,
        options.frozen_lockfile,
        false,
        false,
        3,
        false,
        false,
    )
}

pub fn resolve_package(source: &str, yes: bool) -> Result<ResolvedPackage> {
    let (package, version, sharable_manifest, source_path, registry_handle, repo_type, git_sha) =
        zoi_resolver::resolve::resolve_package_and_version(source, None, true, yes)?;
    Ok(ResolvedPackage {
        package,
        version,
        sharable_manifest,
        source_path,
        registry_handle,
        repo_type,
        git_sha,
    })
}

pub fn resolve_dependency_graph(
    sources: &[String],
    options: &DependencyResolutionOptions,
) -> Result<DependencyResolution> {
    let (graph, non_zoi_dependencies) = zoi_install::resolver::resolve_dependency_graph(
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
        fakeroot: false,
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
/// * *scope_override*: Optionally override the installation scope (`User`, `System`, `Project`).
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
    zoi_uninstall::run(package_name, scope_override, false, false, false).map(|_| ())
}
