use crate::pkg::{audit, install, local, types, uninstall};
use anyhow::{Result, anyhow};
use chrono::Utc;
use colored::*;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use uuid::{Timestamp, Uuid};

#[derive(Debug, Clone)]
pub struct TransactionMetadata {
    pub id: String,
    pub start_time: String,
    pub operation_count: usize,
}

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

pub fn read_transaction(transaction_id: &str) -> Result<types::Transaction> {
    let path = get_transaction_path(transaction_id)?;
    if !path.exists() {
        return Err(anyhow!(
            "Transaction log not found for ID: {}",
            transaction_id
        ));
    }
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

pub fn record_operation(
    transaction_id: &str,
    operation: types::TransactionOperation,
) -> Result<()> {
    match &operation {
        types::TransactionOperation::Install { manifest } => {
            let _ = audit::log_event(audit::AuditAction::Install, manifest);
        }
        types::TransactionOperation::Uninstall { manifest } => {
            let _ = audit::log_event(audit::AuditAction::Uninstall, manifest);
        }
        types::TransactionOperation::Upgrade { new_manifest, .. } => {
            let _ = audit::log_event(audit::AuditAction::Upgrade, new_manifest);
        }
    }

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

pub fn get_modified_files(transaction_id: &str) -> Result<Vec<String>> {
    let path = get_transaction_path(transaction_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)?;
    let transaction: types::Transaction = serde_json::from_str(&content)?;

    let mut files = HashSet::new();
    for op in transaction.operations {
        match op {
            types::TransactionOperation::Install { manifest } => {
                for file in manifest.installed_files {
                    files.insert(file);
                }
            }
            types::TransactionOperation::Uninstall { manifest } => {
                for file in manifest.installed_files {
                    files.insert(file);
                }
            }
            types::TransactionOperation::Upgrade {
                old_manifest,
                new_manifest,
            } => {
                for file in old_manifest.installed_files {
                    files.insert(file);
                }
                for file in new_manifest.installed_files {
                    files.insert(file);
                }
            }
        }
    }
    Ok(files.into_iter().collect())
}

pub fn delete_log(transaction_id: &str) -> Result<()> {
    let path = get_transaction_path(transaction_id)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn list_transactions() -> Result<Vec<TransactionMetadata>> {
    let dir = get_transactions_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut transactions = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let content = fs::read_to_string(&path)?;
        let transaction: types::Transaction = serde_json::from_str(&content)?;
        transactions.push(TransactionMetadata {
            id: transaction.id,
            start_time: transaction.start_time,
            operation_count: transaction.operations.len(),
        });
    }

    transactions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    Ok(transactions)
}

fn has_files_outside_store(manifest: &types::InstallManifest) -> bool {
    if let Ok(store_base) = local::get_store_base_dir(manifest.scope) {
        for file in &manifest.installed_files {
            let p = std::path::Path::new(file);
            if !p.starts_with(&store_base) {
                return true;
            }
        }
    }
    false
}

fn install_source_for_manifest(manifest: &types::InstallManifest) -> String {
    local::installed_manifest_source(manifest)
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

    println!("\n{} Starting Rollback...", "::".bold().blue());

    for operation in transaction.operations.iter().rev() {
        match operation {
            types::TransactionOperation::Install { manifest } => {
                println!(
                    "Rolling back installation of {} v{}...",
                    manifest.name.cyan(),
                    manifest.version.yellow()
                );
                let source = install_source_for_manifest(manifest);
                if let Err(e) = uninstall::run(&source, Some(manifest.scope), true) {
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

                let version_dir = match local::get_package_version_dir(
                    manifest.scope,
                    &manifest.registry_handle,
                    &manifest.repo,
                    &manifest.name,
                    &manifest.version,
                ) {
                    Ok(dir) => dir,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to get version directory for rollback: {}",
                            "Error:".red().bold(),
                            e
                        );
                        continue;
                    }
                };

                let manifest_filename = if let Some(sub) = &manifest.sub_package {
                    format!("manifest-{}.yaml", sub)
                } else {
                    "manifest.yaml".to_string()
                };
                let manifest_path = version_dir.join(&manifest_filename);

                if version_dir.exists()
                    && manifest_path.exists()
                    && !has_files_outside_store(manifest)
                {
                    println!("Restoring version {} from local store...", manifest.version);
                    if let Err(e) = local::write_manifest(manifest) {
                        eprintln!(
                            "{} Failed to restore manifest for '{}': {}",
                            "Error:".red().bold(),
                            manifest.name,
                            e
                        );
                    }
                    continue;
                }

                println!(
                    "Version not found locally or contains global files. Re-installing from registry..."
                );

                let source = install_source_for_manifest(manifest);
                let (graph, _) = match install::resolver::resolve_dependency_graph(
                    &[source],
                    Some(manifest.scope),
                    true,
                    true,
                    true,
                    None,
                    true,
                ) {
                    Ok(res) => res,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to resolve dependency graph for rollback of '{}': {}",
                            "Error:".red().bold(),
                            manifest.name,
                            e
                        );
                        continue;
                    }
                };

                let install_plan =
                    match install::plan::create_install_plan(&graph.nodes, None, false) {
                        Ok(plan) => plan,
                        Err(e) => {
                            eprintln!(
                                "{} Failed to create install plan for rollback of '{}': {}",
                                "Error:".red().bold(),
                                manifest.name,
                                e
                            );
                            continue;
                        }
                    };

                let stages = match graph.toposort() {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to sort dependency graph for rollback of '{}': {}",
                            "Error:".red().bold(),
                            manifest.name,
                            e
                        );
                        continue;
                    }
                };

                for stage in stages {
                    for id in stage {
                        let Some(node) = graph.nodes.get(&id) else {
                            continue;
                        };
                        if let Some(action) = install_plan.get(&id)
                            && let Err(e) = install::installer::install_node(
                                node, action, None, None, true, true,
                            )
                        {
                            eprintln!(
                                "{} Failed to re-install during rollback of '{}': {}",
                                "Error:".red().bold(),
                                manifest.name,
                                e
                            );
                        }
                    }
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
                let source = install_source_for_manifest(new_manifest);
                if let Err(e) = uninstall::run(&source, Some(new_manifest.scope), true) {
                    eprintln!(
                        "{} Failed to uninstall new version during upgrade-rollback for '{}': {}",
                        "Error:".red().bold(),
                        new_manifest.name,
                        e
                    );
                }

                let version_dir = match local::get_package_version_dir(
                    old_manifest.scope,
                    &old_manifest.registry_handle,
                    &old_manifest.repo,
                    &old_manifest.name,
                    &old_manifest.version,
                ) {
                    Ok(dir) => dir,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to get version directory for rollback: {}",
                            "Error:".red().bold(),
                            e
                        );
                        continue;
                    }
                };

                let manifest_filename = if let Some(sub) = &old_manifest.sub_package {
                    format!("manifest-{}.yaml", sub)
                } else {
                    "manifest.yaml".to_string()
                };
                let manifest_path = version_dir.join(&manifest_filename);

                if version_dir.exists()
                    && manifest_path.exists()
                    && !has_files_outside_store(old_manifest)
                {
                    println!(
                        "Restoring version {} from local store...",
                        old_manifest.version
                    );
                    if let Err(e) = local::write_manifest(old_manifest) {
                        eprintln!(
                            "{} Failed to restore manifest for '{}': {}",
                            "Error:".red().bold(),
                            old_manifest.name,
                            e
                        );
                    }
                    continue;
                }

                println!(
                    "Version not found locally or contains global files. Re-installing from registry..."
                );

                let source = install_source_for_manifest(old_manifest);
                let (graph, _) = match install::resolver::resolve_dependency_graph(
                    std::slice::from_ref(&source),
                    Some(old_manifest.scope),
                    true,
                    true,
                    true,
                    None,
                    true,
                ) {
                    Ok(res) => res,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to resolve dependency graph for rollback of '{}': {}",
                            "Error:".red().bold(),
                            old_manifest.name,
                            e
                        );
                        continue;
                    }
                };

                let install_plan =
                    match install::plan::create_install_plan(&graph.nodes, None, false) {
                        Ok(plan) => plan,
                        Err(e) => {
                            eprintln!(
                                "{} Failed to create install plan for rollback of '{}': {}",
                                "Error:".red().bold(),
                                old_manifest.name,
                                e
                            );
                            continue;
                        }
                    };

                let stages = match graph.toposort() {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to sort dependency graph for rollback of '{}': {}",
                            "Error:".red().bold(),
                            old_manifest.name,
                            e
                        );
                        continue;
                    }
                };

                for stage in stages {
                    for id in stage {
                        let Some(node) = graph.nodes.get(&id) else {
                            continue;
                        };
                        if let Some(action) = install_plan.get(&id)
                            && let Err(e) = install::installer::install_node(
                                node, action, None, None, true, true,
                            )
                        {
                            eprintln!(
                                "{} Failed to re-install during rollback of '{}': {}",
                                "Error:".red().bold(),
                                old_manifest.name,
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    println!("{}", ":: Rollback Complete".bold().blue());
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

            if last_modified_time.is_none_or(|last| modified_time > last) {
                last_modified_time = Some(modified_time);
                last_transaction_id = path.file_stem().and_then(|s| s.to_str()).map(String::from);
            }
        }
    }

    Ok(last_transaction_id)
}
