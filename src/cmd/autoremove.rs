use crate::pkg;
use colored::*;

pub fn run() {
    println!("{}", "\n--- Autoremoving Unused Packages ---".yellow());

    if let Err(e) = pkg::autoremove::run() {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }

    println!("\n{}", "Cleanup complete.".green());
}
