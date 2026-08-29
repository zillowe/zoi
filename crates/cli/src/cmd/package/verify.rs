//! Implementation of the `package verify` command.
//!
//! Verifies the integrity of installed packages (rpm -V style, against the
//! digests recorded at install time) or of a `.zpa` archive (against its
//! internal pooled manifest).

use std::path::PathBuf;

use anyhow::Result;
use clap::ValueEnum;
use colored::Colorize;
use serde_json::json;

use crate::pkg::install::verify;
use crate::pkg::{local, types};

/// What to verify.
#[derive(ValueEnum, Debug, Clone)]
pub enum VerifyTarget {
    /// Verify installed packages.
    Installed,
    /// Verify a `.zpa` archive.
    Archive
}

/// Arguments for the `zoi package verify` command.
#[derive(clap::Parser, Debug)]
pub struct VerifyCommand {
    /// What to verify.
    #[arg(default_value = "installed")]
    pub target: VerifyTarget,

    /// Packages to verify (defaults to every installed package). Only used
    /// with the `installed` target.
    #[arg(value_name = "PACKAGES")]
    pub packages: Vec<String>,

    /// Path to the `.zpa` archive to verify (required with the `archive`
    /// target).
    #[arg(long = "file")]
    pub archive: Option<PathBuf>,

    /// Restrict verification to the given scope (installed target only).
    #[arg(long, value_enum)]
    pub scope: Option<crate::cli::SetupScope>,

    /// Output results as JSON.
    #[arg(long)]
    pub json: bool
}

/// Runs the verify command.
///
/// # Errors
///
/// Returns an error if reading manifests or hashing files fails. Exits with
/// a non-zero status when any verified item reports a problem.
pub fn run(cmd: &VerifyCommand) -> Result<()> {
    match cmd.target {
        VerifyTarget::Archive => run_archive(cmd),
        VerifyTarget::Installed => run_installed(cmd)
    }
}

/// Runs archive-level verification against the provided file.
fn run_archive(cmd: &VerifyCommand) -> Result<()> {
    let Some(archive) = cmd.archive.as_ref() else {
        return Err(anyhow::anyhow!(
            "The --file option is required when verifying archives"
        ));
    };

    let report = verify::verify_archive(archive)?;

    if cmd.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "{} Verifying archive {}",
            "::".bold().blue(),
            report.archive
        );
        if report.signed_embed {
            println!("  Embedded signature: present");
        }
        println!("  Pooled files checked: {}", report.checked);

        if report.ok {
            println!("{}", "Archive integrity verified.".green());
        } else {
            for issue in &report.issues {
                println!("  {}: {issue}", "PROBLEM".red().bold());
            }
        }
    }

    if !report.ok {
        std::process::exit(1);
    }
    Ok(())
}

/// Runs installed-package verification for matching packages.
fn run_installed(cmd: &VerifyCommand) -> Result<()> {
    let all = local::get_installed_packages()?;
    let manifests: Vec<_> = if cmd.packages.is_empty() {
        all
    } else {
        let wanted = &cmd.packages;
        let mut found = Vec::new();
        for manifest in all {
            let matches_name = wanted.iter().any(|p| {
                p.eq_ignore_ascii_case(&manifest.name)
                    || p.eq_ignore_ascii_case(&manifest.repo)
                    || manifest.sub_package.as_deref() == Some(p.as_str())
                    || format!("@{}/{}", manifest.repo, manifest.name)
                        .eq_ignore_ascii_case(p)
            });
            if matches_name {
                found.push(manifest);
            } else {
                eprintln!(
                    "{}: Package not installed: {}",
                    "Warning".yellow().bold(),
                    // Reconstruct which name failed is ambiguous; report the
                    // unmatched package by its identity instead.
                    local::installed_manifest_source(&manifest)
                );
            }
        }
        found
    };

    let manifests: Vec<_> = manifests
        .into_iter()
        .filter(|m| {
            cmd.scope.is_none_or(|s| {
                matches!(
                    (s, m.scope),
                    (crate::cli::SetupScope::User, types::Scope::User)
                        | (
                            crate::cli::SetupScope::System,
                            types::Scope::System
                        )
                )
            })
        })
        .collect();

    if manifests.is_empty() {
        if !cmd.json {
            println!("No installed packages matched.");
        }
        return Ok(());
    }

    let mut total_bad = 0usize;
    let mut json_reports = Vec::new();

    for manifest in &manifests {
        let statuses = verify::verify_installed(manifest)?;
        let bad: Vec<_> = statuses
            .iter()
            .filter(|s| {
                !matches!(
                    s.status,
                    verify::VerifyStatus::Ok | verify::VerifyStatus::Unverified
                )
            })
            .collect();

        if cmd.json {
            json_reports.push(json!({
                "name": manifest.name,
                "version": manifest.version,
                "sub_package": manifest.sub_package,
                "scope": format!("{:?}", manifest.scope).to_lowercase(),
                "files": statuses,
                "ok": bad.is_empty()
            }));
        } else {
            let display_name = match &manifest.sub_package {
                Some(sub) => format!("{}:{sub}", manifest.name),
                None => manifest.name.clone()
            };
            let scope_tag = match manifest.scope {
                types::Scope::User => "",
                types::Scope::System => " [system]",
                types::Scope::Project => " [project]"
            };
            println!(
                "{} {} v{}{scope_tag}",
                "::".bold().blue(),
                display_name.cyan(),
                manifest.version.yellow()
            );

            for status in &statuses {
                if !matches!(
                    status.status,
                    verify::VerifyStatus::Ok | verify::VerifyStatus::Unverified
                ) {
                    println!(
                        "  {}: {}",
                        status.status.as_tag().red().bold(),
                        status.path
                    );
                }
            }

            if bad.is_empty() {
                let unverified = statuses
                    .iter()
                    .filter(|s| s.status == verify::VerifyStatus::Unverified)
                    .count();
                if unverified > 0 {
                    println!(
                        "  {} ({unverified} file(s) unverified)",
                        "no problems detected".green()
                    );
                } else {
                    println!("{}", "all files verified".green());
                }
            } else {
                total_bad += bad.len();
            }
        }
    }

    if cmd.json {
        println!("{}", serde_json::to_string_pretty(&json_reports)?);
    } else if total_bad > 0 {
        eprintln!(
            "{}: {total_bad} file(s) failed verification",
            "FAILED".red().bold()
        );
        std::process::exit(1);
    }

    Ok(())
}
