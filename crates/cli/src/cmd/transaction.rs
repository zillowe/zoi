//! Logic for the `transaction` command.

use anyhow::Result;
use colored::Colorize;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};

use crate::pkg::{local, transaction, types};

/// Returns the source of the installed manifest.
fn manifest_source(manifest: &types::InstallManifest,) -> String {
    local::installed_manifest_source(manifest,)
}

/// List all transaction logs.
///
/// # Errors
///
/// Returns an error if the transaction list cannot be retrieved.
pub fn list() -> Result<(),> {
    let transactions = transaction::list_transactions()?;
    if transactions.is_empty() {
        println!("No transaction logs found.");
        return Ok((),);
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL,)
        .set_content_arrangement(ContentArrangement::Dynamic,)
        .set_header(vec!["ID", "Started", "Operations"],);

    for entry in transactions {
        table.add_row(vec![
            entry.id,
            entry.start_time,
            entry.operation_count.to_string(),
        ],);
    }

    println!("{table}");
    Ok((),)
}

/// List modified files for a specific transaction.
///
/// # Errors
///
/// Returns an error if the modified files for the given transaction ID cannot
/// be retrieved.
pub fn files(transaction_id: &str,) -> Result<(),> {
    let mut modified_files = transaction::get_modified_files(transaction_id,)?;
    modified_files.sort();

    if modified_files.is_empty() {
        println!(
            "No modified files recorded for transaction '{transaction_id}'."
        );
        return Ok((),);
    }

    println!(
        "{} Files modified by transaction '{}':",
        "::".bold().blue(),
        transaction_id.cyan()
    );
    for path in modified_files {
        println!("  - {path}");
    }
    Ok((),)
}

/// Show details for a specific transaction.
///
/// # Errors
///
/// Returns an error if the transaction with the given ID cannot be read.
pub fn show(transaction_id: &str,) -> Result<(),> {
    let transaction = transaction::read_transaction(transaction_id,)?;

    println!(
        "{} Transaction {}",
        "::".bold().blue(),
        transaction.id.cyan()
    );
    println!("Started: {}", transaction.start_time);
    println!("Operations: {}", transaction.operations.len());

    for (index, operation,) in transaction.operations.iter().enumerate() {
        match operation {
            types::TransactionOperation::Install { manifest, } => {
                println!(
                    "{}. install {}",
                    index + 1,
                    manifest_source(manifest).green()
                );
            }
            types::TransactionOperation::Uninstall { manifest, } => {
                println!(
                    "{}. uninstall {}",
                    index + 1,
                    manifest_source(manifest).red()
                );
            }
            types::TransactionOperation::Upgrade {
                old_manifest,
                new_manifest,
            } => {
                println!(
                    "{}. upgrade {} -> {}",
                    index + 1,
                    manifest_source(old_manifest).yellow(),
                    manifest_source(new_manifest).green()
                );
            }
        }
    }

    Ok((),)
}
