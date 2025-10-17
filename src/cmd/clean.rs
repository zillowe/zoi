use crate::pkg;
use colored::*;

pub fn run() {
    println!("{}", "--- Cleaning Cache ---".yellow().bold());
    if let Err(e) = pkg::cache::clear() {
        eprintln!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
    if let Err(e) = pkg::cache::clear_archives() {
        eprintln!("{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
    println!("{}", "Cache cleaned successfully.".green());
}
