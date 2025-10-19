use crate::pkg::{self, transaction, types};
use colored::*;

pub fn run(
    package_names: &[String],
    scope: Option<crate::cli::InstallScope>,
    local: bool,
    global: bool,
) {
    let mut scope_override = scope.map(|s| match s {
        crate::cli::InstallScope::User => types::Scope::User,
        crate::cli::InstallScope::System => types::Scope::System,
        crate::cli::InstallScope::Project => types::Scope::Project,
    });

    if local {
        scope_override = Some(types::Scope::Project);
    } else if global {
        scope_override = Some(types::Scope::User);
    }

    let transaction = match transaction::begin() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to begin transaction: {}", e);
            std::process::exit(1);
        }
    };

    let mut failed_packages = Vec::new();

    for name in package_names {
        println!("--- Uninstalling package '{}' ---", name.blue().bold(),);

        match pkg::uninstall::run(name, scope_override) {
            Ok(uninstalled_manifest) => {
                if let Err(e) = transaction::record_operation(
                    &transaction.id,
                    types::TransactionOperation::Uninstall {
                        manifest: Box::new(uninstalled_manifest),
                    },
                ) {
                    eprintln!("Failed to record transaction operation for {}: {}", name, e);
                    failed_packages.push(name.clone());
                } else {
                    println!("\n{} Uninstallation complete.", "Success:".green());
                }
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                failed_packages.push(name.clone());
            }
        }
    }

    if !failed_packages.is_empty() {
        eprintln!("\nError: Uninstallation failed for some packages.");
        eprintln!("\n{} Rolling back changes...", "---".yellow().bold());
        if let Err(e) = transaction::rollback(&transaction.id) {
            eprintln!("\nCRITICAL: Rollback failed: {}", e);
            eprintln!(
                "The system may be in an inconsistent state. The transaction log is at ~/.zoi/transactions/{}.json",
                transaction.id
            );
        } else {
            println!("\n{} Rollback successful.", "Success:".green().bold());
        }
        std::process::exit(1);
    } else if let Err(e) = transaction::commit(&transaction.id) {
        eprintln!("Warning: Failed to commit transaction: {}", e);
    }
}
