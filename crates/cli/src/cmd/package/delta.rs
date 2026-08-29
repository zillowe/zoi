//! Implementation of the `package delta` command.
//!
//! Generates a `.zdelta` file that transforms an older `.zpa` archive into a
//! newer one using content-addressed pool matching and bsdiff patches.

use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;

/// Arguments for the `zoi package delta` command.
#[derive(clap::Parser, Debug)]
pub struct DeltaCommand {
    /// The older `.zpa` archive (the delta's base).
    pub old: PathBuf,

    /// The newer `.zpa` archive (the delta's target).
    pub new: PathBuf,

    /// Where to write the `.zdelta`. Defaults to
    /// `<new>.from-v<old>-to-v<new>.zdelta` next to the new archive when the
    /// archives carry version suffixes, otherwise `<new>.zdelta`.
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Sign the delta patch with a PGP key from the Zoi keyring.
    #[arg(long)]
    pub sign: Option<String>
}

/// Runs the `package delta` command.
///
/// # Errors
///
/// Returns an error if either archive cannot be read or the delta cannot be
/// generated.
pub fn run(cmd: &DeltaCommand) -> Result<()> {
    let output = cmd
        .output
        .clone()
        .unwrap_or_else(|| default_output(&cmd.new));
    crate::pkg::delta::create_zpa_delta(
        &cmd.old,
        &cmd.new,
        &output,
        cmd.sign.as_deref()
    )?;
    println!(
        "{} Apply it with 'zoi package install --apply-delta' or distribute \
         it alongside the release.",
        "::".bold().blue()
    );
    Ok(())
}

/// Derives a default output path for the delta file from the new archive
/// name.
fn default_output(new_archive: &std::path::Path) -> PathBuf {
    // Best-effort convention: <name>-vA-to-vB.zdelta derived from the new
    // archive name; falls back to appending .zdelta.
    let stem = new_archive.file_name().map_or_else(
        || "package.zpa".to_string(),
        |f| f.to_string_lossy().to_string()
    );
    new_archive.with_file_name(format!("{stem}.zdelta"))
}
