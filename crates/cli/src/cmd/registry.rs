use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct RegistryCommand {
    #[command(subcommand)]
    pub command: RegistryCommands,
}

#[derive(Subcommand, Debug)]
pub enum RegistryCommands {
    /// Initialize a new Zoi registry
    Init {
        /// Path where the registry should be initialized
        #[arg(default_value = ".")]
        path: std::path::PathBuf,
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
        repo: Option<String>,
    },
    /// Add a new security advisory for a package
    #[command(alias = "sec")]
    AddAdvisory {
        /// Package name to add an advisory for
        package: String,
    },
}

pub fn run(args: RegistryCommand) -> Result<()> {
    let registry_root = std::path::Path::new(".");
    match args.command {
        RegistryCommands::Init { path } => crate::pkg::registry::init(&path),
        RegistryCommands::GenerateMetadata => {
            crate::pkg::registry::generate_metadata(registry_root)
        }
        RegistryCommands::Check => crate::pkg::registry::check(registry_root),
        RegistryCommands::AddPackage { name, repo } => {
            crate::pkg::registry::add_package(registry_root, name.as_deref(), repo.as_deref())
        }
        RegistryCommands::AddAdvisory { package } => {
            crate::pkg::registry::add_advisory(registry_root, &package)
        }
    }
}
