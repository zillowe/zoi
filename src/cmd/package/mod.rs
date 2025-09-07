use clap::{Parser, Subcommand};

pub mod build;
pub mod install;
pub mod meta;

#[derive(Parser, Debug)]
pub struct PackageCommand {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate package metadata from a package file
    Meta(meta::MetaCommand),
    /// Build a package from a metadata file
    Build(build::BuildCommand),
    /// Install a package from a local archive
    Install(install::InstallCommand),
}

pub fn run(args: PackageCommand) {
    match args.command {
        Commands::Meta(cmd) => meta::run(cmd),
        Commands::Build(cmd) => build::run(cmd),
        Commands::Install(cmd) => install::run(cmd),
    }
}
