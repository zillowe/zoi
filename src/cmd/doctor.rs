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
            println!("Please run 'zoi setup' to add Zoi's binary directory to your PATH.");
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
