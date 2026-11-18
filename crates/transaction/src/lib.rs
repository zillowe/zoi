//! Transaction management for Zoi package operations.
//!
//! This crate provides the mechanism for recording and rolling back package
//! operations (install, uninstall, upgrade) to ensure system consistency.

/// Rollback logic for Zoi transactions.
pub mod rollback;

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use chrono::Utc;
use colored::Colorize;
use uuid::{Timestamp, Uuid};
use zoi_audit as audit;
use zoi_core::types;
use zoi_install as install;
use zoi_resolver::local;
use zoi_uninstall as uninstall;

/// High-level metadata summarizing a completed or in-progress transaction.
#[derive(Debug, Clone)]
pub struct TransactionMetadata {
    /// The UUID v7 identifier for the transaction.
    pub id: String,
    /// The RFC 3339 timestamp when the transaction began.
    pub start_time: String,
    /// The number of distinct package operations (install, uninstall, upgrade)
    /// recorded.
    pub operation_count: usize
}

/// Gets the directory where transaction logs are stored.
fn get_transactions_dir() -> Result<PathBuf> {
    let dir = zoi_core::utils::get_user_state_dir()?.join("transactions");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Validates an externally supplied transaction identifier before using it in
/// a filesystem path.
fn validate_transaction_id(id: &str) -> Result<()> {
    Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| anyhow!("Invalid transaction ID: {id}"))
}

/// Gets the path to a specific transaction log file.
fn get_transaction_path(id: &str) -> Result<PathBuf> {
    validate_transaction_id(id)?;
    let dir = get_transactions_dir()?;
    let active_path = dir.join(format!("{id}.json"));
    if active_path.exists() {
        return Ok(active_path);
    }
    let history_path = dir.join("history").join(format!("{id}.json"));
    Ok(history_path)
}

/// Starts a new package transaction.
///
/// Returns a `Transaction` object with a UUID v7 ID, which provides both
/// uniqueness and chronological sorting. No log file is written until the
/// first operation is recorded.
///
/// # Errors
///
/// Returns an error if the home directory cannot be found.
pub fn begin() -> Result<types::Transaction> {
    Ok(types::Transaction {
        id: Uuid::new_v7(Timestamp::from_unix(
            uuid::NoContext,
            Utc::now().timestamp_millis().cast_unsigned(),
            0
        ))
        .to_string(),
        start_time: Utc::now().to_rfc3339(),
        operations: Vec::new()
    })
}

