use crate::pkg::{install, local, pin, resolve, types};
use crate::utils;
use semver::Version;
use std::collections::HashSet;
use std::error::Error;
use std::fs;

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateResult {
    Updated { from: String, to: String },
    AlreadyUpToDate,
    Pinned,
}

pub fn run(package_name: &str, yes: bool) -> Result<UpdateResult, Box<dyn Error>> {
    let (new_pkg, new_version, _, _, registry_handle) =
        match resolve::resolve_package_and_version(package_name) {
            Ok(result) => result,
            Err(e) => {
                return Err(format!("Could not resolve package '{}': {}", package_name, e).into());
            }
        };

    if pin::is_pinned(package_name)? {
        return Ok(UpdateResult::Pinned);
    }

    let manifest = match local::is_package_installed(&new_pkg.name, types::Scope::User)?.or(
        local::is_package_installed(&new_pkg.name, types::Scope::System)?,
    ) {
        Some(m) => m,
        None => {
            return Err(format!(
                "Package '{}' is not installed. Use 'zoi install' instead.",
                package_name
            )
            .into());
        }
    };

    if manifest.version == new_version {
        return Ok(UpdateResult::AlreadyUpToDate);
    }

    if !utils::ask_for_confirmation(
        &format!("Update from {} to {}?", manifest.version, new_version),
        yes,
    ) {
        return Err("Update aborted by user.".into());
    }

    let mode = if let Some(updater_method) = &new_pkg.updater {
        install::InstallMode::Updater(updater_method.clone())
    } else {
        install::InstallMode::PreferBinary
    };

    let mut processed_deps = HashSet::new();
    install::run_installation(
        package_name,
        mode,
        true,
        types::InstallReason::Direct,
        yes,
        false,
        &mut processed_deps,
        None,
    )?;

    let handle = registry_handle.as_deref().unwrap_or("local");
    let package_dir = local::get_package_dir(manifest.scope, handle, &new_pkg.repo, &new_pkg.name)?;

    let mut versions = Vec::new();
    if let Ok(entries) = fs::read_dir(&package_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Some(version_str) = path.file_name().and_then(|s| s.to_str())
                && version_str != "latest"
                && let Ok(version) = Version::parse(version_str.trim_start_matches('v'))
            {
                versions.push(version);
            }
        }
    }

    versions.sort();

    if versions.len() > 2 {
        let versions_to_delete = &versions[..versions.len() - 2];
        for version in versions_to_delete {
            let version_dir = package_dir.join(format!("v{}", version));
            if version_dir.exists() {
                fs::remove_dir_all(version_dir)?;
            }
        }
    }

    Ok(UpdateResult::Updated {
        from: manifest.version,
        to: new_version,
    })
}
