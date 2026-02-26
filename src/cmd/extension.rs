use crate::cli::{ExtensionCommand, ExtensionCommands};
use crate::pkg;
use anyhow::Result;

pub fn run(
    args: ExtensionCommand,
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    match args.command {
        ExtensionCommands::Add { name } => pkg::extension::add(&name, yes, plugin_manager),
        ExtensionCommands::Remove { name } => pkg::extension::remove(&name, yes, plugin_manager),
    }
}
