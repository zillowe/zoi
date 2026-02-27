use crate::pkg::{cache, local, resolve, types};
use anyhow::{Result, anyhow};
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use semver::Version;
use std::collections::HashSet;
use std::fs;

pub fn run(
    package_name: &str,
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    println!(
        "--- Downgrading package '{}' ---",
        package_name.blue().bold()
    );

    let _request = resolve::parse_source_string(package_name)?;
    let (pkg, current_version, _, _, registry_handle) =
        resolve::resolve_package_and_version(package_name, true)?;

    let mut versions = HashSet::new();

    let handle = registry_handle.as_deref().unwrap_or("local");
    if let Ok(package_dir) =
        local::get_package_dir(types::Scope::User, handle, &pkg.repo, &pkg.name)
        && let Ok(entries) = fs::read_dir(package_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Some(v_str) = path.file_name().and_then(|s| s.to_str())
                && v_str != "latest"
                && v_str != "dependents"
                && Version::parse(v_str).is_ok()
            {
                versions.insert(v_str.to_string());
            }
        }
    }

    if let Ok(archive_cache) = cache::get_archive_cache_root()
        && let Ok(entries) = fs::read_dir(archive_cache)
    {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.starts_with(&pkg.name) && filename.ends_with(".pkg.tar.zst") {
                let parts: Vec<&str> = filename.split('-').collect();
                if parts.len() >= 2 {
                    let v_str = parts[1];
                    if Version::parse(v_str).is_ok() {
                        versions.insert(v_str.to_string());
                    }
                }
            }
        }
    }

    if let Some(versions_map) = &pkg.versions {
        for channel in versions_map.keys() {
            if channel != "stable" {
                versions.insert(format!("@{}", channel));
            }
        }
    }

    if versions.is_empty() {
        return Err(anyhow!(
            "No other versions found for '{}' in local store or cache.",
            pkg.name
        ));
    }

    let mut sorted_versions: Vec<String> = versions.into_iter().collect();
    sorted_versions.sort_by(|a, b| {
        let va = Version::parse(a.trim_start_matches('@'));
        let vb = Version::parse(b.trim_start_matches('@'));
        match (va, vb) {
            (Ok(a), Ok(b)) => b.cmp(&a),
            _ => b.cmp(a),
        }
    });

    println!("Currently installed: {}", current_version.yellow());

    if yes {
        return Err(anyhow!(
            "Downgrade requires interactive version selection. Use 'zoi install {}@<version>' for non-interactive scripts.",
            pkg.name
        ));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select version to install")
        .items(&sorted_versions)
        .default(0)
        .interact_opt()?
        .ok_or(anyhow!("No version selected."))?;

    let selected_version = &sorted_versions[selection];

    if selected_version == &current_version {
        println!("Version {} is already installed.", selected_version);
        return Ok(());
    }

    let install_source = format!(
        "{}@{}",
        package_name.split('@').next().unwrap(),
        selected_version
    );

    println!(
        "
Initiating install for {}...",
        install_source.cyan()
    );

    crate::cmd::install::run(
        &[install_source],
        None,
        true,
        false,
        yes,
        Some(crate::cli::InstallScope::User),
        false,
        false,
        false,
        None,
        plugin_manager,
    )
}
