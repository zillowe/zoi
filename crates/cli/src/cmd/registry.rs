//! Logic for the `registry` command.
//!
//! This module provides commands for managing Zoi registries, including
//! initialization, metadata generation, and package/advisory management.

use anyhow::Result;
use clap::{Parser, Subcommand};

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
    }
}

/// Run the registry management command.
///
/// # Errors
///
/// This function returns an error if any of the underlying registry operations
/// (initialization, metadata generation, checking, or adding
/// packages/advisories) fail. # Errors
///
/// Returns an error if the registry operation fails.
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
    }
}
