use crate::pkg::{clone, resolve};
use crate::utils;
use colored::*;

pub fn run(source: String, target_dir: Option<String>) {
    println!(
        "{}{}{}",
        "--- Cloning package source '".yellow(),
        source.blue().bold(),
        "' ---".yellow()
    );

    match resolve::resolve_source(&source) {
        Ok(resolved_source) => {
            if let Err(e) = utils::confirm_untrusted_source(&resolved_source.source_type) {
                eprintln!("\n{}", e.to_string().red());
                return;
            }

            if let Err(e) = clone::run(&resolved_source.path, target_dir.as_deref()) {
                eprintln!("\n{}: {}", "Error".red().bold(), e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            std::process::exit(1);
        }
    }

    println!("\n{}", "Clone complete.".green());
}
