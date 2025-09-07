use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct MetaCommand {
    /// Path to the package file (e.g. path/to/name.pkg.lua)
    #[arg(required = true)]
    pub package_file: PathBuf,
}

pub fn run(args: MetaCommand) {
    if let Err(e) = crate::pkg::package::meta::run(&args.package_file) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
