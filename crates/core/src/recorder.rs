use crate::types;
use anyhow::{Result, anyhow};
use std::fs;
use std::path::PathBuf;

fn get_lockfile_path(scope: types::Scope) -> Result<PathBuf> {
    let path = if scope == types::Scope::Project {
        std::env::current_dir()?.join("zoi.lock")
    } else {
        let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
        crate::sysroot::apply_sysroot(home_dir.join(".zoi").join("pkgs").join("zoi.lock"))
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

fn read_lockfile(scope: types::Scope) -> Result<types::ZoiLockV2> {
    let path = get_lockfile_path(scope)?;
    if !path.exists() || fs::read_to_string(&path)?.trim().is_empty() {
        return Ok(types::ZoiLockV2 {
            version: "2".to_string(),
            ..Default::default()
        });
    }
    let content = fs::read_to_string(path)?;
    let lockfile = serde_json::from_str(&content)?;
    Ok(lockfile)
}

fn write_lockfile(lockfile: &types::ZoiLockV2, scope: types::Scope) -> Result<()> {
    let path = get_lockfile_path(scope)?;
    let content = serde_json::to_string_pretty(lockfile)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn record_package(
    pkg: &types::Package,
    reason: &types::InstallReason,
    _installed_dependencies: &[String],
    registry_handle: &str,
    _chosen_options: &[String],
    _chosen_optionals: &[String],
    sub_package: Option<String>,
) -> Result<()> {
    let mut lockfile = read_lockfile(pkg.scope)?;

    let package_key = if let Some(sub) = &sub_package {
        format!("@{}/{}:{}", pkg.repo, pkg.name, sub)
    } else {
        format!("@{}/{}", pkg.repo, pkg.name)
    };

    let os = std::env::consts::OS;
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    let platform = format!("{}-{}", os, arch);

    let detail = types::LockPackageDetailV2 {
        name: pkg.name.clone(),
        sub_package: sub_package.clone(),
        repo: pkg.repo.clone(),
        repo_type: "official".to_string(),
        version: pkg.version.clone().unwrap_or_default(),
        revision: pkg.revision.clone(),
        registry: registry_handle.to_string(),
        why: match reason {
            types::InstallReason::Direct => "direct".to_string(),
            types::InstallReason::Dependency { .. } => "dependency".to_string(),
        },
        description: pkg.description.clone(),
        package_type_install: "pre-compiled".to_string(),
        install_method: "pre-built".to_string(),
        installed_sub_packages: sub_package.clone().map(|s| vec![s]).unwrap_or_default(),
        platform,
        hash: "".to_string(),
        dependencies: pkg.dependencies.clone().map(types::to_dependencies_v2),
    };

    lockfile.installed_packages.insert(package_key, detail);
    lockfile.version = "2".to_string();

    write_lockfile(&lockfile, pkg.scope)
}

pub fn update_package_reason(
    manifest: &types::InstallManifest,
    new_reason: types::InstallReason,
) -> Result<()> {
    let mut lockfile = read_lockfile(manifest.scope)?;
    let package_key = if let Some(sub) = &manifest.sub_package {
        format!("@{}/{}:{}", manifest.repo, manifest.name, sub)
    } else {
        format!("@{}/{}", manifest.repo, manifest.name)
    };

    if let Some(pkg) = lockfile.installed_packages.get_mut(&package_key) {
        pkg.why = match new_reason {
            types::InstallReason::Direct => "direct".to_string(),
            types::InstallReason::Dependency { .. } => "dependency".to_string(),
        };
        lockfile.version = "2".to_string();
        write_lockfile(&lockfile, manifest.scope)?;
        Ok(())
    } else {
        Err(anyhow!("Package '{}' not found in record.", manifest.name))
    }
}

pub fn remove_package_from_record(manifest: &types::InstallManifest) -> Result<()> {
    let mut lockfile = read_lockfile(manifest.scope)?;
    let package_key = if let Some(sub) = &manifest.sub_package {
        format!("@{}/{}:{}", manifest.repo, manifest.name, sub)
    } else {
        format!("@{}/{}", manifest.repo, manifest.name)
    };

    if lockfile.installed_packages.remove(&package_key).is_some() {
        lockfile.version = "2".to_string();
        write_lockfile(&lockfile, manifest.scope)?;
    }

    Ok(())
}

pub fn get_recorded_packages() -> Result<Vec<types::LockPackageDetailV2>> {
    let mut all_packages = Vec::new();
    for scope in [
        types::Scope::User,
        types::Scope::System,
        types::Scope::Project,
    ] {
        if let Ok(lockfile) = read_lockfile(scope) {
            all_packages.extend(lockfile.installed_packages.into_values());
        }
    }
    Ok(all_packages)
}
