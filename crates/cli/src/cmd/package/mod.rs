//! Package development and maintenance commands.
//!
//! This module provides tools for package maintainers to build, test, bundle,
//! and validate Zoi packages.

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Build command module.
pub mod build;
/// Bundle command module.
pub mod bundle;
/// Doctor command module.
pub mod doctor;
/// Init-LSP command module.
pub mod init_lsp;
/// Inspect command module.
pub mod inspect;
/// Install command module.
pub mod install;
/// Test command module.
pub mod test;

/// Arguments for the `package` command.
#[derive(Parser, Debug)]
pub struct PackageCommand {
    /// The package sub-command to run.
    #[command(subcommand)]
    command: Commands,
}

/// Available package sub-commands.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Build a package from a pkg.lua file
    Build(build::BuildCommand),
    /// Bundle a package and its local assets into a .zsa archive
    Bundle(bundle::BundleCommand),
    /// Test a package from a pkg.lua file
    Test(build::BuildCommand),
    /// Install a package from a local archive
    Install(install::InstallCommand),
    /// Lint and validate a package definition for maintainers
    Doctor(doctor::DoctorCommand),
    /// Initialize LSP support for .pkg.lua files
    InitLsp(init_lsp::InitLspCommand),
    /// Inspect a package definition and output metadata
    Inspect(inspect::InspectCommand),
}

/// Runs the `package` command.
pub fn run(args: PackageCommand) -> Result<()> {
    match args.command {
        Commands::Build(cmd) => build::run(cmd),
        Commands::Bundle(cmd) => bundle::run(cmd),
        Commands::Test(cmd) => test::run(cmd),
        Commands::Install(cmd) => install::run(cmd),
        Commands::Doctor(cmd) => doctor::run(cmd),
        Commands::InitLsp(cmd) => init_lsp::run(cmd),
        Commands::Inspect(cmd) => inspect::run(cmd),
    }
}
