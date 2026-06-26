use anyhow::Result;
use clap::Parser;
use colored::Colorize;

#[derive(Parser, Debug)]
pub struct InitLspCommand {
    /// Path where the LSP configuration should be initialized
    #[arg(default_value = ".")]
    pub path: std::path::PathBuf,
}

pub fn run(args: InitLspCommand) -> Result<()> {
    println!(
        "{} Initializing LSP support in {}...",
        "::".bold().blue(),
        args.path.display()
    );

    crate::pkg::package::init_lsp::setup_lsp_workspace(&args.path)?;

    println!(
        "{} LSP support initialized. Created .luarc.json and type definitions.",
        "::".bold().green()
    );
    println!(
        "{} Use 'lua-language-server' for rich autocomplete and documentation.",
        "Note:".yellow()
    );

    Ok(())
}
