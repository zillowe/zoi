use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct BuildCommand {
    /// Path to the package metadata file (e.g. path/to/name.meta.json)
    #[arg(required = true)]
    pub meta_file: PathBuf,

    /// The platform to build for (e.g. 'linux-amd64', 'windows-arm64', 'all', 'current').
    /// Can be specified multiple times.
    #[arg(long, short, num_args = 1.., default_values_t = vec!["current".to_string()])]
    pub platform: Vec<String>,
}

pub fn run(args: BuildCommand) {
    if let Err(e) = crate::pkg::package::build::run(&args.meta_file, &args.platform) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
