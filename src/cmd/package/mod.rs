use clap::{Parser, Subcommand};

pub mod build;
pub mod install;

#[derive(Parser, Debug)]
pub struct PackageCommand {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build a package from a pkg.lua file
    Build(build::BuildCommand),
    /// Install a package from a local archive
    Install(install::InstallCommand),
}

pub fn run(args: PackageCommand) {
    match args.command {
        Commands::Build(cmd) => build::run(cmd),
        Commands::Install(cmd) => install::run(cmd),
    }
}
