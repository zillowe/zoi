//! Command for managing Zoi extensions (plugins).

use anyhow::Result;

use crate::cli::{ExtensionCommand, ExtensionCommands};
use crate::pkg;

/// Runs the 'extension' command.
///
/// Dispatches to subcommands for adding or removing extensions.
///
/// # Errors
///
/// Returns an error if the extension addition or removal fails.
pub fn run(
    args: ExtensionCommand,
    yes: bool,
    plugin_manager: Option<&crate::pkg::plugin::PluginManager>
) -> Result<()> {
    match args.command {
        ExtensionCommands::Add { name } => {
            pkg::extension::add(&name, yes, plugin_manager)
        }
        ExtensionCommands::Remove { name } => {
            pkg::extension::remove(&name, yes, plugin_manager)
        }
    }
}
