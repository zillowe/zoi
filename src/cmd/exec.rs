use crate::pkg;
use colored::*;

pub fn run(source: String, args: Vec<String>, upstream: bool, cache: bool, local: bool) {
    if let Err(e) = pkg::exec::run(&source, args, upstream, cache, local) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}
