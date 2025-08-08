use crate::pkg::{install, resolve, types};
use crate::utils;
use colored::*;
use std::collections::HashSet;

pub fn run(sources: &[String], force: bool, interactive: bool, yes: bool) {
    let mode = if interactive {
        install::InstallMode::Interactive
    } else {
        install::InstallMode::PreferBinary
    };

    let mut failed_packages = Vec::new();
    let mut processed_deps = HashSet::new();

    for source in sources {
        println!("=> Installing package: {}", source.cyan().bold());

        match resolve::resolve_source(source) {
            Ok(resolved_source) => {
                if let Err(e) = utils::confirm_untrusted_source(&resolved_source.source_type, yes) {
                    eprintln!("\n{}", e.to_string().red());
                    failed_packages.push(source.to_string());
                    continue;
                }

                utils::print_repo_warning(&resolved_source.repo_name);

                if let Err(e) = install::run_installation(
                    resolved_source.path.to_str().unwrap(),
                    mode.clone(),
                    force,
                    types::InstallReason::Direct,
                    yes,
                    &mut processed_deps,
                ) {
                    if e.to_string().contains("aborted by user") {
                        eprintln!("\n{}", e.to_string().yellow());
                    } else {
                        eprintln!(
                            "{}: Failed to install '{}': {}",
                            "Error".red().bold(),
                            source,
                            e
                        );
                        eprintln!(
                            "{} telemetry not sent due to install failure",
                            "Info:".yellow()
                        );
                    }
                    failed_packages.push(source.to_string());
                }
            }
            Err(e) => {
                eprintln!("{}: {}", "Error".red().bold(), e);
                failed_packages.push(source.to_string());
            }
        }
    }

    if !failed_packages.is_empty() {
        eprintln!(
            "\n{}: The following packages failed to install:",
            "Error".red().bold()
        );
        for pkg in &failed_packages {
            eprintln!("  - {}", pkg);
        }
        std::process::exit(1);
    }
}
