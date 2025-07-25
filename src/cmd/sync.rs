use crate::pkg;
use colored::*;

pub fn run(verbose: bool) {
    println!("{}", "--- Syncing Package Database ---".yellow().bold());

    if let Err(e) = pkg::sync::run(verbose) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }

    println!("{}", "Sync complete.".green());
}
