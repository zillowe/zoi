use crate::cli::{ExtensionCommand, ExtensionCommands};
use crate::pkg;
use std::error::Error;

pub fn run(args: ExtensionCommand, yes: bool) -> Result<(), Box<dyn Error>> {
    match args.command {
        ExtensionCommands::Add { name } => pkg::extension::add(&name, yes),
        ExtensionCommands::Remove { name } => pkg::extension::remove(&name, yes),
    }
}
