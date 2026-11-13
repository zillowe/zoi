//! Rollback logic for Zoi transactions.

use std::cmp::Ordering;
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use colored::Colorize;
use zoi_core::{types, utils as core_utils};
use zoi_resolver::{local, resolve};
use zoi_uninstall as uninstall;

/// Rolls back a package to its previous version.
///
/// The previous version is the highest version that is still kept in the
/// store, has a valid install manifest for the same package (and
/// sub-package), and is lower than the active version. The active version is
/// removed through the regular uninstall path and the previous one is
/// re-activated from the store: `latest`, shims, completions, dependent
/// links, database row, file index and lockfile all move to it.
///
/// Nothing is changed when the previous version cannot be restored from the
/// store alone (for example because it installed files under `${usrroot}`
/// or `${usrhome}`, which the store does not keep).
///
/// # Errors
///
/// Returns an error if the package is not installed or ambiguous, no older
/// stored version exists, the previous version cannot be restored from the
/// store, or the uninstall/activation fails.
pub fn run(package_name: &str, yes: bool) -> Result<()> {
    println!("Attempting to roll back '{}'...", package_name.cyan());

    let request = resolve::parse_source_string(package_name)?;

    let scope_order = [
        types::Scope::User,
        types::Scope::System,
        types::Scope::Project
    ];
    let mut current_manifest = None;
    for candidate_scope in scope_order {
        let mut matches = local::find_installed_manifests_matching(
            &request,
            candidate_scope
        )?;
        match matches.len() {
            0 => {}
            1 => {
                current_manifest = Some(matches.remove(0));
                break;
            }
            _ => {
                return Err(anyhow!(
                    "Ambiguous package name '{package_name}' matches multiple \
                     installed packages.",
                ));
            }
        }
    }

    let Some(current_manifest) = current_manifest else {
        return Err(anyhow!("Package '{package_name}' is not installed."));
    };

    let package_dir = local::get_package_dir(
        current_manifest.scope,
        &current_manifest.registry_handle,
        &current_manifest.repo,
        &current_manifest.name
    )?;

    let Some((previous_version, _previous_manifest)) =
        find_previous_version(&package_dir, &current_manifest)
    else {
        return Err(anyhow!(
            "No previous version older than {} found in the store for package \
             '{}'.",
            current_manifest.version,
            current_manifest.name
        ));
    };

    println!(
        "Rolling back from version {} to {}...",
        current_manifest.version.yellow(),
        previous_version.green()
    );

    if !yes
        && !core_utils::ask_for_confirmation("Do you want to proceed?", false)
    {
        return Err(anyhow!("Rollback aborted by user."));
    }

    uninstall::rollback_to_stored_version(
        &current_manifest,
        &previous_version,
        yes,
        false
    )?;

    println!(
        "{} Successfully rolled back {} to version {}.",
        "::".bold().green(),
        current_manifest.name.cyan(),
        previous_version.green()
    );

    Ok(())
}

/// Compares two version strings, using semver when both parse and plain
/// string comparison otherwise.
fn compare_versions(a: &str, b: &str) -> Ordering {
    match (
        semver::Version::parse(a.trim_start_matches('v')),
        semver::Version::parse(b.trim_start_matches('v'))
    ) {
        (Ok(va), Ok(vb)) => va.cmp(&vb),
        _ => a.cmp(b)
    }
}

/// Finds the version to roll back to: the highest stored version that is
/// lower than the active one.
///
/// Only real version directories are considered: entries that are not
/// directories (such as `*.zoisave` backups), hidden directories (such as
/// leftover `.tmp-install-*` staging directories), and directories without a
/// readable install manifest for the same package, sub-package, repository
/// and registry are skipped. The manifest's version must match its directory
/// name.
fn find_previous_version(
    package_dir: &Path,
    current: &types::InstallManifest
) -> Option<(String, types::InstallManifest)> {
    let entries = fs::read_dir(package_dir).ok()?;
    let mut candidates: Vec<(String, types::InstallManifest)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "latest" || name == "dependents" || name.starts_with('.') {
            continue;
        }
        let version_dir = entry.path();
        // `latest` is a symlink; real version directories are not.
        if version_dir.is_symlink() || !version_dir.is_dir() {
            continue;
        }
        let Some(manifest_path) = local::find_store_manifest(
            &version_dir,
            current.sub_package.as_deref()
        ) else {
            continue;
        };
        let Ok(manifest) = local::read_store_manifest(&manifest_path) else {
            continue;
        };
        if manifest.version != name
            || !manifest.name.eq_ignore_ascii_case(&current.name)
            || manifest.sub_package != current.sub_package
            || !manifest.repo.eq_ignore_ascii_case(&current.repo)
            || !manifest
                .registry_handle
                .eq_ignore_ascii_case(&current.registry_handle)
        {
            continue;
        }
        if compare_versions(&name, &current.version) != Ordering::Less {
            continue;
        }
        candidates.push((name, manifest));
    }
    candidates
        .into_iter()
        .max_by(|a, b| compare_versions(&a.0, &b.0))
}
