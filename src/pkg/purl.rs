use crate::pkg::mini_resolve;
use crate::utils;
use anyhow::{Result, anyhow};
use purl::GenericPurl;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

fn default_version() -> String {
    "1".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CentralDbSpec {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(flatten)]
    pub registries: HashMap<String, RegistryInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistryInfo {
    pub name: String,
    pub description: String,
    pub git: String,
    pub branch: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PurlPackageIndex {
    pub repo: String,
    pub repo_type: String,
    pub version: String,
    pub description: String,
    pub dependencies: Option<Vec<String>>,
    pub sub_packages: Option<serde_json::Value>,
    pub vuln: Option<Vec<mini_resolve::MiniVulnerability>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistryIndex {
    #[serde(default = "default_version")]
    pub version: String,
    pub packages: HashMap<String, PurlPackageIndex>,
}

pub fn fetch_central_db() -> Result<HashMap<String, RegistryInfo>> {
    let url = std::env::var("ZOI_PURL_DB_URL")
        .unwrap_or_else(|_| "https://zillowe.pages.dev/zoi/registries.json".to_string());
    let client = utils::get_http_client()?;
    let response = client.get(&url).send()?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch central Zoi registry database: {}",
            response.status()
        ));
    }

    let spec: CentralDbSpec = response.json()?;
    Ok(spec.registries)
}

pub fn construct_raw_url(git_url: &str, branch: &str, file_path: &str) -> Result<String> {
    let url = git_url.trim_end_matches(".git").trim_end_matches('/');

    if let Some(path) = url.strip_prefix("https://github.com/") {
        Ok(format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            path, branch, file_path
        ))
    } else if let Some(path) = url.strip_prefix("https://gitlab.com/") {
        Ok(format!(
            "https://gitlab.com/{}/-/raw/{}/{}",
            path, branch, file_path
        ))
    } else if let Some(path) = url.strip_prefix("https://codeberg.org/") {
        Ok(format!(
            "https://codeberg.org/{}/raw/branch/{}/{}",
            path, branch, file_path
        ))
    } else {
        Err(anyhow!(
            "Unsupported git provider for PURL resolution: {}",
            git_url
        ))
    }
}

pub fn fetch_registry_index(registry: &RegistryInfo) -> Result<RegistryIndex> {
    let url = construct_raw_url(&registry.git, &registry.branch, "packages.json")?;
    let client = utils::get_http_client()?;
    let response = client.get(url).send()?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch packages.json from registry {}: {}",
            registry.name,
            response.status()
        ));
    }

    Ok(response.json()?)
}

pub fn fetch_package_lua(registry: &RegistryInfo, repo: &str, name: &str) -> Result<String> {
    let file_path = if repo.is_empty() {
        format!("{}/{}.pkg.lua", name, name)
    } else {
        format!("{}/{}/{}.pkg.lua", repo, name, name)
    };

    let url = construct_raw_url(&registry.git, &registry.branch, &file_path)?;
    let client = utils::get_http_client()?;
    let response = client.get(url).send()?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch pkg.lua for package {} from registry {}: {}",
            name,
            registry.name,
            response.status()
        ));
    }

    Ok(response.text()?)
}

#[derive(Debug)]
pub struct ResolvedPurl {
    pub registry_handle: String,
    pub registry: RegistryInfo,
    pub package_path: String,
    pub package_info: PurlPackageIndex,
    pub version: String,
    pub index: RegistryIndex,
}

