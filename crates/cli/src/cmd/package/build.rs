//! Logic for the `package build` command.
//!
//! This module provides the functionality to build Zoi packages from their
//! definition files, handling dependencies, platforms, and isolation.

use anyhow::Result;
use clap::Parser;
use colored::*;
use std::path::PathBuf;

/// Arguments for the `package build` command.
#[derive(Parser, Debug)]
pub struct BuildCommand {
    /// Path to the package file (e.g. path/to/name.pkg.lua)
    #[arg(required = true)]
    pub package_file: PathBuf,

    /// The type of package to build (e.g. 'source', 'pre-compiled').
    #[arg(long)]
    pub r#type: Option<String>,

    /// The platform to build for (e.g. 'linux-amd64', 'windows-arm64', 'all', 'current').
    /// Can be specified multiple times.
    #[arg(long, short, num_args = 1.., default_values_t = vec!["current".to_string()])]
    pub platform: Vec<String>,

    /// The sub-packages to build.
    #[arg(long, num_args = 1..)]
    pub sub: Option<Vec<String>>,

    /// Sign the package with the given PGP key (name or fingerprint)
    #[arg(long)]
    pub sign: Option<String>,

    /// Run tests before building
    #[arg(long)]
    pub test: bool,

    /// Directory to output the built package to
    #[arg(long, short = 'o')]
    pub output_dir: Option<PathBuf>,

    /// Automatically install build-time dependencies
    #[arg(long)]
    pub install_deps: bool,

    /// Override the package version
    #[arg(long)]
    pub version_override: Option<String>,

    /// Method to use for building ('native', 'bwrap', or 'docker')
    #[arg(long, default_value = "native")]
    pub method: String,

    /// Docker image to use when method is 'docker'
    #[arg(long)]
    pub image: Option<String>,

    /// Force root ownership (UID/GID 0) in the built archive
    #[arg(long)]
    pub fakeroot: bool,

    /// Build in a clean, isolated sysroot.
    /// Requires a base package (specified by --root-package) to be installed into the sysroot.
    #[arg(long)]
    pub pure: bool,

    /// The base package to install when using --pure (default: @core/base:dev).
    #[arg(long, default_value = "@core/base:dev")]
    pub root_package: String,
}

/// Run the package build command.
pub fn run(mut args: BuildCommand) -> Result<()> {
    let mut _temp_root = None;

    if args.pure {
        if args.method == "docker" {
            return Err(anyhow::anyhow!(
                "--pure is not compatible with --method docker"
            ));
        }
        args.method = "bwrap".to_string();

        let temp = tempfile::Builder::new().prefix("zoi-pure-").tempdir()?;
        println!(
            "{} Initializing pure build environment in {}...",
            "::".bold().blue(),
            temp.path().display()
        );

        // Create ZoiOS marker so Zoi uses /usr/bin instead of /usr/local/bin
        // This ensures the root package and build deps land where bwrap expects them.
        let etc_dir = temp.path().join("etc");
        std::fs::create_dir_all(&etc_dir)?;
        std::fs::write(etc_dir.join("os-release"), "ID=zoios\nID_LIKE=zoios\n")?;

        crate::pkg::sysroot::set_sysroot(temp.path().to_path_buf());
        _temp_root = Some(temp);

        // Install the root base environment package
        let root_dep = crate::pkg::dependencies::parse_dependency_string(&args.root_package)?;
        crate::pkg::install::dep_install::install_dependency(
            &root_dep,
            "pure-root",
            crate::pkg::types::Scope::System,
            true,
            true,
            &std::sync::Mutex::new(std::collections::HashSet::new()),
            &mut Vec::new(),
            None,
        )?;
    }

    if args.install_deps {
        install_dependencies_for_build(&args, args.test)?;
    }

    if args.test {
        println!("Running tests before building...");
        crate::pkg::package::test::run(&args)?;
        println!("Tests passed, proceeding with build...");
    }

    crate::pkg::package::build::run(
        &args.package_file,
        args.r#type.as_deref(),
        &args.platform,
        args.sign,
        args.output_dir.as_deref(),
        args.version_override.as_deref(),
        args.sub,
        false,
        &args.method,
        args.image.as_deref(),
        args.fakeroot,
        args.install_deps,
        args.test,
    )
}

/// Install dependencies required for building the package.
pub fn install_dependencies_for_build(args: &BuildCommand, include_test: bool) -> Result<()> {
    for platform in &args.platform {
        let current_platform = if platform == "current" {
            crate::pkg::utils::get_platform()?
        } else {
            platform.clone()
        };

        if let Some(mut dep_strings) = crate::pkg::package::build::get_build_dependencies(
            &args.package_file,
            args.r#type.as_deref(),
            &current_platform,
            args.version_override.as_deref(),
            false,
        )? {
            if include_test
                && let Some(test_deps) = crate::pkg::package::build::get_test_dependencies(
                    &args.package_file,
                    &current_platform,
                    args.version_override.as_deref(),
                    false,
                )?
            {
                dep_strings.extend(test_deps);
            }

            if !dep_strings.is_empty() {
                println!(
                    "{} Installing build/test dependencies...",
                    "::".bold().blue()
                );
                let processed = std::sync::Mutex::new(std::collections::HashSet::new());
                let mut installed = Vec::new();
                for dep_str in dep_strings {
                    let dep = crate::pkg::dependencies::parse_dependency_string(&dep_str)?;
                    crate::pkg::install::dep_install::install_dependency(
                        &dep,
                        "build",
                        if crate::pkg::sysroot::get_sysroot().is_some() {
                            crate::pkg::types::Scope::System
                        } else {
                            crate::pkg::types::Scope::User
                        },
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
    Ok(())
}
