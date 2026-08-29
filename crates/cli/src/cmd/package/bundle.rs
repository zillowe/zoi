//! Implementation of the `package bundle` command.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

/// Arguments for the `package bundle` command.
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

    /// How to attach the signature when --sign is used. `embed` stores the
    /// signature inside the bundle; `file` writes a legacy `.sig` sidecar.
    #[arg(long, value_enum, default_value_t = super::build::SignModeArg::Embed)]
    pub sign_mode: super::build::SignModeArg,

    /// Override the package version
    #[arg(long)]
    pub version_override: Option<String>,

    /// The build type to bundle (e.g. 'source', 'pre-compiled')
    #[arg(long, short = 't')]
    pub build_type: Option<String>
}

/// Runs the `package bundle` command.
///
/// # Errors
///
/// Returns an error if the package cannot be bundled, if there are missing
/// dependencies, or if there is an error writing the bundle file.
pub fn run(args: BundleCommand) -> Result<()> {
    crate::pkg::package::bundle::run(
        &args.package_file,
        args.output_dir.as_deref(),
        args.sign,
        args.sign_mode.into(),
        args.version_override.as_deref(),
        args.build_type.as_deref()
    )?;
    Ok(())
}
