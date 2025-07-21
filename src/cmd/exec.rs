use crate::pkg;
use colored::*;

pub fn run(source: String, args: Vec<String>) {
    if let Err(e) = pkg::exec::run(&source, args) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}