/// Reads a transaction from a log file.
///
/// # Errors
///
/// Returns an error if the transaction log file does not exist or cannot be
/// read, or if the content is not valid JSON.
pub fn read_transaction(transaction_id: &str) -> Result<types::Transaction> {
    let path = get_transaction_path(transaction_id)?;
    if !path.exists() {
        return Err(anyhow!(
            "Transaction log not found for ID: {transaction_id}"
        ));
    }
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Records a package operation in the current transaction.
///
/// # Errors
///
/// Returns an error if the audit event cannot be logged or if the transaction
/// log file cannot be written.
pub fn record_operation(
    transaction: &mut types::Transaction,
    operation: types::TransactionOperation
) -> Result<()> {
    match &operation {
        types::TransactionOperation::Install { manifest } => {
            audit::log_event(audit::AuditAction::Install, manifest)?;
        }
        types::TransactionOperation::Uninstall { manifest } => {
            audit::log_event(audit::AuditAction::Uninstall, manifest)?;
        }
        types::TransactionOperation::Upgrade {
            old_manifest: _,
            new_manifest
        } => {
            audit::log_event(audit::AuditAction::Upgrade, new_manifest)?;
        }
    }

    transaction.operations.push(operation);

    let path = get_transactions_dir()?.join(format!("{}.json", transaction.id));
    let content = serde_json::to_string_pretty(&transaction)?;
    // Atomic so a crash mid-write never leaves a truncated log that breaks
    // `list_transactions` and the rollback it exists for.
    local::write_file_atomic(&path, content.as_bytes())?;
    Ok(())
}

/// Commits a transaction by moving it to the history directory.
///
/// # Errors
///
/// Returns an error if the transaction directory cannot be accessed or if
/// the log file cannot be moved to the history directory.
pub fn commit(transaction_id: &str) -> Result<()> {
    validate_transaction_id(transaction_id)?;
    let dir = get_transactions_dir()?;
    let path = dir.join(format!("{transaction_id}.json"));
    if !path.exists() {
        return Ok(());
    }

    let history_dir = dir.join("history");
    fs::create_dir_all(&history_dir)?;
    let dest = history_dir.join(format!("{transaction_id}.json"));
    fs::rename(path, dest)?;
    Ok(())
}

/// Returns a list of all files modified during a transaction.
///
/// # Errors
///
/// Returns an error if the transaction log file cannot be read or if the
/// content is not valid JSON.
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
            types::TransactionOperation::Install { manifest }
            | types::TransactionOperation::Uninstall { manifest } => {
                for file in manifest.installed_files {
                    files.insert(file);
                }
            }
            types::TransactionOperation::Upgrade {
                old_manifest,
                new_manifest
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

/// Returns a list of all packages modified during a transaction.
///
/// # Errors
///
/// Returns an error if the transaction log file cannot be read or if the
/// content is not valid JSON.
pub fn get_modified_packages(transaction_id: &str) -> Result<Vec<String>> {
    let path = get_transaction_path(transaction_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)?;
    let transaction: types::Transaction = serde_json::from_str(&content)?;

    let mut packages = HashSet::new();
    for op in transaction.operations {
        match op {
            types::TransactionOperation::Install { manifest }
            | types::TransactionOperation::Uninstall { manifest } => {
                packages.insert(manifest.name);
            }
            types::TransactionOperation::Upgrade {
                old_manifest,
                new_manifest
            } => {
                packages.insert(old_manifest.name);
                packages.insert(new_manifest.name);
            }
        }
    }
    Ok(packages.into_iter().collect())
}

/// Deletes a transaction log file.
///
/// # Errors
///
/// Returns an error if the transaction log file cannot be deleted.
pub fn delete_log(transaction_id: &str) -> Result<()> {
    let path = get_transaction_path(transaction_id)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Lists all completed and in-progress transactions.
///
/// # Errors
///
/// Returns an error if the transaction directory cannot be read.
pub fn list_transactions() -> Result<Vec<TransactionMetadata>> {
    let dir = get_transactions_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut transactions = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file()
            || path.extension().and_then(|s| s.to_str()) != Some("json")
        {
            continue;
        }

        // A single unreadable log (e.g. truncated by a crash before writes
        // were atomic) must not hide every other transaction.
        let transaction: types::Transaction = match fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|content| {
                serde_json::from_str(&content).map_err(anyhow::Error::from)
            }) {
            Ok(transaction) => transaction,
            Err(e) => {
                eprintln!(
                    "{} skipping unreadable transaction log {}: {e}",
                    "Warning:".yellow(),
                    path.display()
                );
                continue;
            }
        };
        transactions.push(TransactionMetadata {
            id: transaction.id,
            start_time: transaction.start_time,
            operation_count: transaction.operations.len()
        });
    }

    transactions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    Ok(transactions)
}

/// Checks if a package has installed files outside of the Zoi store.
///
/// Manifest entries are placeholder strings: `${pkgstore}/...` is inside the
/// store, while `${usrroot}`, `${usrhome}`, `${applications}` and absolute
/// paths outside the store directory are not (see
/// [`uninstall::files_outside_store`]).
fn has_files_outside_store(manifest: &types::InstallManifest) -> bool {
    !uninstall::files_outside_store(manifest).is_empty()
}

/// Generates an installation source string for a manifest.
fn install_source_for_manifest(manifest: &types::InstallManifest) -> String {
    local::installed_manifest_source(manifest)
}

/// `name` or `name:sub`, for messages.
fn manifest_label(manifest: &types::InstallManifest) -> String {
    match &manifest.sub_package {
        Some(sub) => format!("{}:{sub}", manifest.name),
        None => manifest.name.clone()
    }
}

/// Removes the dependent links that packages installed by `transaction`
/// recorded on each other.
///
/// Those links describe the state being rolled back. Removing them up front
/// means no package directory that survives the rollback (one holding other
/// versions or sub-packages) keeps a link to a package that no longer
/// exists. Links to packages installed before the transaction are left to
/// each package's own removal, which drops them.
fn unlink_transaction_packages(transaction: &types::Transaction) {
    let installed: Vec<&types::InstallManifest> = transaction
        .operations
        .iter()
        .filter_map(|op| match op {
            types::TransactionOperation::Install { manifest } => {
                Some(manifest.as_ref())
            }
            _ => None
        })
        .collect();
    let ids: Vec<String> =
        installed.iter().map(|m| local::installed_manifest_source(m)).collect();
    for manifest in &installed {
        let Ok(dir) = local::get_package_dir(
            manifest.scope,
            &manifest.registry_handle,
            &manifest.repo,
            &manifest.name
        ) else {
            continue;
        };
        for id in &ids {
            let _ = local::remove_dependent(&dir, id);
        }
    }
}

/// Identity of a package for matching dependencies:
/// `(handle, repo, name, sub-package)`.
type PackageKey = (String, String, String, Option<String>);

/// Returns the identity of an installed manifest.
fn manifest_key(manifest: &types::InstallManifest) -> PackageKey {
    (
        manifest.registry_handle.clone(),
        manifest.repo.clone(),
        manifest.name.clone(),
        manifest.sub_package.clone()
    )
}

/// Parses a recorded `zoi:#handle@repo/name[:sub]@version` dependency into a
/// package identity, ignoring the version.
fn dependency_key(dep: &str) -> Option<PackageKey> {
    let source = dep.strip_prefix("zoi:")?.trim().strip_prefix('#')?;
    let (handle, rest) = source.split_once('@')?;
    let (repo_name, _version) = rest.rsplit_once('@')?;
    let (repo, name_sub) = repo_name.rsplit_once('/')?;
    let (name, sub) = match name_sub.split_once(':') {
        Some((name, sub)) => (name, Some(sub.to_string())),
        None => (name_sub, None)
    };
    Some((handle.into(), repo.into(), name.into(), sub))
}

/// Whether the dependency `dep` names the package `key`. A dependency
/// without a sub-package matches every sub-package of that package.
fn dependency_matches(dep: &PackageKey, key: &PackageKey) -> bool {
    dep.0 == key.0
        && dep.1 == key.1
        && dep.2 == key.2
        && (dep.3.is_none() || dep.3 == key.3)
}

/// Orders a transaction's operations for rollback.
///
/// Operations are undone newest first. When every operation is an install
/// (an `install` transaction, the common case), they are additionally
/// ordered by the dependencies the manifests recorded: a package is removed
/// before the packages it depends on, so its pre-remove hooks still find
/// them. Packages that do not depend on each other, and any dependency
/// cycle, keep newest-first order.
fn rollback_order(
    operations: &[types::TransactionOperation]
) -> Vec<&types::TransactionOperation> {
    let newest_first: Vec<&types::TransactionOperation> =
        operations.iter().rev().collect();
    let installs: Option<Vec<&types::InstallManifest>> = newest_first
        .iter()
        .map(|op| match op {
            types::TransactionOperation::Install { manifest } => {
                Some(manifest.as_ref())
            }
            _ => None
        })
        .collect();
    let Some(installs) = installs else {
        return newest_first;
    };

    let keys: Vec<PackageKey> =
        installs.iter().map(|m| manifest_key(m)).collect();
    // depends_on[i] lists the installs that install i depends on.
    let depends_on: Vec<Vec<usize>> = installs
        .iter()
        .enumerate()
        .map(|(i, manifest)| {
            let deps: Vec<PackageKey> = manifest
                .installed_dependencies
                .iter()
                .filter_map(|d| dependency_key(d))
                .collect();
            keys.iter()
                .enumerate()
                .filter(|(j, key)| {
                    *j != i && deps.iter().any(|d| dependency_matches(d, key))
                })
                .map(|(j, _)| j)
                .collect()
        })
        .collect();

    // Remove a package only once nothing remaining depends on it.
    let mut dependents = vec![0usize; installs.len()];
    for deps in &depends_on {
        for &j in deps {
            if let Some(count) = dependents.get_mut(j) {
                *count += 1;
            }
        }
    }
    let mut done = vec![false; installs.len()];
    let mut order = Vec::with_capacity(installs.len());
    let pending = |done: &[bool], i: usize| done.get(i) == Some(&false);
    while order.len() < installs.len() {
        // The newest package nothing remaining depends on; with a cycle,
        // the newest remaining package.
        let next = (0..installs.len())
            .find(|&i| pending(&done, i) && dependents.get(i) == Some(&0))
            .or_else(|| (0..installs.len()).find(|&i| pending(&done, i)));
        let Some(i) = next else { break };
        if let Some(flag) = done.get_mut(i) {
            *flag = true;
        }
        order.push(i);
        for &j in depends_on.get(i).map_or(&[][..], Vec::as_slice) {
            if let Some(count) = dependents.get_mut(j) {
                *count = count.saturating_sub(1);
            }
        }
    }
    order.into_iter().filter_map(|i| newest_first.get(i).copied()).collect()
}

/// Reverts all operations recorded in a transaction.
///
/// This is the "Atomic Rollback" mechanism. It processes operations in reverse
/// order:
/// - Installs are uninstalled. Dependents never block this (see
///   [`uninstall::rollback_install`]), collections do not cascade into their
///   dependencies, and a package that is already gone counts as rolled back,
///   so an incomplete rollback can simply be run again.
/// - Uninstalls are re-installed (either from local store or registry).
/// - Upgrades are reverted to the previous version.
///
/// # Errors
///
/// Returns an error if the transaction log file cannot be read, if the content
/// is not valid JSON, or if the rollback operation fails.
pub fn rollback(transaction_id: &str) -> Result<()> {
    let path = get_transaction_path(transaction_id)?;
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&path)?;
    let transaction: types::Transaction = serde_json::from_str(&content)?;

    println!("\n{} Starting Rollback...", "::".bold().blue());
    let mut rollback_failed = false;

    unlink_transaction_packages(&transaction);

    for operation in rollback_order(&transaction.operations) {
        match operation {
            types::TransactionOperation::Install { manifest } => {
                println!(
                    "Rolling back installation of {} v{}...",
                    manifest.name.cyan(),
                    manifest.version.yellow()
                );
                if let Err(e) = uninstall::rollback_install(manifest, false) {
                    eprintln!(
                        "{} Failed to rollback install of '{}': {}",
                        "Error:".red().bold(),
                        manifest_label(manifest),
                        e
                    );
                    rollback_failed = true;
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
                    &manifest.version
                ) {
                    Ok(dir) => dir,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to get version directory for rollback: \
                             {}",
                            "Error:".red().bold(),
                            e
                        );
                        rollback_failed = true;
                        continue;
                    }
                };

                let manifest_path = local::find_store_manifest(
                    &version_dir,
                    manifest.sub_package.as_deref()
                );

                if version_dir.exists()
                    && manifest_path.is_some()
                    && !has_files_outside_store(manifest)
                {
                    println!(
                        "Restoring version {} from local store...",
                        manifest.version
                    );
                    // Restores `latest`, manifest, shims, completions,
                    // dependent links, database row, file index and lockfile.
                    if let Err(e) =
                        uninstall::activate_stored_manifest(manifest, false)
                    {
                        eprintln!(
                            "{} Failed to restore '{}' from the store: {}",
                            "Error:".red().bold(),
                            manifest.name,
                            e
                        );
                        rollback_failed = true;
                    }
                    continue;
                }

                println!(
                    "Version not found locally or contains global files. \
                     Re-installing from registry..."
                );

                let source = install_source_for_manifest(manifest);
                let (graph, _) =
                    match install::resolver::resolve_dependency_graph(
                        &[source],
                        Some(manifest.scope),
                        true,
                        true,
                        true,
                        None,
                        true,
                        None
                    ) {
                        Ok(res) => res,
                        Err(e) => {
                            eprintln!(
                                "{} Failed to resolve dependency graph for \
                                 rollback of '{}': {}",
                                "Error:".red().bold(),
                                manifest.name,
                                e
                            );
                            rollback_failed = true;
                            continue;
                        }
                    };

                let install_plan = match install::plan::create_install_plan(
                    &graph.nodes,
                    None,
                    false
                ) {
                    Ok(plan) => plan,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to create install plan for rollback of \
                             '{}': {}",
                            "Error:".red().bold(),
                            manifest.name,
                            e
                        );
                        rollback_failed = true;
                        continue;
                    }
                };

                let stages = match graph.toposort() {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to sort dependency graph for rollback \
                             of '{}': {}",
                            "Error:".red().bold(),
                            manifest.name,
                            e
                        );
                        rollback_failed = true;
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
                                node, action, None, None, true, true, true,
                                false, false
                            )
                        {
                            eprintln!(
                                "{} Failed to re-install during rollback of \
                                 '{}': {}",
                                "Error:".red().bold(),
                                manifest.name,
                                e
                            );
                            rollback_failed = true;
                        }
                    }
                }
            }
            types::TransactionOperation::Upgrade {
                old_manifest,
                new_manifest
            } => {
                println!(
                    "Rolling back upgrade of {} from {} to {}...",
                    old_manifest.name.cyan(),
                    new_manifest.version.yellow(),
                    old_manifest.version.green()
                );
                if let Err(e) =
                    uninstall::rollback_upgrade_removal(new_manifest, false)
                {
                    eprintln!(
                        "{} Failed to uninstall new version during \
                         upgrade-rollback for '{}': {}",
                        "Error:".red().bold(),
                        new_manifest.name,
                        e
                    );
                    rollback_failed = true;
                }

                let version_dir = match local::get_package_version_dir(
                    old_manifest.scope,
                    &old_manifest.registry_handle,
                    &old_manifest.repo,
                    &old_manifest.name,
                    &old_manifest.version
                ) {
                    Ok(dir) => dir,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to get version directory for rollback: \
                             {}",
                            "Error:".red().bold(),
                            e
                        );
                        rollback_failed = true;
                        continue;
                    }
                };

                let manifest_path = local::find_store_manifest(
                    &version_dir,
                    old_manifest.sub_package.as_deref()
                );

                if version_dir.exists()
                    && manifest_path.is_some()
                    && !has_files_outside_store(old_manifest)
                {
                    println!(
                        "Restoring version {} from local store...",
                        old_manifest.version
                    );
                    // Restores `latest`, manifest, shims, completions,
                    // dependent links, database row, file index and lockfile.
                    if let Err(e) =
                        uninstall::activate_stored_manifest(old_manifest, false)
                    {
                        eprintln!(
                            "{} Failed to restore '{}' from the store: {}",
                            "Error:".red().bold(),
                            old_manifest.name,
                            e
                        );
                        rollback_failed = true;
                    }
                    continue;
                }

                println!(
                    "Version not found locally or contains global files. \
                     Re-installing from registry..."
                );

                let source = install_source_for_manifest(old_manifest);
                let (graph, _) =
                    match install::resolver::resolve_dependency_graph(
                        std::slice::from_ref(&source),
                        Some(old_manifest.scope),
                        true,
                        true,
                        true,
                        None,
                        true,
                        None
                    ) {
                        Ok(res) => res,
                        Err(e) => {
                            eprintln!(
                                "{} Failed to resolve dependency graph for \
                                 rollback of '{}': {}",
                                "Error:".red().bold(),
                                old_manifest.name,
                                e
                            );
                            rollback_failed = true;
                            continue;
                        }
                    };

                let install_plan = match install::plan::create_install_plan(
                    &graph.nodes,
                    None,
                    false
                ) {
                    Ok(plan) => plan,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to create install plan for rollback of \
                             '{}': {}",
                            "Error:".red().bold(),
                            old_manifest.name,
                            e
                        );
                        rollback_failed = true;
                        continue;
                    }
                };

                let stages = match graph.toposort() {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "{} Failed to sort dependency graph for rollback \
                             of '{}': {}",
                            "Error:".red().bold(),
                            old_manifest.name,
                            e
                        );
                        rollback_failed = true;
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
                                node, action, None, None, true, true, true,
                                false, false
                            )
                        {
                            eprintln!(
                                "{} Failed to re-install during rollback of \
                                 '{}': {}",
                                "Error:".red().bold(),
                                old_manifest.name,
                                e
                            );
                            rollback_failed = true;
                        }
                    }
                }
            }
        }
    }

    if rollback_failed {
        return Err(anyhow!(
            "Rollback for transaction '{transaction_id}' was incomplete; its \
             log was retained for recovery"
        ));
    }

    println!("{}", ":: Rollback Complete".bold().blue());
    delete_log(transaction_id)?;
    Ok(())
}

