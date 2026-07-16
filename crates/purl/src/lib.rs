use anyhow::{Result, anyhow};
use purl::GenericPurl;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use zoi_core::types::MiniVulnerability;

/// Manages Package URL (PURL) resolution for the Zoi ecosystem.
///
/// PURL (pkg:zoi/...) enables decentralized, human-readable identifiers
/// that can be resolved to any Zoi registry globally. This module:
/// - Fetches the "Central Database" of known registries.
/// - Resolves PURL namespaces to specific Git-backed registries.
/// - Dynamically fetches `.pkg.lua` definitions from Git providers (GitHub, GitLab, etc.).
///
/// This allows Zoi to install packages without requiring the user to
/// manually add repositories first.
fn default_version() -> String {
    "1".to_string()
}

fn default_revision() -> String {
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
    #[serde(default = "default_revision")]
    pub revision: String,
    pub description: String,
    pub sub_packages: Vec<String>,
    pub main_sub_packages: Vec<String>,
    pub vuln: Vec<MiniVulnerability>,
    pub dependencies: Option<zoi_core::types::Dependencies>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistryIndex {
    pub version: String,
    pub packages: BTreeMap<String, PurlPackageIndex>,
}

pub fn fetch_central_db() -> Result<HashMap<String, RegistryInfo>> {
    let url = std::env::var("ZOI_PURL_DB_URL")
        .unwrap_or_else(|_| "https://zillowe.pages.dev/zoi/registries.json".to_string());

    let is_test = std::env::var("ZOI_TEST").is_ok();
    let data = if !url.starts_with("http") {
        std::fs::read(&url).map_err(|e| anyhow!("Failed to read central DB from {}: {}", url, e))?
    } else {
        let trusted_keys = zoi_core::config::get_builtin_authorities();
        if !trusted_keys.is_empty() && !is_test {
            zoi_core::config::verify_remote_file(&url, &trusted_keys)?
        } else {
            let client = zoi_core::utils::get_http_client()?;
            let response = client.get(&url).send()?;
            if !response.status().is_success() {
                return Err(anyhow!(
                    "Failed to fetch central Zoi registry database: {}",
                    response.status()
                ));
            }
            response.bytes()?.to_vec()
        }
    };

    let spec: CentralDbSpec = serde_json::from_slice(&data)?;
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
    let data = if !registry.git.starts_with("http") {
        let path = Path::new(&registry.git).join("packages.json");
        std::fs::read(&path).map_err(|e| {
            anyhow!(
                "Failed to read registry index from {}: {}",
                path.display(),
                e
            )
        })?
    } else {
        let url = construct_raw_url(&registry.git, &registry.branch, "packages.json")?;
        let client = zoi_core::utils::get_http_client()?;
        let response = client.get(url).send()?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch packages.json from registry {}: {}",
                registry.name,
                response.status()
            ));
        }
        response.bytes()?.to_vec()
    };

    Ok(serde_json::from_slice(&data)?)
}

pub fn fetch_package_lua(registry: &RegistryInfo, repo: &str, name: &str) -> Result<String> {
    let file_path = if repo.is_empty() {
        format!("{}/{}.pkg.lua", name, name)
    } else {
        format!("{}/{}/{}.pkg.lua", repo, name, name)
    };

    if !registry.git.starts_with("http") {
        let path = Path::new(&registry.git).join(&file_path);
        return std::fs::read_to_string(&path)
            .map_err(|e| anyhow!("Failed to read pkg.lua from {}: {}", path.display(), e));
    }

    let url = construct_raw_url(&registry.git, &registry.branch, &file_path)?;
    let client = zoi_core::utils::get_http_client()?;
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

    let packages_key = format!("@{}/{}", expected_repo, package_path);
    let package_info = index.packages.get(&packages_key).ok_or_else(|| {
        anyhow!(
            "Package '{}' not found in registry '{}' within repository '{}'",
            package_path,
            registry_handle,
            expected_repo
        )
    })?;

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
    let db_root = zoi_core::utils::get_db_root()?;

    let mut fetched = std::collections::HashSet::new();
    let packages_key = format!("@{}/{}", resolved.package_info.repo, resolved.package_path);
    fetch_and_store_recursive(
        &resolved.registry_handle,
        &resolved.registry,
        &resolved.index,
        &packages_key,
        &db_root,
        &mut fetched,
    )?;

    let ident = format!(
        "#{}@{}@{}",
        resolved.registry_handle, packages_key, resolved.version
    );
    Ok(ident)
}

fn fetch_and_store_recursive(
    registry_handle: &str,
    registry: &RegistryInfo,
    index: &RegistryIndex,
    packages_key: &str,
    db_root: &Path,
    fetched: &mut std::collections::HashSet<String>,
) -> Result<()> {
    if fetched.contains(packages_key) {
        return Ok(());
    }
    fetched.insert(packages_key.to_string());

    let pkg_info = index.packages.get(packages_key).ok_or_else(|| {
        anyhow!(
            "Dependency '{}' not found in registry '{}'",
            packages_key,
            registry_handle
        )
    })?;

    let package_name = packages_key.split('/').next_back().unwrap_or(packages_key);

    let lua_content = fetch_package_lua(registry, &pkg_info.repo, package_name)?;

    let mut dest_dir = db_root.join(registry_handle);
    if !pkg_info.repo.is_empty() {
        dest_dir = dest_dir.join(&pkg_info.repo);
    }
    dest_dir = dest_dir.join(package_name);

    std::fs::create_dir_all(&dest_dir)?;
    let dest_file = dest_dir.join(format!("{}.pkg.lua", package_name));
    std::fs::write(&dest_file, lua_content)?;

    if let Some(deps) = &pkg_info.dependencies {
        let mut to_fetch = Vec::new();
        if let Some(runtime) = &deps.runtime {
            match runtime {
                zoi_core::types::DependencyGroup::Simple(d) => to_fetch.extend(d.clone()),
                zoi_core::types::DependencyGroup::Complex(c) => {
                    to_fetch.extend(c.required.clone());
                    to_fetch.extend(c.optional.clone());
                    for opt in &c.options {
                        to_fetch.extend(opt.depends.clone());
                    }
                }
            }
        }

        let current_repo = packages_key
            .strip_prefix('@')
            .and_then(|k| k.split_once('/'))
            .map(|(repo, _)| repo)
            .unwrap_or("");

        for dep_str in to_fetch {
            if let Some(zoi_dep) = dep_str.strip_prefix("zoi:") {
                let found_key = if zoi_dep.starts_with('@') {
                    if index.packages.contains_key(zoi_dep) {
                        Some(zoi_dep.to_string())
                    } else {
                        None
                    }
                } else {
                    let dep_pkg_name = zoi_dep.split('@').next().unwrap_or(zoi_dep);
                    let scoped = format!("@{}/{}", current_repo, dep_pkg_name);

                    if index.packages.contains_key(&scoped) {
                        Some(scoped)
                    } else {
                        index
                            .packages
                            .keys()
                            .find(|k| k.ends_with(&format!("/{}", dep_pkg_name)))
                            .cloned()
                    }
                };

                if let Some(key) = found_key {
                    let _ = fetch_and_store_recursive(
                        registry_handle,
                        registry,
                        index,
                        &key,
                        db_root,
                        fetched,
                    );
                }
            }
        }
    }
    Ok(())
}
