use anyhow::{Result, anyhow};
use clap::Parser;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct DoctorCommand {
    /// Path to the package file (e.g. path/to/name.pkg.lua)
    #[arg(required = true)]
    pub package_file: PathBuf,

    /// Validate as this target platform (defaults to current platform)
    #[arg(long)]
    pub platform: Option<String>,

    /// Override package version while validating
    #[arg(long)]
    pub version_override: Option<String>,
}

pub fn run(args: DoctorCommand) -> Result<()> {
    println!(
        "{} Running package doctor for {}",
        "::".bold().blue(),
        args.package_file.display()
    );

    let report = crate::pkg::package::doctor::run(
        &args.package_file,
        args.platform.as_deref(),
        args.version_override.as_deref(),
    )?;

    for error in &report.errors {
        eprintln!("{} {}", "Error:".red().bold(), error);
    }

    for warning in &report.warnings {
        println!("{} {}", "Warning:".yellow().bold(), warning);
    }

    if report.errors.is_empty() {
        println!(
            "{} package doctor completed (warnings: {}).",
            "::".bold().green(),
            report.warnings.len()
        );
        Ok(())
    } else {
        Err(anyhow!(
            "package doctor found {} error(s)",
            report.errors.len()
        ))
    }
}
