use crate::pkg;
use anyhow::{Result, anyhow};
use clap::{ArgGroup, Parser, Subcommand};
use std::path::Path;

#[derive(Parser, Debug)]
#[command(long_about = "Manages PGP keys for package signature verification.")]
pub struct PgpCommand {
    #[command(subcommand)]
    pub command: PgpCommands,
}

#[derive(Subcommand, Debug)]
pub enum PgpCommands {
    /// Add a PGP key from a file, URL, or a keyserver
    Add(AddKey),
    /// Remove a PGP key
    #[command(alias = "rm")]
    Remove(RemoveKey),
    /// List all imported PGP keys
    #[command(alias = "ls")]
    List,
    /// Search for a PGP key by user ID or fingerprint
    Search(SearchKey),
    /// Show the public key of a stored PGP key
    Show(ShowKey),
    /// Verify a file's detached signature
    Verify(VerifySig),
}

#[derive(Parser, Debug)]
#[command(group(
    ArgGroup::new("source")
        .required(true)
        .args(["path", "fingerprint", "url"]),
))]
pub struct AddKey {
    /// Path to the PGP key file (.asc)
    #[arg(long)]
    pub path: Option<String>,

    /// Fingerprint of the PGP key to fetch from keys.openpgp.org
    #[arg(long)]
    pub fingerprint: Option<String>,

    /// URL of the PGP key to import
    #[arg(long)]
    pub url: Option<String>,

    /// Name to associate with the key (defaults to filename if adding from path/url)
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Parser, Debug)]
#[command(group(
    ArgGroup::new("key_id")
        .required(true)
        .args(["name", "fingerprint"]),
))]
pub struct RemoveKey {
    /// Name of the key to remove
    pub name: Option<String>,

    /// Fingerprint of the key to remove
    #[arg(long)]
    pub fingerprint: Option<String>,
}

#[derive(Parser, Debug)]
pub struct SearchKey {
    /// The user ID (name, email) or fingerprint to search for
    #[arg(required = true)]
    pub term: String,
}

#[derive(Parser, Debug)]
pub struct ShowKey {
    /// The name of the key to show
    #[arg(required = true)]
    pub name: String,
}

#[derive(Parser, Debug)]
pub struct VerifySig {
    /// Path to the file to verify
    #[arg(long)]
    pub file: String,

    /// Path to the detached signature file
    #[arg(long)]
    pub sig: String,

    /// Name of the key in the local store to use for verification
    #[arg(long)]
    pub key: String,
}

pub fn run(args: PgpCommand) -> Result<()> {
    match args.command {
        PgpCommands::Add(add_args) => {
            if let Some(path) = add_args.path {
                pkg::pgp::add_key_from_path(&path, add_args.name.as_deref())?;
            } else if let Some(fingerprint) = add_args.fingerprint {
                if let Some(name) = add_args.name {
                    pkg::pgp::add_key_from_fingerprint(&fingerprint, &name)?;
                } else {
                    return Err(anyhow!(
                        "A name must be provided when adding a key by fingerprint."
                    ));
                }
            } else if let Some(url) = add_args.url {
                let name = if let Some(n) = add_args.name {
                    n
                } else {
                    Path::new(&url)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or(anyhow!("Could not derive name from URL"))?
                        .to_string()
                };
                pkg::pgp::add_key_from_url(&url, &name)?;
            }
        }
        PgpCommands::Remove(remove_args) => {
            if let Some(name) = remove_args.name {
                pkg::pgp::remove_key_by_name(&name)?;
            } else if let Some(fingerprint) = remove_args.fingerprint {
                pkg::pgp::remove_key_by_fingerprint(&fingerprint)?;
            }
        }
        PgpCommands::List => {
            pkg::pgp::list_keys()?;
        }
        PgpCommands::Search(search_args) => {
            pkg::pgp::search_keys(&search_args.term)?;
        }
        PgpCommands::Show(show_args) => {
            pkg::pgp::show_key(&show_args.name)?;
        }
        PgpCommands::Verify(verify_args) => {
            pkg::pgp::cli_verify_signature(&verify_args.file, &verify_args.sig, &verify_args.key)?;
        }
    }
    Ok(())
}
