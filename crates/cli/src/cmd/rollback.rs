//! Rolling back packages and transactions.

use crate::pkg::{self, transaction};
use crate::utils;
use anyhow::{Result, anyhow};

/// Rolls back a specific package to its previous state.
///
/// This looks for an installed package matching the name and triggers a rollback operation.
///
/// # Errors
///
/// Returns an error if the package is not found or the rollback operation fails.
pub fn run(
    package_name: &str,
    yes: bool,
    plugin_manager: Option<&crate::pkg::plugin::PluginManager>,
) -> Result<()> {
    let request = pkg::resolve::parse_source_string(package_name)?;
    let mut candidates = Vec::new();
    for scope in [
        pkg::types::Scope::User,
        pkg::types::Scope::System,
        pkg::types::Scope::Project,
    ] {
        candidates.extend(pkg::local::find_installed_manifests_matching(
            &request, scope,
        )?);
    }
    if candidates.is_empty() {
        return Err(anyhow!("Package '{package_name}' is not installed."));
    }
    let chosen =
        crate::cmd::installed_select::choose_installed_manifest(package_name, &candidates, yes)?;

    if let Some(pm) = plugin_manager {
        pm.set_context(chosen.scope)?;
        pm.trigger_hook("on_rollback", None)?;
    }
    pkg::rollback::run(&pkg::local::installed_manifest_source(&chosen), yes)
}

/// Rolls back the most recent transaction.
///
/// This reverts all changes made in the last recorded transaction.
///
/// # Errors
///
/// Returns an error if the transaction rollback fails.
pub fn run_transaction_rollback(
    yes: bool,
    plugin_manager: Option<&crate::pkg::plugin::PluginManager>,
) -> Result<()> {
    if !utils::ask_for_confirmation(
        "This will roll back the last recorded transaction. Are you sure?",
        yes,
    ) {
        println!("Operation aborted.");
        return Ok(());
    }

    if let Some(id) = transaction::get_last_transaction_id()? {
        println!("Rolling back transaction {id}...");
        if let Some(pm) = plugin_manager {
            pm.trigger_hook("on_rollback", None)?;
        }
        transaction::rollback(&id)
    } else {
        println!("No transactions found to roll back.");
        Ok(())
    }
}
