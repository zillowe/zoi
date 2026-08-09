//! Initialization of LSP support for package development.

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

/// Command to initialize LSP support in a workspace.
#[derive(Parser, Debug,)]
pub struct InitLspCommand {
    /// Path where the LSP configuration should be initialized
    #[arg(default_value = ".")]
    pub path: std::path::PathBuf,
}

/// Runs the LSP initialization command.
///
/// This setup creates necessary configuration files (like `.luarc.json`) and
/// downloads type definitions to enable better development experience in
/// LSP-supported editors.
///
/// # Errors
///
/// Returns an error if the LSP workspace setup fails.
pub fn run(args: &InitLspCommand,) -> Result<(),> {
    println!(
        "{} Initializing LSP support in {}...",
        "::".bold().blue(),
        args.path.display()
    );

    crate::pkg::package::init_lsp::setup_lsp_workspace(&args.path,)?;

    println!(
        "{} LSP support initialized. Created .luarc.json and type definitions.",
        "::".bold().green()
    );
    println!(
        "{} Use 'lua-language-server' for rich autocomplete and documentation.",
        "Note:".yellow()
    );

    Ok((),)
}
