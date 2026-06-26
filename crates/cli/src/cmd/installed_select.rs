use crate::pkg::{db, local, types};
use anyhow::{Result, anyhow};
use colored::*;
use comfy_table::{Table, presets::UTF8_FULL};
use dialoguer::{Select, theme::ColorfulTheme};
use std::collections::HashMap;

#[derive(Clone)]
struct CandidateDisplay {
    manifest: types::InstallManifest,
    description: String,
}

fn scope_label(scope: types::Scope) -> &'static str {
    match scope {
        types::Scope::User => "user",
        types::Scope::System => "system",
        types::Scope::Project => "project",
    }
}

fn lookup_descriptions() -> HashMap<(String, String, Option<String>, &'static str, String), String>
{
    let mut descriptions = HashMap::new();
    if let Ok(packages) = db::list_all_packages("local") {
        for pkg in packages {
            let key = (
                pkg.name.clone(),
                pkg.repo.clone(),
                pkg.sub_package.clone(),
                scope_label(pkg.scope),
                pkg.registry_handle.unwrap_or_else(|| "local".to_string()),
            );
            descriptions.insert(key, pkg.description);
        }
    }
    descriptions
}

fn build_candidate_displays(candidates: &[types::InstallManifest]) -> Vec<CandidateDisplay> {
    let descriptions = lookup_descriptions();
    candidates
        .iter()
        .cloned()
        .map(|manifest| {
            let description = descriptions
                .get(&(
                    manifest.name.clone(),
                    manifest.repo.clone(),
                    manifest.sub_package.clone(),
                    scope_label(manifest.scope),
                    manifest.registry_handle.clone(),
                ))
                .cloned()
                .or_else(|| {
                    let source_path = local::get_package_source_path(&manifest).ok()?;
                    let path = source_path.to_str()?;
                    let pkg = crate::pkg::lua::parser::parse_lua_package(
                        path,
                        Some(&manifest.version),
                        true,
                    )
                    .ok()?;
                    Some(pkg.description)
                })
                .unwrap_or_else(|| "Description unavailable".to_string());

            CandidateDisplay {
                manifest,
                description,
            }
        })
        .collect()
}

pub fn choose_installed_manifest(
    package_name: &str,
    candidates: &[types::InstallManifest],
    yes: bool,
) -> Result<types::InstallManifest> {
    if candidates.is_empty() {
        return Err(anyhow!("Package '{}' is not installed.", package_name));
    }
    if candidates.len() == 1 {
        return Ok(candidates[0].clone());
    }
    if yes {
        return Err(anyhow!(
            "Package '{}' matches multiple installed packages. Use an explicit source like '#handle@repo/name[:sub]@version'.",
            package_name
        ));
    }

    let displays = build_candidate_displays(candidates);

    println!(
        "Found multiple installed packages matching '{}'. Please choose one:",
        package_name.cyan()
    );

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["#", "Scope", "Source", "Version", "Description"]);

    for (i, display) in displays.iter().enumerate() {
        table.add_row(vec![
            (i + 1).to_string(),
            scope_label(display.manifest.scope).to_string(),
            local::installed_manifest_source(&display.manifest),
            display.manifest.version.clone(),
            display.description.clone(),
        ]);
    }
    println!("{table}");

    let items: Vec<String> = displays
        .iter()
        .map(|display| {
            format!(
                "{} ({}, v{})",
                local::installed_manifest_source(&display.manifest),
                scope_label(display.manifest.scope),
                display.manifest.version
            )
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select an installed package")
        .items(&items)
        .default(0)
        .interact()?;

    Ok(displays[selection].manifest.clone())
}
