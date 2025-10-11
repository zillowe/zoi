use crate::pkg::{install, resolve, types};
use crate::utils;
use colored::Colorize;
use std::collections::HashSet;

pub fn run(
    sources: &[String],
    repo: Option<String>,
    force: bool,
    all_optional: bool,
    yes: bool,
    scope: Option<crate::cli::SetupScope>,
) {
    if let Some(repo_spec) = repo {
        if let Err(e) = crate::pkg::repo_install::run(&repo_spec, force, all_optional, yes, scope) {
            eprintln!(
                "{}: Failed to install from repo '{}': {}",
                "Error".red().bold(),
                repo_spec,
                e
            );
            std::process::exit(1);
        }
        return;
    }
    let mode = install::InstallMode::PreferPrebuilt;

    let scope_override = scope.map(|s| match s {
        crate::cli::SetupScope::User => types::Scope::User,
        crate::cli::SetupScope::System => types::Scope::System,
    });

    let mut failed_packages = Vec::new();
    let mut processed_deps = HashSet::new();

    let mut temp_files = Vec::new();
    let mut sources_to_process: Vec<String> = Vec::new();

    for source in sources {
        if source.ends_with("zoi.pkgs.json") {
            if let Err(e) = install::lockfile::process_lockfile(
                source,
                &mut sources_to_process,
                &mut temp_files,
            ) {
                eprintln!(
                    "{}: Failed to process lockfile '{}': {}",
                    "Error".red().bold(),
                    source,
                    e
                );
                failed_packages.push(source.to_string());
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

                if let Some(repo_name) = &resolved_source.repo_name {
                    utils::print_repo_warning(repo_name);
                }

                if let Err(e) = install::run_installation(
                    source,
                    mode.clone(),
                    force,
                    types::InstallReason::Direct,
                    yes,
                    all_optional,
                    &mut processed_deps,
                    scope_override,
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
