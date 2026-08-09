//! Package URL (PURL) resolution for Zoi.
//!
//! This crate implements the resolution of PURLs in the `pkg:zoi/` namespace.
//! It allows Zoi to discover, resolve, and fetch package definitions from
//! decentralized Git-backed registries.

use anyhow::{Result, anyhow};
use purl::GenericPurl;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use zoi_core::types::MiniVulnerability;

/// Returns the default version ("1") for the central database.
fn default_version() -> String {
    "1".to_string()
}

/// Returns the default revision ("1") for a package index.
fn default_revision() -> String {
    "1".to_string()
}

/// Specification for the Central Registry Database.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CentralDbSpec {
    /// Version of the database format.
    #[serde(default = "default_version")]
    pub version: String,
    /// Map of registry handles to their connection information.
    #[serde(flatten)]
    pub registries: HashMap<String, RegistryInfo>,
}

/// Connection information for a Zoi package registry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistryInfo {
    /// Human-readable name of the registry.
    pub name: String,
    /// Brief description of the registry's purpose or content.
    pub description: String,
    /// URL to the Git repository containing the package definitions.
    pub git: String,
    /// Branch name to use when fetching data from the Git repository.
    pub branch: String,
}

/// Index entry for a specific package in a registry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PurlPackageIndex {
    /// Repository path within the registry (e.g. "base", "extra").
    pub repo: String,
    /// Type of the repository.
    pub repo_type: String,
    /// Latest version of the package.
    pub version: String,
    /// Revision of the package version.
    #[serde(default = "default_revision")]
    pub revision: String,
    /// Brief description of the package.
    pub description: String,
    /// List of sub-packages included in this package.
    pub sub_packages: Vec<String>,
    /// List of main sub-packages.
    pub main_sub_packages: Vec<String>,
    /// Known vulnerabilities for this package.
    pub vuln: Vec<MiniVulnerability>,
    /// Dependencies required by this package.
    pub dependencies: Option<zoi_core::types::Dependencies>,
}

/// The full index of a Zoi registry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistryIndex {
    /// Version of the registry index format.
    pub version: String,
    /// Map of package identifiers to their index entries.
    pub packages: BTreeMap<String, PurlPackageIndex>,
}

/// Fetches the central Zoi registry database from a remote URL or local file.
///
/// The URL can be overridden by the `ZOI_PURL_DB_URL` environment variable.
///
/// # Errors
/// Returns an error if the database cannot be fetched, verified, or parsed.
pub fn fetch_central_db() -> Result<HashMap<String, RegistryInfo>> {
    let url = std::env::var("ZOI_PURL_DB_URL")
        .unwrap_or_else(|_| "https://zillowe.pages.dev/zoi/registries.json".to_string());

    let is_test = std::env::var("ZOI_TEST").is_ok();
    let data = if url.starts_with("http") {
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
    } else {
        std::fs::read(&url).map_err(|e| anyhow!("Failed to read central DB from {url}: {e}"))?
    };

    let spec: CentralDbSpec = serde_json::from_slice(&data)?;
    Ok(spec.registries)
}

/// Constructs a raw content URL for a file in a Git repository.
///
/// Supports GitHub, GitLab, and Codeberg.
///
/// # Errors
/// Returns an error if the Git provider is unsupported.
pub fn construct_raw_url(git_url: &str, branch: &str, file_path: &str) -> Result<String> {
    let url = git_url.trim_end_matches(".git").trim_end_matches('/');

    if let Some(path) = url.strip_prefix("https://github.com/") {
        Ok(format!(
            "https://raw.githubusercontent.com/{path}/{branch}/{file_path}"
        ))
    } else if let Some(path) = url.strip_prefix("https://gitlab.com/") {
        Ok(format!(
            "https://gitlab.com/{path}/-/raw/{branch}/{file_path}"
        ))
    } else if let Some(path) = url.strip_prefix("https://codeberg.org/") {
        Ok(format!(
            "https://codeberg.org/{path}/raw/branch/{branch}/{file_path}"
        ))
    } else {
        Err(anyhow!(
            "Unsupported git provider for PURL resolution: {git_url}"
        ))
    }
}

/// Fetches the `packages.json` index from a specific registry.
///
/// # Errors
/// Returns an error if the index cannot be fetched or parsed.
pub fn fetch_registry_index(registry: &RegistryInfo) -> Result<RegistryIndex> {
    let data = if registry.git.starts_with("http") {
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
    } else {
        let path = Path::new(&registry.git).join("packages.json");
        std::fs::read(&path).map_err(|e| {
            anyhow!(
                "Failed to read registry index from {}: {}",
                path.display(),
                e
            )
        })?
    };

    Ok(serde_json::from_slice(&data)?)
}

/// Fetches the `.pkg.lua` definition for a package from a registry.
///
/// # Errors
/// Returns an error if the file cannot be fetched or read.
pub fn fetch_package_lua(registry: &RegistryInfo, repo: &str, name: &str) -> Result<String> {
    let file_path = if repo.is_empty() {
        format!("{name}/{name}.pkg.lua")
    } else {
        format!("{repo}/{name}/{name}.pkg.lua")
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

/// Details of a successfully resolved PURL.
#[derive(Debug)]
pub struct ResolvedPurl {
    /// The handle of the registry where the package was found.
    pub registry_handle: String,
    /// Connection info for the registry.
    pub registry: RegistryInfo,
    /// Path to the package within the registry.
    pub package_path: String,
    /// Index entry for the package.
    pub package_info: PurlPackageIndex,
    /// The specific version resolved.
    pub version: String,
    /// The full registry index.
    pub index: RegistryIndex,
}

/// Resolves a Zoi PURL string to its registry and package information.
///
/// Expected format: `pkg:zoi/[registry-handle]/[repo]/[package]`
///
/// # Errors
/// Returns an error if the PURL is invalid, unsupported, or cannot be found in the registry.
pub fn resolve_purl(purl_str: &str) -> Result<ResolvedPurl> {
    let purl: GenericPurl<String> = purl_str
        .parse()
        .map_err(|e| anyhow!("Invalid PURL: {e}"))?;

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
            "Registry handle '{registry_handle}' not found in central database"
        )
    })?;

    let index = fetch_registry_index(registry)?;

    let packages_key = format!("@{expected_repo}/{package_path}");
    let package_info = index.packages.get(&packages_key).ok_or_else(|| {
        anyhow!(
            "Package '{package_path}' not found in registry '{registry_handle}' within repository '{expected_repo}'"
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

/// Fetches a package and all its Zoi dependencies by PURL and stores them locally.
///
/// # Errors
/// Returns an error if resolution, fetching, or local storage fails.
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

/// Recursively fetches and stores package definitions.
///
/// # Errors
/// Returns an error if fetching or writing to disk fails.
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
            "Dependency '{packages_key}' not found in registry '{registry_handle}'"
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
    let dest_file = dest_dir.join(format!("{package_name}.pkg.lua"));
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
            .map_or("", |(repo, _)| repo);

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
                    let scoped = format!("@{current_repo}/{dep_pkg_name}");

                    if index.packages.contains_key(&scoped) {
                        Some(scoped)
                    } else {
                        index
                            .packages
                            .keys()
                            .find(|k| k.ends_with(&format!("/{dep_pkg_name}")))
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
