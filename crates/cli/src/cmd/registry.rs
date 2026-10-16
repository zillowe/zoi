//! Logic for the `registry` command.
//!
//! This module provides commands for managing Zoi registries, including
//! initialization, metadata generation, and package/advisory management.

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

/// The root registry management command.
#[derive(Parser, Debug)]
pub struct RegistryCommand {
    /// The specific registry subcommand to execute.
    #[command(subcommand)]
    pub command: RegistryCommands
}

/// Available registry subcommands.
#[derive(Subcommand, Debug)]
pub enum RegistryCommands {
    /// Initialize a new Zoi registry
    Init {
        /// Path where the registry should be initialized
        #[arg(default_value = ".")]
        path: std::path::PathBuf
    },
    /// Generate metadata files (packages.json and advisories.json)
    #[command(alias = "gen-meta")]
    GenerateMetadata,
    /// Check registry integrity and validate packages
    #[command(aliases = ["lint", "audit"])]
    Check,
    /// Add a new package to the registry
    #[command(alias = "add-pkg")]
    AddPackage {
        /// Name of the package to add
        name: Option<String>,
        /// Repository tier (e.g. community, main)
        #[arg(long, short)]
        repo: Option<String>
    },
    /// Add a new security advisory for a package
    #[command(alias = "sec")]
    AddAdvisory {
        /// Package name to add an advisory for
        package: Option<String>,
        /// Repository tier (e.g. community, main)
        #[arg(long, short)]
        repo: Option<String>
    },
    /// Build a .zrepo snapshot of this registry, plus its zsync control file
    #[command(alias = "snapshot")]
    Zrepo {
        /// Registry directory to snapshot. Defaults to the current directory
        #[arg(default_value = ".")]
        path: std::path::PathBuf,
        /// Where to write the .zrepo. Defaults to <handle>.zrepo
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,
        /// Sign the snapshot with a PGP key from the Zoi keyring
        #[arg(long)]
        sign: Option<String>
    }
}

/// Run the registry management command.
///
/// # Errors
///
/// This function returns an error if any of the underlying registry operations
/// (initialization, metadata generation, checking, adding packages/advisories,
/// or building a snapshot and its update patch) fail.
pub fn run(args: RegistryCommand) -> Result<()> {
    let registry_root = std::path::Path::new(".");
    match args.command {
        RegistryCommands::Init { path } => crate::pkg::registry::init(&path),
        RegistryCommands::GenerateMetadata => {
            crate::pkg::registry::generate_metadata(registry_root)
        }
        RegistryCommands::Check => crate::pkg::registry::check(registry_root),
        RegistryCommands::AddPackage { name, repo } => {
            crate::pkg::registry::add_package(
                registry_root,
                name.as_deref(),
                repo.as_deref()
            )
        }
        RegistryCommands::AddAdvisory { package, repo } => {
            crate::pkg::registry::add_advisory(
                registry_root,
                package.as_deref(),
                repo.as_deref()
            )
        }
        RegistryCommands::Zrepo { path, output, sign } => {
            build_zrepo(&path, output.as_deref(), sign.as_deref())
        }
    }
}

/// Builds a `.zrepo` snapshot of a registry directory.
///
/// # Errors
///
/// Returns an error if the directory is not a registry or the snapshot cannot
/// be written.
fn build_zrepo(
    path: &std::path::Path,
    output: Option<&std::path::Path>,
    sign: Option<&str>
) -> Result<()> {
    let output = match output {
        Some(explicit) => explicit.to_path_buf(),
        None => default_snapshot_output(path)?
    };

    let stats = crate::pkg::sync::zrepo::create_zrepo(path, &output, sign)?;

    println!(
        "{} Created snapshot: {}",
        "::".bold().green(),
        output.display()
    );
    println!("  Files:       {}", stats.file_count);
    println!(
        "  Size:        {} (uncompressed {})",
        zoi_core::utils::format_bytes(stats.compressed_size),
        zoi_core::utils::format_bytes(stats.tar_size)
    );
    println!("  SHA-256:     {}", stats.sha256);
    println!(
        "  Sidecars:    .hash, .size{}",
        if sign.is_some() { ", .sig" } else { "" }
    );
    println!(
        "{} Publish the snapshot and its .zsync control file together at the \
         'url' in the registry's 'zrepo' section, and point 'hash' at the \
         .hash sidecar. Clients use the control file to transfer only the \
         frames that changed.",
        "::".bold().blue()
    );
    Ok(())
}

/// Derives a default snapshot filename from the registry's declared handle.
fn default_snapshot_output(
    path: &std::path::Path
) -> Result<std::path::PathBuf> {
    let handle = zoi_core::config::read_repo_config(path)?.name;
    let handle = if handle.is_empty() {
        "registry".to_string()
    } else {
        handle
    };
    Ok(path.join(format!("{handle}.zrepo")))
}
