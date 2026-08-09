//! Rollback logic for Zoi transactions.

use std::fs;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use colored::Colorize;
use zoi_core::{types, utils as core_utils};
use zoi_resolver::{local, resolve};

/// Rolls back a package to its previous version.
/// # Errors
///
/// Returns an error if the rollback fails.
/// # Panics
///
/// Panics if the scope cannot be resolved.
pub fn run(package_name: &str, yes: bool,) -> Result<(),> {
    println!("Attempting to roll back '{}'...", package_name.cyan());

    let request = resolve::parse_source_string(package_name,)?;
    let sub_package = request.sub_package.clone();

    let scope_order = [
        types::Scope::User,
        types::Scope::System,
        types::Scope::Project,
    ];
    let mut current_manifest = None;
    let mut scope = None;
    for candidate_scope in scope_order {
        let mut matches = local::find_installed_manifests_matching(
            &request,
            candidate_scope,
        )?;
        match matches.len() {
            0 => {}
            1 => {
                current_manifest = Some(matches.remove(0,),);
                scope = Some(candidate_scope,);
                break;
            }
            _ => {
                return Err(anyhow!(
                    "Ambiguous package name '{package_name}' matches multiple \
                     installed packages.",
                ),);
            }
        }
    }

    let Some(current_manifest,) = current_manifest else {
        return Err(anyhow!("Package '{package_name}' is not installed."),);
    };
    let scope = scope.expect("Scope should be resolved if manifest is found",);

    let package_dir = local::get_package_dir(
        scope,
        &current_manifest.registry_handle,
        &current_manifest.repo,
        &current_manifest.name,
    )?;

    let current_version = current_manifest.version.clone();
    let manifest_filename = if let Some(sub,) = &sub_package {
        format!("manifest-{sub}.yaml")
    } else {
        "manifest.yaml".to_string()
    };

    let mut versions: Vec<String,> = Vec::new();
    if let Ok(entries,) = fs::read_dir(&package_dir,) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "latest"
                && name != "dependents"
                && name != current_version
            {
                versions.push(name,);
            }
        }
    }

    if versions.is_empty() {
        return Err(anyhow!(
            "No previous versions found for package '{}'.",
            current_manifest.name
        ),);
    }

    versions.sort_by(|a, b| {
        let va = semver::Version::parse(a,)
            .unwrap_or_else(|_| semver::Version::new(0, 0, 0,),);
        let vb = semver::Version::parse(b,)
            .unwrap_or_else(|_| semver::Version::new(0, 0, 0,),);
        va.cmp(&vb,)
    },);
    let previous_version =
        versions.last().expect("Versions list is not empty",);

    println!(
        "Rolling back from version {} to {}...",
        current_version.yellow(),
        previous_version.green()
    );

    let prev_manifest_path = package_dir
        .join(previous_version,)
        .join(&manifest_filename,);
    if !prev_manifest_path.exists() {
        return Err(anyhow!(
            "Previous manifest not found at: {}",
            prev_manifest_path.display()
        ),);
    }

    let prev_manifest_content = fs::read_to_string(&prev_manifest_path,)?;
    let prev_manifest: types::InstallManifest =
        serde_yaml::from_str(&prev_manifest_content,)?;

    if !yes
        && !core_utils::ask_for_confirmation("Do you want to proceed?", false,)
    {
        return Err(anyhow!("Rollback aborted by user."),);
    }

    for file_path_str in &prev_manifest.installed_files {
        let file_path = std::path::Path::new(file_path_str,);
        create_shim(file_path,);
    }

    if let Some(completions,) = &prev_manifest.completions {
        for completion in completions {
            let store_path = package_dir
                .join(previous_version,)
                .join("data/shell",)
                .join(&completion.shell,)
                .join(&completion.filename,);
            let completions_root =
                super::get_completions_root(scope, &completion.shell,)?;
            let pkg_dir = completions_root.join(&prev_manifest.name,);
            let link_path = pkg_dir.join(&completion.filename,);
            if store_path.exists() {
                let _ =
                    super::create_completion_symlink(&store_path, &link_path,);
            }
        }
    }

    let current_version_dir = package_dir.join(&current_version,);
    let mut has_other_manifests = false;
    if let Ok(entries,) = fs::read_dir(&current_version_dir,) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("manifest",)
                && std::path::Path::new(&name,)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml",),)
                && name != manifest_filename
            {
                has_other_manifests = true;
                break;
            }
        }
    }

    if !has_other_manifests {
        for file_path_str in &current_manifest.installed_files {
            let file_path = std::path::Path::new(file_path_str,);
            if file_path.exists() {
                if file_path.is_dir() {
                    let _ = fs::remove_dir_all(file_path,);
                } else {
                    let _ = fs::remove_file(file_path,);
                }
            }
        }
    }

    let current_manifest_path = current_version_dir.join(&manifest_filename,);
    if current_manifest_path.exists() {
        fs::remove_file(current_manifest_path,)?;
    }

    if !has_other_manifests && current_version_dir.exists() {
        let _ = fs::remove_dir_all(current_version_dir,);
    }

    let latest_link = package_dir.join("latest",);
    if latest_link.exists() || latest_link.is_symlink() {
        let _ = fs::remove_file(&latest_link,);
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(previous_version, latest_link,)?;
    #[cfg(windows)]
    let _ = std::os::windows::fs::symlink_dir(previous_version, latest_link,);

    println!(
        "{} Successfully rolled back {} to version {}.",
        "::".bold().green(),
        current_manifest.name.cyan(),
        previous_version.green()
    );

    Ok((),)
}

/// Searches for a binary in the previous manifest to determine if a shim is
/// needed.
#[allow(dead_code)]
fn find_binary_in_prev_manifest(_path: &std::path::Path,) -> Option<PathBuf,> {
    None
}

/// Ensures a shim or binary entry exists at the given path.
fn create_shim(path: &std::path::Path,) {
    let _ = zoi_install::shim::create_shim(path,);
}
