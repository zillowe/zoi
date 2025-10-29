use crate::pkg::{
    install::{self, flow::InstallMode},
    types, uninstall,
};
use anyhow::{Result, anyhow};
use chrono::Utc;
use colored::*;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::{Timestamp, Uuid};

fn get_transactions_dir() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    let dir = home_dir.join(".zoi").join("transactions");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn get_transaction_path(id: &str) -> Result<PathBuf> {
    Ok(get_transactions_dir()?.join(format!("{}.json", id)))
}

pub fn begin() -> Result<types::Transaction> {
    let transaction = types::Transaction {
        id: Uuid::new_v7(Timestamp::from_unix(
            uuid::NoContext,
            Utc::now().timestamp_millis() as u64,
            0,
        ))
        .to_string(),
        start_time: Utc::now().to_rfc3339(),
        operations: Vec::new(),
    };
    let path = get_transaction_path(&transaction.id)?;
    let content = serde_json::to_string_pretty(&transaction)?;
    fs::write(path, content)?;
    Ok(transaction)
}

pub fn record_operation(
    transaction_id: &str,
    operation: types::TransactionOperation,
) -> Result<()> {
    let path = get_transaction_path(transaction_id)?;
    let content = fs::read_to_string(&path)?;
    let mut transaction: types::Transaction = serde_json::from_str(&content)?;
    transaction.operations.push(operation);
    let new_content = serde_json::to_string_pretty(&transaction)?;
    fs::write(path, new_content)?;
    Ok(())
}

pub fn commit(transaction_id: &str) -> Result<()> {
    delete_log(transaction_id)
}

pub fn delete_log(transaction_id: &str) -> Result<()> {
    let path = get_transaction_path(transaction_id)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn rollback(transaction_id: &str) -> Result<()> {
    let path = get_transaction_path(transaction_id)?;
    if !path.exists() {
        return Err(anyhow!(
            "Transaction log not found for ID: {}",
            transaction_id
        ));
    }
    let content = fs::read_to_string(&path)?;
    let transaction: types::Transaction = serde_json::from_str(&content)?;

    println!("\n{}", "--- Starting Rollback ---".yellow().bold());

    for operation in transaction.operations.iter().rev() {
        match operation {
            types::TransactionOperation::Install { manifest } => {
                println!(
                    "Rolling back installation of {} v{}...",
                    manifest.name.cyan(),
                    manifest.version.yellow()
                );
                if let Err(e) = uninstall::run(&manifest.name, Some(manifest.scope)) {
                    eprintln!(
                        "{} Failed to rollback install of '{}': {}",
                        "Error:".red().bold(),
                        manifest.name,
                        e
                    );
                }
            }
            types::TransactionOperation::Uninstall { manifest } => {
                println!(
                    "Rolling back uninstallation of {} v{}...",
                    manifest.name.cyan(),
                    manifest.version.yellow()
                );
                let source = format!(
                    "#{}@{}/{}@{}",
                    manifest.registry_handle, manifest.repo, manifest.name, manifest.version
                );
                if let Err(e) = install::run_installation(
                    &source,
                    InstallMode::PreferPrebuilt,
                    true,
                    manifest.reason.clone(),
                    true,
                    true,
                    true,
                    &Mutex::new(HashSet::new()),
                    Some(manifest.scope),
                    None,
                    None,
                ) {
                    eprintln!(
                        "{} Failed to rollback uninstall of '{}': {}",
                        "Error:".red().bold(),
                        manifest.name,
                        e
                    );
                }
            }
            types::TransactionOperation::Upgrade {
                old_manifest,
                new_manifest,
            } => {
                println!(
                    "Rolling back upgrade of {} from {} to {}...",
                    old_manifest.name.cyan(),
                    new_manifest.version.yellow(),
                    old_manifest.version.green()
                );
                if let Err(e) = uninstall::run(&new_manifest.name, Some(new_manifest.scope)) {
                    eprintln!(
                        "{} Failed to uninstall new version during upgrade-rollback for '{}': {}",
                        "Error:".red().bold(),
                        new_manifest.name,
                        e
                    );
                }
                let source = format!(
                    "#{}@{}/{}@{}",
                    old_manifest.registry_handle,
                    old_manifest.repo,
                    old_manifest.name,
                    old_manifest.version
                );
                if let Err(e) = install::run_installation(
                    &source,
                    InstallMode::PreferPrebuilt,
                    true,
                    old_manifest.reason.clone(),
                    true,
                    true,
                    true,
                    &Mutex::new(HashSet::new()),
                    Some(old_manifest.scope),
                    None,
                    None,
                ) {
                    eprintln!(
                        "{} Failed to re-install old version during upgrade-rollback for '{}': {}",
                        "Error:".red().bold(),
                        old_manifest.name,
                        e
                    );
                }
            }
        }
    }

    println!("{}", "--- Rollback Complete ---".yellow().bold());
    delete_log(transaction_id)?;
    Ok(())
}

pub fn get_last_transaction_id() -> Result<Option<String>> {
    let dir = get_transactions_dir()?;
    let mut last_modified_time = None;
    let mut last_transaction_id = None;

    if !dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let metadata = fs::metadata(&path)?;
            let modified_time = metadata.modified()?;

            if last_modified_time.is_none() || modified_time > last_modified_time.unwrap() {
                last_modified_time = Some(modified_time);
                last_transaction_id = path.file_stem().and_then(|s| s.to_str()).map(String::from);
            }
        }
    }

    Ok(last_transaction_id)
}