/// Returns the ID of the most recently created transaction, if any.
///
/// # Errors
///
/// Returns an error if the transaction directory cannot be read.
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
        if path.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some("json")
        {
            let metadata = fs::metadata(&path)?;
            let modified_time = metadata.modified()?;

            if last_modified_time.is_none_or(|last| modified_time > last) {
                last_modified_time = Some(modified_time);
                last_transaction_id =
                    path.file_stem().and_then(|s| s.to_str()).map(String::from);
            }
        }
    }

    Ok(last_transaction_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An install of `name[:sub]` that depends on `deps`
    /// (`name` or `name:sub`).
    fn install(name: &str, deps: &[&str]) -> types::TransactionOperation {
        let (name, sub) = match name.split_once(':') {
            Some((name, sub)) => (name, Some(sub.to_string())),
            None => (name, None)
        };
        types::TransactionOperation::Install {
            manifest: Box::new(types::InstallManifest {
                name: name.to_string(),
                version: "1.0.0".to_string(),
                epoch: 0,
                revision: "1".to_string(),
                sub_package: sub,
                repo: "core".to_string(),
                repo_type: "official".to_string(),
                registry_handle: "zoidberg".to_string(),
                package_type: types::PackageType::Package,
                description: String::new(),
                reason: types::InstallReason::Direct,
                scope: types::Scope::System,
                bins: None,
                alternatives: None,
                conflicts: None,
                replaces: None,
                provides: None,
                backup: None,
                installed_dependencies: deps
                    .iter()
                    .map(|d| format!("zoi:#zoidberg@core/{d}@1.0.0"))
                    .collect(),
                dependencies_v2: None,
                chosen_options: vec![],
                chosen_optionals: vec![],
                install_method: None,
                platform: String::new(),
                service: None,
                installed_files: vec![],
                file_digests: None,
                installed_size: None,
                sandbox: None,
                completions: None
            })
        }
    }

    /// The rollback order as `name[:sub]` labels.
    fn order(ops: &[types::TransactionOperation]) -> Vec<String> {
        rollback_order(ops)
            .into_iter()
            .filter_map(|op| match op {
                types::TransactionOperation::Install { manifest } => {
                    Some(manifest_label(manifest))
                }
                _ => None
            })
            .collect()
    }

    #[test]
    fn dependents_are_removed_before_their_dependencies() {
        // Recorded out of dependency order, as parallel installs can be.
        let ops = [
            install("app", &["lib"]),
            install("lib", &["glibc"]),
            install("glibc", &[]),
            install("tool", &["glibc"])
        ];
        let order = order(&ops);
        let pos = |name: &str| {
            order.iter().position(|n| n == name).expect("in order")
        };
        assert!(pos("app") < pos("lib"), "{order:?}");
        assert!(pos("lib") < pos("glibc"), "{order:?}");
        assert!(pos("tool") < pos("glibc"), "{order:?}");
    }

    #[test]
    fn sub_packages_and_cycles_are_handled() {
        let ops = [
            install("gcc:libgcc", &[]),
            install("gcc:libstdc++", &["gcc:libgcc"]),
            install("a", &["b"]),
            install("b", &["a"])
        ];
        let order = order(&ops);
        assert_eq!(order.len(), 4, "{order:?}");
        let pos = |name: &str| {
            order.iter().position(|n| n == name).expect("in order")
        };
        assert!(pos("gcc:libstdc++") < pos("gcc:libgcc"), "{order:?}");
    }
}
