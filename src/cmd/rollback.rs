use crate::pkg::{self, transaction};
use crate::utils;
use anyhow::Result;

pub fn run(package_name: &str, yes: bool) -> Result<()> {
    pkg::rollback::run(package_name, yes)
}

pub fn run_transaction_rollback(yes: bool) -> Result<()> {
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
            transaction::rollback(&id)
        }
        None => {
            println!("No transactions found to roll back.");
            Ok(())
        }
    }
}
