use crate::pkg::{install, resolve, types};
use crate::utils;
use colored::*;
use std::collections::HashSet;
use std::fs;

pub fn run(sources: &[String], force: bool, interactive: bool, yes: bool) {
    let mode = if interactive {
        install::InstallMode::Interactive
    } else {
        install::InstallMode::PreferBinary
    };

    let mut failed_packages = Vec::new();
    let mut processed_deps = HashSet::new();

    let mut sources_to_process: Vec<String> = Vec::new();
    for source in sources {
        if source.ends_with("zoi.pkgs.json") {
            println!(
                "=> Installing packages from lockfile: {}",
                source.cyan().bold()
            );
            match fs::read_to_string(source) {
                Ok(content) => {
                    match serde_json::from_str::<Vec<types::RecordedPackage>>(&content) {
                        Ok(packages) => {
                            for pkg in packages {
                                sources_to_process
                                    .push(format!("@{}/{}@{}", pkg.repo, pkg.name, pkg.version));
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "{}: Failed to parse lockfile '{}': {}",
                                "Error".red().bold(),
                                source,
                                e
                            );
                            failed_packages.push(source.to_string());
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{}: Failed to read lockfile '{}': {}",
                        "Error".red().bold(),
                        source,
                        e
                    );
                    failed_packages.push(source.to_string());
                }
            }
        } else {
            sources_to_process.push(source.to_string());
        }
    }

    for source in &sources_to_process {
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
