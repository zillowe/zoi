use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct BuildCommand {
    /// Path to the package metadata file (e.g. path/to/name.meta.json)
    #[arg(required = true)]
    pub meta_file: PathBuf,
}

pub fn run(args: BuildCommand) {
    if let Err(e) = crate::pkg::package::build::run(&args.meta_file) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
