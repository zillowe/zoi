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

    /// The sub-packages to build.
    #[arg(long, num_args = 1..)]
    pub sub: Option<Vec<String>>,

    /// Sign the package with the given PGP key (name or fingerprint)
    #[arg(long)]
    pub sign: Option<String>,

    /// Run tests before building
    #[arg(long)]
    pub test: bool,

    /// Directory to output the built package to
    #[arg(long, short = 'o')]
    pub output_dir: Option<PathBuf>,
}

pub fn run(args: BuildCommand) {
    if args.test {
        println!("Running tests before building...");
        if let Err(e) = crate::pkg::package::test::run(&args) {
            eprintln!("Tests failed: {}", e);
            std::process::exit(1);
        }
        println!("Tests passed, proceeding with build...");
    }

    if let Err(e) = crate::pkg::package::build::run(
        &args.package_file,
        &args.r#type,
        &args.platform,
        args.sign,
        args.output_dir.as_deref(),
        None,
        args.sub,
    ) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
