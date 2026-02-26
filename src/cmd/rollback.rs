use crate::pkg::{self, transaction};
use crate::utils;
use anyhow::Result;

pub fn run(
    package_name: &str,
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    plugin_manager.trigger_hook("on_rollback", None)?;
    pkg::rollback::run(package_name, yes)
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
