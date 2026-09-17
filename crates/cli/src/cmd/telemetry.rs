//! Telemetry command implementation.

use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;

/// Crash-report subcommands (`zoi telemetry crash ...`).
#[derive(Debug, Clone)]
pub enum CrashCommand {
    /// List locally stored crash reports.
    List,
    /// Print a crash report envelope to stdout.
    Show {
        /// Crash report file name or full path.
        file: PathBuf
    },
    /// Upload a crash report to Sentry (requires telemetry opt-in).
    Send {
        /// Crash report file name or full path.
        file: PathBuf,
        /// Skip the confirmation prompt.
        yes: bool
    },
    /// Delete all locally stored crash reports.
    Clear {
        /// Skip the confirmation prompt.
        yes: bool
    }
}

/// Telemetry subcommands.
#[derive(Debug, Clone)]
pub enum TelemetryCommand {
    /// Show telemetry status.
    Status,
    /// Enable telemetry.
    Enable,
    /// Disable telemetry.
    Disable,
    /// Manage local crash reports.
    Crash(CrashCommand),
    /// Deliver queued analytics events (internal flusher entry point).
    Flush
}

/// Resolves a user-supplied crash file (bare name or path) against the crash
/// directory.
///
/// # Errors
///
/// Returns an error if the file does not exist.
fn resolve_crash_file(file: &PathBuf) -> Result<PathBuf> {
    if file.exists() {
        return Ok(file.clone());
    }
    let dir = zoi_telemetry::crash::list_crashes();
    for known in dir {
        if known.ends_with(file)
            || known.file_name().is_some_and(|n| n == file.as_os_str())
        {
            return Ok(known);
        }
    }
    Err(anyhow::anyhow!(
        "Crash report not found: {}. Run 'zoi telemetry crash list' to see \
         available reports.",
        file.display()
    ))
}

/// Run the telemetry command.
///
/// # Errors
///
/// Returns an error if the configuration cannot be read or written, or if a
/// crash-report operation fails.
pub fn run(cmd: TelemetryCommand) -> Result<()> {
    match cmd {
        TelemetryCommand::Status => {
            let cfg = crate::pkg::config::read_config()?;
            let status = if cfg.telemetry_enabled {
                "Enabled".green()
            } else {
                "Disabled".yellow()
            };
            println!(
                "{} telemetry is currently {}.",
                "::".bold().blue(),
                status
            );

            if cfg.telemetry_enabled {
                let id = crate::pkg::telemetry::get_anonymous_id();
                println!("Anonymous Client ID: {}", id.cyan());
                println!(
                    "\nThank you for helping us improve Zoi! We collect \
                     minimal, anonymous data about:"
                );
                println!(
                    "- {} (OS, Arch, Distro, Shell)",
                    "Environment".bold()
                );
                println!("- {} (Action, Scope, Reason)", "Operations".bold());
                println!(
                    "- {} (Name, Version, Repo, License)",
                    "Package Metadata".bold()
                );
                let crashes = crate::pkg::telemetry::crash::list_crashes();
                if crashes.is_empty() {
                    println!(
                        "- {}: none stored locally",
                        "Crash Reports".bold()
                    );
                } else {
                    println!(
                        "- {}: {} stored locally (see 'zoi telemetry crash \
                         list')",
                        "Crash Reports".bold(),
                        crashes.len()
                    );
                }
            } else {
                println!(
                    "\nTelemetry is anonymous and helps us prioritize \
                     features and platforms."
                );
                println!("Run 'zoi telemetry enable' to help the project.");
                println!(
                    "Note: crash reports are still saved locally to \
                     $XDG_STATE_HOME/zoi/crash/ but never uploaded while \
                     telemetry is disabled."
                );
            }
        }
        TelemetryCommand::Enable => {
            let mut cfg = crate::pkg::config::read_user_config()?;

            println!(
                "{}",
                "Notice: Enabling telemetry shares anonymous usage data to \
                 help improve Zoi."
                    .dimmed()
            );
            println!(
                "{}",
                "Success analytics contain no personal data or IP addresses. \
                 Failure reports may include error text with local paths."
                    .dimmed()
            );
            println!(
                "{} {}",
                "Full Privacy Policy:".dimmed(),
                "https://zillowe.qzz.io/legal/privacy".cyan()
            );

            cfg.telemetry_enabled = true;
            crate::pkg::config::write_user_config(&cfg)?;
            println!("{} telemetry enabled", "Success:".green());
        }
        TelemetryCommand::Disable => {
            let mut cfg = crate::pkg::config::read_user_config()?;
            cfg.telemetry_enabled = false;
            crate::pkg::config::write_user_config(&cfg)?;
            println!("{} telemetry disabled", "Success:".green());
        }
        TelemetryCommand::Crash(crash_cmd) => run_crash(crash_cmd)?,
        TelemetryCommand::Flush => {
            let summary = crate::pkg::telemetry::queue::flush_queue();
            println!(
                "Telemetry flush: {} sent, {} failed, {} skipped.",
                summary.sent, summary.failed, summary.skipped
            );
        }
    }
    Ok(())
}

/// Runs a `zoi telemetry crash ...` subcommand.
///
/// # Errors
///
/// Returns an error if a crash file cannot be read, uploaded, or deleted.
fn run_crash(cmd: CrashCommand) -> Result<()> {
    match cmd {
        CrashCommand::List => {
            let crashes = crate::pkg::telemetry::crash::list_crashes();
            if crashes.is_empty() {
                println!("No crash reports stored locally.");
                println!(
                    "Crash reports are saved to $XDG_STATE_HOME/zoi/crash/ \
                     (default ~/.local/state/zoi/crash/) with a .zoicrash \
                     extension."
                );
            } else {
                println!("Stored crash reports:");
                for path in crashes {
                    println!("  {}", path.display());
                }
            }
        }
        CrashCommand::Show { file } => {
            let path = resolve_crash_file(&file)?;
            let contents =
                std::fs::read_to_string(&path).with_context(|| {
                    format!("Failed to read {}", path.display())
                })?;
            println!("{contents}");
        }
        CrashCommand::Send { file, yes } => {
            let path = resolve_crash_file(&file)?;
            if !yes
                && !crate::pkg::utils::ask_for_confirmation(
                    &format!(
                        "Upload {} to Sentry? Crash reports may contain stack \
                         memory snapshots.",
                        path.display()
                    ),
                    false
                )
            {
                println!("Upload cancelled.");
                return Ok(());
            }
            let sent = crate::pkg::telemetry::crash::send_crash_report(&path)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if sent {
                println!("{} crash report uploaded", "Success:".green());
                std::fs::remove_file(&path).with_context(|| {
                    format!("Uploaded but failed to remove {}", path.display())
                })?;
            } else {
                println!(
                    "{} telemetry is disabled; crash report was not uploaded. \
                     Enable with 'zoi telemetry enable' first.",
                    "Skipped:".yellow()
                );
            }
        }
        CrashCommand::Clear { yes } => {
            let crashes = crate::pkg::telemetry::crash::list_crashes();
            if crashes.is_empty() {
                println!("No crash reports to delete.");
                return Ok(());
            }
            if !yes
                && !crate::pkg::utils::ask_for_confirmation(
                    &format!(
                        "Delete {} stored crash report(s)?",
                        crashes.len()
                    ),
                    false
                )
            {
                println!("Cancelled.");
                return Ok(());
            }
            for path in crashes {
                std::fs::remove_file(&path).with_context(|| {
                    format!("Failed to remove {}", path.display())
                })?;
            }
            println!("{} crash reports cleared", "Success:".green());
        }
    }
    Ok(())
}
