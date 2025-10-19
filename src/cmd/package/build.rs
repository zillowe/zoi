use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct BuildCommand {
    /// Path to the package file (e.g. path/to/name.pkg.lua)
    #[arg(required = true)]
    pub package_file: PathBuf,

    /// The type of package to build (e.g. 'source', 'pre-compiled').
    #[arg(long, required = true)]
    pub r#type: String,

    /// The platform to build for (e.g. 'linux-amd64', 'windows-arm64', 'all', 'current').
    /// Can be specified multiple times.
    #[arg(long, short, num_args = 1.., default_values_t = vec!["current".to_string()])]
    pub platform: Vec<String>,

    /// Sign the package with the given PGP key (name or fingerprint)
    #[arg(long)]
    pub sign: Option<String>,
}

pub fn run(args: BuildCommand) {
    if let Err(e) = crate::pkg::package::build::run(
        &args.package_file,
        &args.r#type,
        &args.platform,
        args.sign,
        None,
        None,
    ) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
