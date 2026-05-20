use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct HelperCommand {
    #[command(subcommand)]
    pub command: HelperCommands,
}

#[derive(Subcommand, Debug)]
pub enum HelperCommands {
    /// Get a hash of a local file or a file from a URL
    GetHash(GetHashCommand),

    /// Validate a Zoi specification file (e.g. registries.json, repo.yaml, advisories.json)
    #[command(alias = "val")]
    Validate(ValidateCommand),
}

#[derive(Parser, Debug)]
pub struct GetHashCommand {
    /// The local file path or URL to hash
    #[arg(required = true)]
    pub source: String,

    /// The hash algorithm to use
    #[arg(long, value_enum, default_value = "sha512")]
    pub hash: HashAlgorithm,
}

#[derive(Parser, Debug)]
pub struct ValidateCommand {
    /// The local file path to validate
    #[arg(required = true)]
    pub file: std::path::PathBuf,
}

#[derive(clap::ValueEnum, Clone, Debug, Copy)]
pub enum HashAlgorithm {
    Sha512,
    Sha256,
}

pub fn run(args: HelperCommand) -> Result<()> {
    match args.command {
        HelperCommands::GetHash(cmd) => {
            let hash_type = match cmd.hash {
                HashAlgorithm::Sha512 => crate::pkg::helper::HashType::Sha512,
                HashAlgorithm::Sha256 => crate::pkg::helper::HashType::Sha256,
            };
            let hash = crate::pkg::helper::get_hash(&cmd.source, hash_type)?;
            println!("{}", hash);
            Ok(())
        }
        HelperCommands::Validate(cmd) => crate::pkg::helper::validate::run(&cmd.file),
    }
}
