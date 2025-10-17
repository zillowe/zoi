use crate::pkg::{self};
use colored::*;

pub fn run(package_names: &[String]) {
    for name in package_names {
        println!(
            "{}{}{}",
            "--- Uninstalling package '".yellow(),
            name.blue().bold(),
            "' ---".yellow()
        );

        if let Err(e) = pkg::uninstall::run(name) {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            continue;
        }

        println!("\n{}", "Uninstallation complete.".green());
    }
}
