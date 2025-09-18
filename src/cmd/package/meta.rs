use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct MetaCommand {
    /// Path to the package file (e.g. path/to/name.pkg.lua)
    #[arg(required = true)]
    pub package_file: PathBuf,
    /// Generate metadata from a specific installation type
    #[arg(long, value_parser = ["binary", "com_binary", "source"])]
    pub r#type: Option<String>,
    /// Set a version for the package metadata
    #[arg(long)]
    pub version: Option<String>,
}

pub fn run(args: MetaCommand) {
    if let Err(e) =
        crate::pkg::package::meta::run(&args.package_file, args.r#type, args.version.as_deref())
    {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
