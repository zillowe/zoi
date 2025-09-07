use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct InstallCommand {
    /// Path to the package archive file (e.g. path/to/name-os-arch.pkg.tar.zst)
    #[arg(required = true)]
    pub package_file: PathBuf,
}

pub fn run(args: InstallCommand) {
    if let Err(e) = crate::pkg::package::install::run(&args.package_file) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
