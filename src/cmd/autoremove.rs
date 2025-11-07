use crate::pkg;
use colored::*;

pub fn run(yes: bool) {
    println!("{}", "--- Autoremoving Unused Packages ---".yellow());

    if let Err(e) = pkg::autoremove::run(yes) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }

    println!("\n{}", "Cleanup complete.".green());
}
