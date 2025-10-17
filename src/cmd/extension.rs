use crate::cli::{ExtensionCommand, ExtensionCommands};
use crate::pkg;
use anyhow::Result;

pub fn run(args: ExtensionCommand, yes: bool) -> Result<()> {
    match args.command {
        ExtensionCommands::Add { name } => pkg::extension::add(&name, yes),
        ExtensionCommands::Remove { name } => pkg::extension::remove(&name, yes),
    }
}
