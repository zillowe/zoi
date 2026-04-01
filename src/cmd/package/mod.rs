use anyhow::Result;
use clap::{Parser, Subcommand};

pub mod build;
pub mod doctor;
pub mod install;
pub mod test;

#[derive(Parser, Debug)]
pub struct PackageCommand {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build a package from a pkg.lua file
    Build(build::BuildCommand),
    /// Test a package from a pkg.lua file
    Test(build::BuildCommand),
    /// Install a package from a local archive
    Install(install::InstallCommand),
    /// Lint and validate a package definition for maintainers
    Doctor(doctor::DoctorCommand),
}

pub fn run(args: PackageCommand) -> Result<()> {
    match args.command {
        Commands::Build(cmd) => build::run(cmd),
        Commands::Test(cmd) => test::run(cmd),
        Commands::Install(cmd) => install::run(cmd),
        Commands::Doctor(cmd) => doctor::run(cmd),
    }
}
