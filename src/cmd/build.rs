use crate::pkg::{install, resolve};
use crate::utils;
use colored::*;

pub fn run(source: &str) {
    println!(
        "{}{}{}",
        "--- Building package '".yellow(),
        source.blue().bold(),
        "' from source ---".yellow()
    );

    match resolve::resolve_source(source) {
        Ok(resolved_source) => {
            if let Err(e) = utils::confirm_untrusted_source(&resolved_source.source_type) {
                eprintln!("\n{}", e.to_string().red());
                return;
            }

            if let Err(e) = install::run_installation(
                &resolved_source.path,
                install::InstallMode::ForceSource,
                true,
            ) {
                eprintln!("\n{}: {}", "Build failed".red().bold(), e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            std::process::exit(1);
        }
    }

    println!("\n{}", "Build complete.".green());
}
