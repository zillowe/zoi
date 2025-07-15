use crate::pkg::{install, resolve};
use crate::utils;
use colored::*;

pub fn run(source: &str, force: bool) {
    println!(
        "{}{}{}",
        "--- Installing package '".yellow(),
        source.blue().bold(),
        "' ---".yellow()
    );

    match resolve::resolve_source(source) {
        Ok(resolved_source) => {
            if let Err(e) = utils::confirm_untrusted_source(&resolved_source.source_type) {
                eprintln!("\n{}", e.to_string().red());
                return;
            }

            if let Err(e) = install::run_installation(
                &resolved_source.path,
                install::InstallMode::PreferBinary,
                force,
            ) {
                eprintln!("\n{}: {}", "Installation failed".red().bold(), e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            std::process::exit(1);
        }
    }

    println!("\n{}", "Installation complete.".green());
}
