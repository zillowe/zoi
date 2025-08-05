use crate::pkg;
use colored::*;

pub fn run(package_names: &[String]) {
    for name in package_names {
        let package_name = name.trim();
        println!(
            "{}{} {}",
            "--- Uninstalling package '".yellow(),
            package_name.blue().bold(),
            "' ---".yellow()
        );

        if let Err(e) = pkg::uninstall::run(package_name) {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            continue;
        }

        println!("\n{}", "Uninstallation complete.".green());
    }
}
