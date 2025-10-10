use crate::pkg::{install, resolve, types};
use crate::utils;
use colored::*;
use cyclonedx_bom::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

const ZOI_REPO_PROPERTY: &str = "zoi:repo";
const ZOI_SCOPE_PROPERTY: &str = "zoi:scope";
const ZOI_CHOSEN_OPTIONS_PROPERTY: &str = "zoi:chosen_options";
const ZOI_CHOSEN_OPTIONALS_PROPERTY: &str = "zoi:chosen_optionals";

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
            println!(
                "=> Installing packages from SBOM lockfile: {}",
                source.cyan().bold()
            );
            let result: Result<(), Box<dyn std::error::Error>> = (|| {
                let content = fs::read_to_string(source)?;
                let bom = Bom::parse_from_json(content.as_bytes())?;
                if let Some(components) = bom.components {
                    for component in components.0 {
                        let get_prop = |key: &str| {
                            component
                                .properties
                                .as_ref()
                                .and_then(|p| p.0.iter().find(|prop| prop.name == key))
                                .map(|prop| prop.value.clone())
                        };

                        let repo = get_prop(ZOI_REPO_PROPERTY).unwrap_or_else(|| "unknown".into());
                        let name = component.name.to_string();
                        let version = component.version.ok_or("Missing version")?.to_string();

                        let scope_str =
                            get_prop(ZOI_SCOPE_PROPERTY).unwrap_or_else(|| "user".into());
                        let scope = if AsRef::<str>::as_ref(&scope_str) == "system" {
                            types::Scope::System
                        } else {
                            types::Scope::User
                        };

                        let chosen_options = get_prop(ZOI_CHOSEN_OPTIONS_PROPERTY)
                            .map(|s| s.split(',').map(String::from).collect())
                            .unwrap_or_default();
                        let chosen_optionals = get_prop(ZOI_CHOSEN_OPTIONALS_PROPERTY)
                            .map(|s| s.split(',').map(String::from).collect())
                            .unwrap_or_default();

                        let manifest = types::SharableInstallManifest {
                            name,
                            version,
                            repo: repo.to_string(),
                            scope,
                            chosen_options,
                            chosen_optionals,
                        };

                        let mut temp_file = NamedTempFile::new()?;
                        let yaml_content = serde_yaml::to_string(&manifest)?;
                        temp_file.write_all(yaml_content.as_bytes())?;

                        sources_to_process.push(temp_file.path().to_str().unwrap().to_string());
                        temp_files.push(temp_file);
                    }
                }
                Ok(())
            })();

            if let Err(e) = result {
                eprintln!(
                    "{}: Failed to process SBOM lockfile '{}': {}",
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
