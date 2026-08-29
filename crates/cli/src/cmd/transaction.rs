//! Logic for the `transaction` command.

use anyhow::Result;
use colored::Colorize;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};

use crate::pkg::{local, transaction, types};

/// Returns the source of the installed manifest.
fn manifest_source(manifest: &types::InstallManifest) -> String {
    local::installed_manifest_source(manifest)
}

/// List all transaction logs.
///
/// # Errors
///
/// Returns an error if the transaction list cannot be retrieved.
pub fn list() -> Result<()> {
    let transactions = transaction::list_transactions()?;
    if transactions.is_empty() {
        println!("No transaction logs found.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_style(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["ID", "Started", "Operations"]);

    for entry in transactions {
        table.add_row(vec![
            entry.id,
            entry.start_time,
            entry.operation_count.to_string(),
        ]);
    }

    println!("{table}");
    Ok(())
}

/// List modified files for a specific transaction.
///
/// # Errors
///
/// Returns an error if the modified files for the given transaction ID cannot
/// be retrieved.
pub fn files(transaction_id: &str) -> Result<()> {
    let mut modified_files = transaction::get_modified_files(transaction_id)?;
    modified_files.sort();

    if modified_files.is_empty() {
        println!(
            "No modified files recorded for transaction '{transaction_id}'."
        );
        return Ok(());
    }

    println!(
        "{} Files modified by transaction '{}':",
        "::".bold().blue(),
        transaction_id.cyan()
    );
    for path in modified_files {
        println!("  - {path}");
    }
    Ok(())
}

/// Show details for a specific transaction.
///
/// # Errors
///
/// Returns an error if the transaction with the given ID cannot be read.
pub fn show(transaction_id: &str) -> Result<()> {
    let transaction = transaction::read_transaction(transaction_id)?;

    println!(
        "{} Transaction {}",
        "::".bold().blue(),
        transaction.id.cyan()
    );
    println!("Started: {}", transaction.start_time);
    println!("Operations: {}", transaction.operations.len());

    for (index, operation) in transaction.operations.iter().enumerate() {
        match operation {
            types::TransactionOperation::Install { manifest } => {
                println!(
                    "{}. install {}",
                    index + 1,
                    manifest_source(manifest).green()
                );
            }
            types::TransactionOperation::Uninstall { manifest } => {
                println!(
                    "{}. uninstall {}",
                    index + 1,
                    manifest_source(manifest).red()
                );
            }
            types::TransactionOperation::Upgrade {
                old_manifest,
                new_manifest
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

    Ok(())
}

/// Undoes a transaction by reverting all of its recorded operations in
/// reverse order. Falls back to the most recent transaction when no ID is
/// given.
///
/// # Errors
///
/// Returns an error if the transaction cannot be read or the revert fails.
pub fn undo(id: Option<&str>, yes: bool, global_yes: bool) -> Result<()> {
    use crate::utils;

    let skip_confirm = yes || global_yes;
    if !utils::ask_for_confirmation(
        "This will revert all operations recorded in the transaction. Are you \
         sure?",
        skip_confirm
    ) {
        println!("Operation aborted.");
        return Ok(());
    }

    let transaction_id = match id {
        Some(id) => id.to_string(),
        None => transaction::get_last_transaction_id()?
            .ok_or_else(|| anyhow::anyhow!("No transactions found to undo."))?
    };

    println!("Undoing transaction {transaction_id}...");
    transaction::rollback(&transaction_id)?;
    println!("{}", "Transaction undone successfully.".green());
    Ok(())
}
