use crate::pkg::{clone, resolve};
use crate::utils;
use colored::*;

pub fn run(sources: Vec<String>, target_dir: Option<String>, yes: bool) {
    if sources.len() > 1 && target_dir.is_some() {
        eprintln!(
            "{}: The target directory can only be specified when cloning a single source.",
            "Error".red().bold()
        );
        std::process::exit(1);
    }

    for source in sources {
        println!(
            "{}{} {}",
            "--- Cloning package source '".yellow(),
            source.blue().bold(),
            "' ---".yellow()
        );

        match resolve::resolve_source(&source) {
            Ok(resolved_source) => {
                if let Err(e) = utils::confirm_untrusted_source(&resolved_source.source_type, yes) {
                    eprintln!("\n{}", e.to_string().red());
                    continue;
                }

                utils::print_repo_warning(&resolved_source.repo_name);

                if let Err(e) = clone::run(&resolved_source.path, target_dir.as_deref()) {
                    eprintln!("\n{}: {}", "Error".red().bold(), e);
                }
            }
            Err(e) => {
                eprintln!("\n{}: {}", "Error".red().bold(), e);
            }
        }
        println!("\n{}", "Clone complete.".green());
    }
}
