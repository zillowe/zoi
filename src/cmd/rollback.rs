use crate::pkg::{self, transaction};
use crate::utils;
use anyhow::{Result, anyhow};

pub fn run(
    package_name: &str,
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
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
        return Err(anyhow!("Package '{}' is not installed.", package_name));
    }
    let chosen =
        crate::cmd::installed_select::choose_installed_manifest(package_name, &candidates, yes)?;

    plugin_manager.trigger_hook("on_rollback", None)?;
    pkg::rollback::run(&pkg::local::installed_manifest_source(&chosen), yes)
}

pub fn run_transaction_rollback(
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    if !utils::ask_for_confirmation(
        "This will roll back the last recorded transaction. Are you sure?",
        yes,
    ) {
        println!("Operation aborted.");
        return Ok(());
    }

    match transaction::get_last_transaction_id()? {
        Some(id) => {
            println!("Rolling back transaction {}...", id);
            plugin_manager.trigger_hook("on_rollback", None)?;
            transaction::rollback(&id)
        }
        None => {
            println!("No transactions found to roll back.");
            Ok(())
        }
    }
}
