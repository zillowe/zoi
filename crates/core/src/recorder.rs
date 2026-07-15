use crate::types;
use anyhow::{Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

static RECORD_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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

/// Persists the state of the Zoi environment into the lockfile (`zoi.lock`).
///
/// Specification v2 uses a "Snapshot" model for reproducibility. Instead of just
/// recording versions, Zoi computes:
/// - `packages_hash`: A recursive SHA-512 hash of the entire package store.
/// - `registries_hash`: A recursive SHA-512 hash of the metadata database.
/// - Per-Package Hash: A hash of the specific version directory.
///
/// This ensures that a project environment can be verified for 100% bit-for-bit
/// identicality across different machines.
fn write_lockfile(lockfile: &mut types::ZoiLockV2, scope: types::Scope) -> Result<()> {
    if crate::frozen::is_frozen() {
        return Ok(());
    }
    let path = get_lockfile_path(scope)?;

    if let Ok(store_dir) = crate::utils::get_store_base_dir(scope) {
        lockfile.packages_hash = Some(format!(
            "sha512-{}",
            crate::hash::calculate_dir_hash(&store_dir).unwrap_or_default()
        ));
    }

    let db_dir = if scope == types::Scope::Project {
        std::env::current_dir()?
            .join(".zoi")
            .join("pkgs")
            .join("db")
    } else {
        crate::utils::get_db_root().unwrap_or_default()
    };

    if db_dir.exists() {
        lockfile.registries_hash = Some(format!(
            "sha512-{}",
            crate::hash::calculate_dir_hash(&db_dir).unwrap_or_default()
        ));
    }

    let content = serde_json::to_string_pretty(lockfile)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn record_package(
    pkg: &types::Package,
    reason: &types::InstallReason,
    _installed_dependencies: &[String],
    registry_handle: &str,
    repo_type: &str,
    _chosen_options: &[String],
    _chosen_optionals: &[String],
    sub_package: Option<String>,
) -> Result<()> {
    let _lock = RECORD_MUTEX
        .lock()
        .map_err(|e| anyhow!("Mutex poisoned: {}", e))?;
    let mut lockfile = read_lockfile(pkg.scope)?;

    let package_key = if let Some(sub) = &sub_package {
        format!("@{}/{}:{}", pkg.repo.trim(), pkg.name.trim(), sub.trim())
    } else {
        format!("@{}/{}", pkg.repo.trim(), pkg.name.trim())
    };

    let os = std::env::consts::OS;
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    let platform = format!("{}-{}", os, arch);

    let hash = compute_package_hash(pkg, registry_handle);

    let detail = types::LockPackageDetailV2 {
        name: pkg.name.clone(),
        sub_package: sub_package.clone(),
        repo: pkg.repo.clone(),
        repo_type: repo_type.to_string(),
        version: pkg.version.clone().unwrap_or_default(),
        revision: pkg.revision.clone(),
        registry: registry_handle.to_string(),
        why: match reason {
            types::InstallReason::Direct => "direct".to_string(),
            types::InstallReason::Dependency { .. } => "dependency".to_string(),
        },
        description: pkg.description.clone(),
        package_type_install: format!("{:?}", pkg.package_type).to_lowercase(),
        install_method: if pkg.types.contains(&"source".to_string())
            && !pkg.types.contains(&"pre-compiled".to_string())
        {
            "source".to_string()
        } else {
            "pre-compiled".to_string()
        },
        installed_sub_packages: sub_package.clone().map(|s| vec![s]).unwrap_or_default(),
        platform,
        hash,
        dependencies: pkg.dependencies.clone().map(types::to_dependencies_v2),
    };

    lockfile.installed_packages.insert(package_key, detail);
    lockfile.version = "2".to_string();

    if !lockfile.registries.contains_key(registry_handle)
        && let Some(reg_info) = resolve_registry_info(registry_handle)
    {
        lockfile
            .registries
            .insert(registry_handle.to_string(), reg_info);
    }

    write_lockfile(&mut lockfile, pkg.scope)
}

/// Calculates the current SHA-512 directory hash for an installed package version.
///
/// This is used to verify that the files in the store haven't been modified
/// since they were originally staged.
fn compute_package_hash(pkg: &types::Package, registry_handle: &str) -> String {
    let Some(version) = &pkg.version else {
        return String::new();
    };
    let Ok(store_base) = crate::utils::get_store_base_dir(pkg.scope) else {
        return String::new();
    };
    let package_id = crate::utils::generate_package_id(registry_handle, &pkg.repo, &pkg.name);
    let package_dir_name = crate::utils::get_package_dir_name(&package_id, &pkg.name);
    let version_dir = store_base.join(&package_dir_name).join(version);
    if version_dir.exists() {
        format!(
            "sha512-{}",
            crate::hash::calculate_dir_hash(&version_dir).unwrap_or_default()
        )
    } else {
        String::new()
    }
}

fn resolve_registry_info(registry_handle: &str) -> Option<types::LockRegistryV2> {
    let Ok(config) = crate::config::read_config() else {
        return None;
    };
    let reg = config
        .default_registry
        .as_ref()
        .filter(|r| r.handle == registry_handle)
        .or_else(|| {
            config
                .added_registries
                .iter()
                .find(|r| r.handle == registry_handle)
        })?;

    let db_root = crate::utils::get_db_root().ok()?;
    let reg_path = db_root.join(registry_handle);
    let revision = resolve_git_head(&reg_path).unwrap_or_else(|| "unknown".to_string());

    Some(types::LockRegistryV2 {
        url: reg.url.clone(),
        revision,
    })
}

fn resolve_git_head(repo_path: &Path) -> Option<String> {
    let head_file = repo_path.join(".git").join("HEAD");
    let content = fs::read_to_string(&head_file).ok()?;
    let content = content.trim();
    if let Some(ref_path) = content.strip_prefix("ref: ") {
        let ref_file = repo_path.join(".git").join(ref_path);
        fs::read_to_string(&ref_file)
            .ok()
            .map(|s| s.trim().to_string())
    } else {
        Some(content.to_string())
    }
}

pub fn update_package_reason(
    manifest: &types::InstallManifest,
    new_reason: types::InstallReason,
) -> Result<()> {
    let _lock = RECORD_MUTEX
        .lock()
        .map_err(|e| anyhow!("Mutex poisoned: {}", e))?;
    let mut lockfile = read_lockfile(manifest.scope)?;
    let repo = manifest.repo.trim();
    let name = manifest.name.trim();

    let package_key = if let Some(sub) = &manifest.sub_package {
        format!("@{}/{}:{}", repo, name, sub.trim())
    } else {
        format!("@{}/{}", repo, name)
    };

    if let Some(pkg) = lockfile.installed_packages.get_mut(&package_key) {
        pkg.why = match new_reason {
            types::InstallReason::Direct => "direct".to_string(),
            types::InstallReason::Dependency { .. } => "dependency".to_string(),
        };
        lockfile.version = "2".to_string();
        write_lockfile(&mut lockfile, manifest.scope)?;
        Ok(())
    } else {
        Err(anyhow!("Package '{}' not found in record.", manifest.name))
    }
}

pub fn remove_package_from_record(manifest: &types::InstallManifest) -> Result<()> {
    let _lock = RECORD_MUTEX
        .lock()
        .map_err(|e| anyhow!("Mutex poisoned: {}", e))?;
    let mut lockfile = read_lockfile(manifest.scope)?;
    let repo = manifest.repo.trim();
    let name = manifest.name.trim();

    let package_key = if let Some(sub) = &manifest.sub_package {
        format!("@{}/{}:{}", repo, name, sub.trim())
    } else {
        format!("@{}/{}", repo, name)
    };

    if lockfile.installed_packages.remove(&package_key).is_some() {
        lockfile.version = "2".to_string();
        write_lockfile(&mut lockfile, manifest.scope)?;
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
