//! Implementation of the `create` command for scaffolding new applications from
//! templates.

use anyhow::Result;
use clap::Parser;

use crate::pkg;

/// Arguments for the `create` command.
#[derive(Parser)]
pub struct CreateCommand {
    /// The source of the package (name, @repo/name, path to .pkg.lua, or URL)
    pub source: String,
    /// The application name and directory to create (defaults to package name)
    pub app_name: Option<String>
}

/// Runs the `create` command.
/// # Errors
///
/// Returns an error if the application cannot be created or dependencies cannot
/// be resolved.
pub fn run(
    args: CreateCommand,
    yes: bool,
    plugin_manager: Option<&crate::pkg::plugin::PluginManager>
) -> Result<()> {
    pkg::create::run(&args.source, args.app_name, yes, plugin_manager)
}
