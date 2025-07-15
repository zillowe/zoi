use crate::pkg;
use colored::*;

pub fn run(package_name: &str) {
    println!(
        "{}{}{}",
        "--- Uninstalling package '".yellow(),
        package_name.blue().bold(),
        "' ---".yellow()
    );

    if let Err(e) = pkg::uninstall::run(package_name) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }

    println!("\n{}", "Uninstallation complete.".green());
}
