use crate::pkg;
use anyhow::Result;
use colored::*;

pub fn run() -> Result<()> {
    println!("{}", "--- Running Zoi Doctor ---".yellow().bold());
    println!("Checking your system for potential issues...");

    let mut issues_found = 0;

    println!("\n{}", "Checking for broken symlinks...".bold());
    match pkg::doctor::check_broken_symlinks() {
        Ok(broken_links) => {
            if broken_links.is_empty() {
                println!("{}", "No broken symlinks found.".green());
            } else {
                issues_found += broken_links.len();
                println!(
                    "{}: Found {} broken symlinks:",
                    "Warning".yellow(),
                    broken_links.len()
                );
                for link in broken_links {
                    println!("  - {}", link.display());
                }
                println!(
                    "\nConsider running 'zoi uninstall <package>' and reinstalling it for the affected packages."
                );
            }
        }
        Err(e) => {
            eprintln!(
                "{}: Failed to check for broken symlinks: {}",
                "Error".red(),
                e
            );
            issues_found += 1;
        }
    }

    println!("\n{}", "Checking PATH configuration...".bold());
    match pkg::doctor::check_path_configuration() {
        Ok(Some(warning)) => {
            issues_found += 1;
            println!("{}: {}", "Warning".yellow(), warning);
            println!("Please run 'zoi shell <shell>' to add Zoi's binary directory to your PATH.");
        }
        Ok(None) => {
            println!("{}", "PATH configuration looks good.".green());
        }
        Err(e) => {
            eprintln!(
                "{}: Failed to check PATH configuration: {}",
                "Error".red(),
                e
            );
            issues_found += 1;
        }
    }

    println!("\n{}", "Checking for outdated repositories...".bold());
    match pkg::doctor::check_outdated_repos() {
        Ok(Some(warning)) => {
            issues_found += 1;
            println!("{}: {}", "Warning".yellow(), warning);
            println!("Consider running 'zoi sync' to update your local package database.");
        }
        Ok(None) => {
            println!("{}", "Repositories look up to date.".green());
        }
        Err(e) => {
            eprintln!("{}: Failed to check repositories: {}", "Error".red(), e);
            issues_found += 1;
        }
    }

    println!("\n{}", "Checking for duplicate package IDs...".bold());
    match pkg::doctor::check_duplicate_packages() {
        Ok(duplicates) => {
            if duplicates.is_empty() {
                println!("{}", "No duplicate package IDs found.".green());
            } else {
                issues_found += duplicates.len();
                println!(
                    "{}: Found {} duplicate package IDs across registries:",
                    "Warning".yellow(),
                    duplicates.len()
                );
                for (pkg_id, registries) in duplicates {
                    println!(
                        "  - {} (found in: {})",
                        pkg_id.cyan(),
                        registries.join(", ")
                    );
                }
                println!(
                    "\nThis may cause ambiguity during installation. Consider specifying the registry handle (e.g. #registry@repo/name)."
                );
            }
        }
        Err(e) => {
            eprintln!("{}: Failed to check for duplicates: {}", "Error".red(), e);
            issues_found += 1;
        }
    }

    println!("\n{}", "Checking PGP configurations...".bold());
    match pkg::doctor::check_pgp_configuration() {
        Ok(missing_keys) => {
            if missing_keys.is_empty() {
                println!("{}", "PGP configuration looks valid.".green());
            } else {
                issues_found += missing_keys.len();
                println!(
                    "{}: The following trusted PGP keys are missing from your keyring:",
                    "Warning".yellow()
                );
                for key in missing_keys {
                    println!("  - {}", key.red());
                }
                println!("\nRun 'zoi pgp add --name <name> --url <url>' to add missing keys.");
            }
        }
        Err(e) => {
            eprintln!(
                "{}: Failed to check PGP configuration: {}",
                "Error".red(),
                e
            );
            issues_found += 1;
        }
    }

    println!("\n{}", "Validating zoi.pkgs.json integrity...".bold());
    match pkg::doctor::validate_pkgs_json_integrity() {
        Ok(missing_packages) => {
            if missing_packages.is_empty() {
                println!("{}", "zoi.pkgs.json integrity is good.".green());
            } else {
                issues_found += missing_packages.len();
                println!(
                    "{}: The following packages are recorded but missing from the store:",
                    "Warning".yellow()
                );
                for pkg in missing_packages {
                    println!("  - {}", pkg.red());
                }
                println!(
                    "\nYour package record file is out of sync with the actual installation store."
                );
            }
        }
        Err(e) => {
            eprintln!("{}: Failed to validate zoi.pkgs.json: {}", "Error".red(), e);
            issues_found += 1;
        }
    }

    println!("\n{}", "Checking for orphaned packages...".bold());
    match pkg::doctor::check_orphaned_packages() {
        Ok(orphaned) => {
            if orphaned.is_empty() {
                println!("{}", "No orphaned packages found.".green());
            } else {
                issues_found += orphaned.len();
                println!(
                    "{}: Found {} orphaned packages (unused dependencies):",
                    "Warning".yellow(),
                    orphaned.len()
                );
                for pkg in orphaned {
                    println!("  - {}", pkg.cyan());
                }
                println!("\nConsider running 'zoi autoremove' to clean up these packages.");
            }
        }
        Err(e) => {
            eprintln!(
                "{}: Failed to check for orphaned packages: {}",
                "Error".red(),
                e
            );
            issues_found += 1;
        }
    }

    if issues_found == 0 {
        println!(
            "\n{}",
            "Zoi is looking healthy! No issues found.".green().bold()
        );
    } else {
        println!("\nFound {} potential issues.", issues_found);
    }

    Ok(())
}
