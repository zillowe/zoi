use crate::pkg;
use anyhow::Result;
use colored::*;

pub fn run() -> Result<()> {
    println!("{} Running Zoi doctor...", "::".bold().blue());
    println!("Checking your system for potential issues...");

    let mut issues_found = 0;

    println!("\n{} Checking for broken symlinks...", "->".bold().cyan());
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

    println!("\n{} Checking PATH configuration...", "->".bold().cyan());
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

    println!(
        "\n{} Checking for outdated repositories...",
        "->".bold().cyan()
    );
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

    println!(
        "\n{} Checking for duplicate package IDs...",
        "->".bold().cyan()
    );
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

    println!("\n{} Checking PGP configurations...", "->".bold().cyan());
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

    println!(
        "\n{} Validating zoi.pkgs.json integrity...",
        "->".bold().cyan()
    );
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

    println!("\n{} Checking for orphaned packages...", "->".bold().cyan());
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

    println!("\n{} Checking for ghost dependents...", "->".bold().cyan());
    match pkg::doctor::check_ghost_dependents() {
        Ok(ghost_links) => {
            if ghost_links.is_empty() {
                println!("{}", "No ghost dependents found.".green());
            } else {
                issues_found += ghost_links.len();
                println!(
                    "{}: Found {} broken dependent links (ghost parents):",
                    "Warning".yellow(),
                    ghost_links.len()
                );
                for (_, parent_id) in &ghost_links {
                    println!("  - parent missing: {}", parent_id.cyan());
                }

                if crate::utils::ask_for_confirmation(
                    "\nDo you want to prune these broken links?",
                    false,
                ) {
                    pkg::doctor::prune_ghost_dependents(&ghost_links)?;
                    println!("{}", "Successfully pruned broken links.".green());
                    issues_found -= ghost_links.len();
                } else {
                    println!("Broken links were NOT pruned.");
                }
            }
        }
        Err(e) => {
            eprintln!(
                "{}: Failed to check for ghost dependents: {}",
                "Error".red(),
                e
            );
            issues_found += 1;
        }
    }

    println!("\n{} Checking for external tools...", "->".bold().cyan());
    let tool_results = pkg::doctor::check_external_tools();
    if tool_results.essential_missing.is_empty() && tool_results.recommended_missing.is_empty() {
        println!(
            "{}",
            "All essential and recommended tools are installed.".green()
        );
    } else {
        if !tool_results.essential_missing.is_empty() {
            issues_found += tool_results.essential_missing.len();
            println!("{}: Essential tools are missing:", "Error".red().bold());
            for tool in tool_results.essential_missing {
                println!("  - {}", tool.red());
            }
            println!(
                "Please install these tools as they are required for Zoi to function correctly."
            );
        }
        if !tool_results.recommended_missing.is_empty() {
            println!("{}: Recommended tools are missing:", "Note".yellow().bold());
            for tool in tool_results.recommended_missing {
                println!("  - {}", tool.yellow());
            }
            println!("Zoi will work without these, but some features may be limited.");
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
