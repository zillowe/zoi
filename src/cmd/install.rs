use crate::pkg::{install, resolve, types::InstallReason};
use crate::utils;
use colored::*;

pub fn run(source: &str, force: bool, interactive: bool, yes: bool) {
    println!(
        "{}{}{}",
        "--- Installing package '".yellow(),
        source.blue().bold(),
        "' ---".yellow()
    );

    match resolve::resolve_source(source) {
        Ok(resolved_source) => {
            if let Err(e) = utils::confirm_untrusted_source(&resolved_source.source_type, yes) {
                eprintln!("\n{}", e.to_string().red());
                return;
            }

            utils::print_repo_warning(&resolved_source.repo_name);

            let mode = if interactive {
                install::InstallMode::Interactive
            } else {
                install::InstallMode::PreferBinary
            };

            if let Err(e) = install::run_installation(
                &resolved_source.path,
                mode,
                force,
                InstallReason::Direct,
                yes,
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
