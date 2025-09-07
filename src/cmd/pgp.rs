use crate::pkg;
use clap::{ArgGroup, Parser, Subcommand};
use std::error::Error;

#[derive(Parser, Debug)]
#[command(long_about = "Manages PGP keys for package signature verification.")]
pub struct PgpCommand {
    #[command(subcommand)]
    pub command: PgpCommands,
}

#[derive(Subcommand, Debug)]
pub enum PgpCommands {
    /// Add a PGP key from a file or a keyserver
    Add(AddKey),
    /// Remove a PGP key
    Remove(RemoveKey),
}

#[derive(Parser, Debug)]
#[command(group(
    ArgGroup::new("source")
        .required(true)
        .args(["path", "fingerprint"]),
))]
pub struct AddKey {
    /// Path to the PGP key file (.asc)
    pub path: Option<String>,

    /// Fingerprint of the PGP key to fetch from keys.openpgp.org
    #[arg(long)]
    pub fingerprint: Option<String>,

    /// Name to associate with the key (defaults to filename if adding from path)
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

pub fn run(args: PgpCommand) -> Result<(), Box<dyn Error>> {
    match args.command {
        PgpCommands::Add(add_args) => {
            if let Some(path) = add_args.path {
                pkg::pgp::add_key_from_path(&path, add_args.name.as_deref())?;
            } else if let Some(fingerprint) = add_args.fingerprint {
                if let Some(name) = add_args.name {
                    pkg::pgp::add_key_from_fingerprint(&fingerprint, &name)?;
                } else {
                    return Err("A name must be provided when adding a key by fingerprint.".into());
                }
            }
        }
        PgpCommands::Remove(remove_args) => {
            if let Some(name) = remove_args.name {
                pkg::pgp::remove_key_by_name(&name)?;
            } else if let Some(fingerprint) = remove_args.fingerprint {
                pkg::pgp::remove_key_by_fingerprint(&fingerprint)?;
            }
        }
    }
    Ok(())
}
