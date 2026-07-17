use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct BundleCommand {
    /// Path to the package file (e.g. path/to/name.pkg.lua)
    #[arg(required = true)]
    pub package_file: PathBuf,

    /// Directory to output the bundled package to
    #[arg(long, short = 'o')]
    pub output_dir: Option<PathBuf>,

    /// Sign the bundle with a PGP key
    #[arg(long)]
    pub sign: Option<String>,

    /// Override the package version
    #[arg(long)]
    pub version_override: Option<String>,
}

pub fn run(args: BundleCommand) -> Result<()> {
    crate::pkg::package::bundle::run(
        &args.package_file,
        args.output_dir.as_deref(),
        args.sign,
        args.version_override.as_deref(),
    )
}
