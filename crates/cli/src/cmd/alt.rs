//! The `zoi alt` command: inspecting and choosing alternative implementations.
//!
//! `awk`, `vi` and `editor` each have several interchangeable implementations,
//! and the system still has to put *something* at the canonical path. This
//! command is how an administrator sees what is registered and changes which
//! one is current, without reinstalling anything.
//!
//! See `crates/core/src/alternatives.rs` for the mechanism and the layout.

use anyhow::{Result, bail};
use colored::Colorize;
use comfy_table::Table;
use comfy_table::presets::UTF8_FULL;
use zoi_core::alternatives::{self, AlternativeStatus};

use crate::cli::AltCommand;

/// Runs the `alt` subcommand.
///
/// The argument is the clap-generated enum rather than a hand-rolled one, so
/// `zoi alt --help` and the shell completions stay in step with the
/// implementation automatically.
///
/// # Errors
///
/// Returns an error if the named alternative does not exist, if `set` names a
/// path that is not registered for it, or if the registry state cannot be read
/// or written. Every failure is reported without touching the current
/// selection, because group state is written to a temporary file and renamed.
pub fn run(args: &AltCommand) -> Result<()> {
    match args {
        AltCommand::List { verbose } => list(*verbose),
        AltCommand::Get { name } => get(name),
        AltCommand::Set { name, path } => set(name, path),
        AltCommand::Auto { name } => auto(name),
        AltCommand::Remove { name, path } => remove(name, path)
    }
}

/// Prints every registered alternative group.
fn list(verbose: bool) -> Result<()> {
    let statuses = alternatives::status()?;

    if statuses.is_empty() {
        println!(
            "No alternatives are registered. A package registers one by \
             declaring an 'alternatives' block in its .pkg.lua."
        );
        return Ok(());
    }

    if !verbose {
        let mut table = Table::new();
        table.load_style(UTF8_FULL);
        table.set_header(vec![
            "Name",
            "Link",
            "Current",
            "Source",
            "Registered",
        ]);

        for status in &statuses {
            let current = status.current.clone().unwrap_or_else(|| "-".into());

            table.add_row(vec![
                status.name.clone(),
                status.link.clone(),
                current,
                if status.manual { "manual" } else { "auto" }.to_string(),
                status
                    .alternatives
                    .iter()
                    .map(|a| format!("{} ({})", a.path, a.priority))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ]);
        }

        println!("{table}");
        return Ok(());
    }

    // Verbose output groups by name so every candidate and its owner is
    // legible.
    for status in &statuses {
        print_group(status);
    }

    Ok(())
}

/// Prints one group in full.
fn print_group(status: &AlternativeStatus) {
    println!("{} -> {}", status.name.bold(), status.link);

    let current = status.current.clone().unwrap_or_else(|| "-".into());
    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header(vec!["Priority", "Path", "Package", "Current"]);

    // Highest priority first, so the listing reads in the order a reader cares
    // about rather than in registration order.
    let mut sorted = status.alternatives.clone();
    sorted.sort_by_key(|a| std::cmp::Reverse(a.priority));

    for alternative in sorted {
        let is_current = Some(&alternative.path) == status.current.as_ref();
        table.add_row(vec![
            alternative.priority.to_string(),
            alternative.path.clone(),
            alternative.package.clone(),
            if is_current {
                "*".to_string()
            } else {
                String::new()
            },
        ]);
    }

    println!("{table}");
    println!(
        "  selected: {} ({})\n",
        current,
        if status.manual {
            "manual"
        } else {
            "highest priority"
        }
    );
}

/// Shows one group.
fn get(name: &str) -> Result<()> {
    let Some(group) = alternatives::get(name)? else {
        bail!("No such alternative: '{name}'. Run 'zoi alt list' to see them.");
    };

    print_group(&group.status());
    Ok(())
}

/// Manually selects an implementation.
fn set(name: &str, path: &str) -> Result<()> {
    alternatives::set(name, path)?;

    let group = alternatives::get(name)?
        .ok_or_else(|| anyhow::anyhow!("Alternative '{name}' disappeared"))?;

    println!(
        "{} '{}' now points to {}",
        "Updated".green().bold(),
        group.link,
        path.bold()
    );

    Ok(())
}

/// Returns to priority-based selection.
fn auto(name: &str) -> Result<()> {
    if let Some(selected) = alternatives::auto(name)? {
        println!(
            "{} '{}' now points to {} (highest priority)",
            "Updated".green().bold(),
            name,
            selected.bold()
        );
        Ok(())
    } else {
        println!("Alternative '{name}' has no registered implementations.");
        Ok(())
    }
}

/// Removes one implementation from a group.
fn remove(name: &str, path: &str) -> Result<()> {
    alternatives::remove_implementation(name, path)?;

    println!(
        "{} '{path}' from alternative '{name}'",
        "Removed".green().bold()
    );

    // Report what is now current, so the user is not left guessing.
    if let Some(group) = alternatives::get(name)?
        && let Some(current) = group.current()
    {
        println!("{} now points to {}", name, current.path.as_str().bold());
    }

    Ok(())
}

/// Announces alternative changes after a transaction.
///
/// Called from the install, update and uninstall paths. Kept here rather than
/// inlined at each call site so the wording stays consistent, and quiet when
/// nothing changed, because most transactions touch no alternatives at all.
pub fn announce_changes(changed: &[String]) {
    if changed.is_empty() {
        return;
    }

    for name in changed {
        match alternatives::get(name) {
            Ok(Some(group)) => match group.current() {
                Some(current) => println!(
                    "{} alternative '{}' -> {}",
                    "::".blue().bold(),
                    name,
                    current.path.as_str().bold()
                ),
                None => println!(
                    "{} alternative '{}' has no implementations",
                    "::".blue().bold(),
                    name
                )
            },
            Ok(None) => {
                println!("{} alternative '{name}' removed", "::".blue().bold());
            }
            // Reporting must never fail a transaction that already succeeded.
            Err(_) => {}
        }
    }
}
