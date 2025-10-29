use crate::pkg::{self, transaction, types};
use colored::*;

pub fn run(
    package_names: &[String],
    scope: Option<crate::cli::InstallScope>,
    local: bool,
    global: bool,
    save: bool,
    yes: bool,
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

    if save && scope_override != Some(types::Scope::Project) {
        eprintln!(
            "{}: The --save flag can only be used with project-scoped uninstalls.",
            "Error".red().bold()
        );
        std::process::exit(1);
    }

    let installed_packages = match pkg::local::get_installed_packages() {
        Ok(pkgs) => pkgs,
        Err(e) => {
            eprintln!("Error reading installed packages: {}", e);
            std::process::exit(1);
        }
    };
    let mut final_package_names = Vec::new();

    for name in package_names {
        if let Ok(request) = pkg::resolve::parse_source_string(name)
            && request.sub_package.is_none()
        {
            let mut subs_to_uninstall = Vec::new();
            let mut is_split = false;
            for manifest in &installed_packages {
                if manifest.name == request.name {
                    is_split = true;
                    if let Some(sub) = &manifest.sub_package {
                        subs_to_uninstall.push(format!("{}:{}", manifest.name, sub));
                    }
                }
            }

            if is_split && !subs_to_uninstall.is_empty() {
                println!(
                    "'{}' is a split package. Queueing all installed sub-packages for uninstallation: {}",
                    name,
                    subs_to_uninstall.join(", ")
                );
                final_package_names.extend(subs_to_uninstall);
                continue;
            }
        }
        final_package_names.push(name.clone());
    }
    final_package_names.sort();
    final_package_names.dedup();

    let mut total_size_freed_bytes: u64 = 0;
    for name in &final_package_names {
        if let Some(manifest) = installed_packages.iter().find(|m| {
            let manifest_name = if let Some(sub) = &m.sub_package {
                format!("{}:{}", m.name, sub)
            } else {
                m.name.clone()
            };
            &manifest_name == name
        }) {
            total_size_freed_bytes += manifest.installed_size.unwrap_or(0);
        }
    }

    println!("Packages to remove:");
    for name in &final_package_names {
        println!("  - {}", name);
    }

    println!(
        "\nTotal size to be freed: {}",
        crate::utils::format_bytes(total_size_freed_bytes)
    );

    if !crate::utils::ask_for_confirmation(":: Proceed with removal?", yes) {
        return;
    }

    let transaction = match transaction::begin() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to begin transaction: {}", e);
            std::process::exit(1);
        }
    };

    let mut failed_packages = Vec::new();
    let mut successfully_uninstalled = Vec::new();

    for name in &final_package_names {
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
                    successfully_uninstalled.push(name.clone());
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

    if save
        && let Err(e) =
            crate::project::config::remove_packages_from_config(&successfully_uninstalled)
    {
        eprintln!(
            "{}: Failed to remove packages from zoi.yaml: {}",
            "Warning".yellow().bold(),
            e
        );
    }
}
