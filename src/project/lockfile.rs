use crate::pkg::types;
use anyhow::{Result, anyhow};
use std::collections::HashSet;
use std::fs;

fn get_lockfile_path() -> Result<std::path::PathBuf> {
    Ok(std::env::current_dir()?.join("zoi.lock"))
}

pub fn read_zoi_lock() -> Result<types::ZoiLock> {
    let path = get_lockfile_path()?;
    if !path.exists() {
        return Ok(types::ZoiLock {
            version: "1".to_string(),
            ..Default::default()
        });
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(types::ZoiLock {
            version: "1".to_string(),
            ..Default::default()
        });
    }

    serde_json::from_str(&content).map_err(|e| {
        anyhow!(
            "Failed to parse zoi.lock. It might be corrupted or in an old format. Error: {}",
            e
        )
    })
}

pub fn write_zoi_lock(lockfile: &types::ZoiLock) -> Result<()> {
    let path = get_lockfile_path()?;
    let content = serde_json::to_string_pretty(lockfile)?;
    fs::write(path, content)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenLockPackage {
    pub source: String,
    pub direct: bool,
    pub chosen_options: Vec<String>,
    pub chosen_optionals: Vec<String>,
    pub dependencies: Vec<String>,
    pub git_sha: Option<String>,
}

fn append_sub_package_if_needed(base_id: &str, sub_package: Option<&str>) -> String {
    if base_id.contains(':') {
        base_id.to_string()
    } else if let Some(sub_package) = sub_package {
        format!("{}:{}", base_id, sub_package)
    } else {
        base_id.to_string()
    }
}

pub fn locked_packages(lockfile: &types::ZoiLock) -> Vec<FrozenLockPackage> {
    let direct_ids: HashSet<String> = lockfile.packages.keys().cloned().collect();
    let mut packages = Vec::new();

    for (reg_key, pkgs) in &lockfile.details {
        for (short_id, detail) in pkgs {
            let base_id = append_sub_package_if_needed(
                &format!("{}{}", reg_key, short_id),
                detail.sub_package.as_deref(),
            );
            packages.push(FrozenLockPackage {
                source: format!("{}@{}", base_id, detail.version),
                direct: direct_ids.is_empty() || direct_ids.contains(&base_id),
                chosen_options: detail.options_dependencies.clone(),
                chosen_optionals: detail.optionals_dependencies.clone(),
                dependencies: detail.dependencies.clone(),
                git_sha: detail.git_sha.clone(),
            });
        }
    }

    if packages.is_empty() {
        packages.extend(
            lockfile
                .packages
                .iter()
                .map(|(full_id, version)| FrozenLockPackage {
                    source: format!("{}@{}", full_id, version),
                    direct: true,
                    chosen_options: Vec::new(),
                    chosen_optionals: Vec::new(),
                    dependencies: Vec::new(),
                    git_sha: None,
                }),
        );
    }

    packages.sort_by(|a, b| a.source.cmp(&b.source));
    packages.dedup_by(|a, b| a.source == b.source);
    packages
}

pub fn sources_from_lock(lockfile: &types::ZoiLock) -> Vec<String> {
    locked_packages(lockfile)
        .into_iter()
        .map(|entry| entry.source)
        .collect()
}
