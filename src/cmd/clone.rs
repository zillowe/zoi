use crate::pkg::{clone, resolve};
use crate::utils;
use colored::*;

pub fn run(source: String, target_dir: Option<String>, yes: bool) {
    println!(
        "{}{}{}",
        "--- Cloning package source '".yellow(),
        source.blue().bold(),
        "' ---".yellow()
    );

    match resolve::resolve_source(&source) {
        Ok(resolved_source) => {
            if let Err(e) = utils::confirm_untrusted_source(&resolved_source.source_type, yes) {
                eprintln!("\n{}", e.to_string().red());
                return;
            }

            utils::print_repo_warning(&resolved_source.repo_name);

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