pub fn resolve_purl(purl_str: &str) -> Result<ResolvedPurl> {
    let purl: GenericPurl<String> = purl_str
        .parse()
        .map_err(|e| anyhow!("Invalid PURL: {}", e))?;

    if purl.package_type() != "zoi" {
        return Err(anyhow!(
            "Unsupported PURL type: {}. Expected 'zoi'.",
            purl.package_type()
        ));
    }

    let namespace = purl
        .namespace()
        .ok_or_else(|| anyhow!("PURL missing registry handle in namespace"))?;
    let mut ns_parts = namespace.split('/');
    let registry_handle = ns_parts
        .next()
        .ok_or_else(|| anyhow!("PURL missing registry handle"))?;
    let package_path = purl.name();
    let version = purl.version().unwrap_or("latest");

    let remaining_ns: Vec<&str> = ns_parts.collect();
    if remaining_ns.is_empty() {
        return Err(anyhow!(
            "PURL missing repository path. Expected format: pkg:zoi/[registry-handle]/[repo]/[package]"
        ));
    }
    let expected_repo = remaining_ns.join("/");

    let central_db = fetch_central_db()?;
    let registry = central_db.get(registry_handle).ok_or_else(|| {
        anyhow!(
            "Registry handle '{}' not found in central database",
            registry_handle
        )
    })?;

    let index = fetch_registry_index(registry)?;

    let full_path = format!("{}/{}", expected_repo, package_path);
    let package_info = if let Some(info) = index.packages.get(&full_path) {
        info
    } else {
        let info = index.packages.get(package_path).ok_or_else(|| {
            anyhow!(
                "Package '{}' not found in registry '{}'",
                package_path,
                registry_handle
            )
        })?;
        if expected_repo != info.repo {
            return Err(anyhow!(
                "Repository mismatch in PURL. Package '{}' is in repository '{}', but PURL specified '{}'",
                package_path,
                info.repo,
                expected_repo
            ));
        }
        info
    };

    let resolved_version = if version == "latest" {
        package_info.version.clone()
    } else {
        version.to_string()
    };

    Ok(ResolvedPurl {
        registry_handle: registry_handle.to_string(),
        registry: registry.clone(),
        package_path: package_path.to_string(),
        package_info: package_info.clone(),
        version: resolved_version,
        index,
    })
}

pub fn fetch_and_store_purl_package(purl_str: &str) -> Result<String> {
    let resolved = resolve_purl(purl_str)?;
    let db_root = crate::pkg::resolve::get_db_root()?;

    let mut fetched = std::collections::HashSet::new();
    fetch_and_store_recursive(
        &resolved.registry_handle,
        &resolved.registry,
        &resolved.index,
        &resolved.package_path,
        &db_root,
        &mut fetched,
    )?;

    let ident = if resolved.package_info.repo.is_empty() {
        format!(
            "#{}@{}@{}",
            resolved.registry_handle, resolved.package_path, resolved.version
        )
    } else {
        format!(
            "#{}@{}/{}@{}",
            resolved.registry_handle,
            resolved.package_info.repo,
            resolved.package_path,
            resolved.version
        )
    };
    Ok(ident)
}

fn fetch_and_store_recursive(
    registry_handle: &str,
    registry: &RegistryInfo,
    index: &RegistryIndex,
    package_path: &str,
    db_root: &Path,
    fetched: &mut std::collections::HashSet<String>,
) -> Result<()> {
    if fetched.contains(package_path) {
        return Ok(());
    }
    fetched.insert(package_path.to_string());

    let pkg_info = index.packages.get(package_path).ok_or_else(|| {
        anyhow!(
            "Dependency '{}' not found in registry '{}'",
            package_path,
            registry_handle
        )
    })?;

    let lua_content = fetch_package_lua(registry, &pkg_info.repo, package_path)?;

    let mut dest_dir = db_root.join(registry_handle);
    if !pkg_info.repo.is_empty() {
        dest_dir = dest_dir.join(&pkg_info.repo);
    }
    dest_dir = dest_dir.join(package_path);

    std::fs::create_dir_all(&dest_dir)?;
    let dest_file = dest_dir.join(format!("{}.pkg.lua", package_path));
    std::fs::write(&dest_file, lua_content)?;

    if let Some(deps) = &pkg_info.dependencies {
        for dep in deps {
            if dep.starts_with('@') {
                continue;
            }

            if index.packages.contains_key(dep) {
                let _ = fetch_and_store_recursive(
                    registry_handle,
                    registry,
                    index,
                    dep,
                    db_root,
                    fetched,
                );
            }
        }
    }
    Ok(())
}
