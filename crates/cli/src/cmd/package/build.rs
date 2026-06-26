use anyhow::Result;
use clap::Parser;
use colored::*;
use std::path::PathBuf;

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

    /// Method to use for building ('native' or 'docker')
    #[arg(long, default_value = "native")]
    pub method: String,

    /// Docker image to use when method is 'docker'
    #[arg(long)]
    pub image: Option<String>,

    /// Force root ownership (UID/GID 0) in the built archive
    #[arg(long)]
    pub fakeroot: bool,
}

pub fn run(args: BuildCommand) -> Result<()> {
    if args.test {
        println!("Running tests before building...");
        crate::pkg::package::test::run(&args)?;
        println!("Tests passed, proceeding with build...");
    }

    if args.install_deps {
        for platform in &args.platform {
            let current_platform = if platform == "current" {
                crate::pkg::utils::get_platform()?
            } else {
                platform.clone()
            };

            if let Some(dep_strings) = crate::pkg::package::build::get_build_dependencies(
                &args.package_file,
                args.r#type.as_deref(),
                &current_platform,
                args.version_override.as_deref(),
                false,
            )? && !dep_strings.is_empty()
            {
                println!("{} Installing build dependencies...", "::".bold().blue());
                let processed = std::sync::Mutex::new(std::collections::HashSet::new());
                let mut installed = Vec::new();
                for dep_str in dep_strings {
                    let dep = crate::pkg::dependencies::parse_dependency_string(&dep_str)?;
                    crate::pkg::install::dep_install::install_dependency(
                        &dep,
                        "build",
                        crate::pkg::types::Scope::User,
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
    )
}
