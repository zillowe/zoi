use anyhow::{Result, anyhow};
use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
pub use zoi_core::types::MiniVulnerability;

fn default_revision() -> String {
    "1".to_string()
}

/// A simplified version of package metadata used in Zoi Mini's remote index.
///
/// This index allows Zoi Mini to perform fast lookups and vulnerability checks
/// without downloading individual `.pkg.lua` files or cloning entire registries.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniPackageIndex {
    pub repo: String,
    pub repo_type: String,
    pub version: String,
    #[serde(default = "default_revision")]
    pub revision: String,
    pub description: String,
    #[serde(default, deserialize_with = "deserialize_sub_packages")]
    pub sub_packages: Option<Vec<String>>,
    pub vuln: Option<Vec<MiniVulnerability>>,
}

fn deserialize_sub_packages<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    if v.is_array() {
        Ok(serde_json::from_value(v).ok())
    } else {
        Ok(None)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniRegistryIndex {
    pub packages: HashMap<String, MiniPackageIndex>,
}

/// Fetches the optimized JSON index from the official Zoidberg registry.
///
/// This index is the backbone of Zoi Mini, providing a pre-resolved mapping
/// of package names to their current versions and metadata.
pub fn fetch_registry_index() -> Result<MiniRegistryIndex> {
    let url = "https://gitlab.com/zillowe/zillwen/zusty/zoidberg/-/raw/main/packages.json";
    let client = zoi_core::utils::get_http_client()?;
    let response = client.get(url).send()?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch packages.json from Zoidberg registry: {}",
            response.status()
        ));
    }
    let index: MiniRegistryIndex = response.json()?;
    Ok(index)
}

pub fn fetch_registry_config() -> Result<zoi_core::types::RepoConfig> {
    let url = "https://gitlab.com/zillowe/zillwen/zusty/zoidberg/-/raw/main/repo.yaml";
    let client = zoi_core::utils::get_http_client()?;
    let response = client.get(url).send()?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch repo.yaml from Zoidberg registry: {}",
            response.status()
        ));
    }
    let content = response.text()?;
    let config: zoi_core::types::RepoConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

pub fn get_package_lua_url(repo: &str, name: &str) -> String {
    format!(
        "https://gitlab.com/zillowe/zillwen/zusty/zoidberg/-/raw/main/{}/{}/{}.pkg.lua",
        repo, name, name
    )
}

/// Scans the package metadata for known security advisories.
///
/// Returns `true` if the package is safe to install, or if the user
/// explicitly chooses to bypass a security warning.
pub fn check_vulnerabilities(
    pkg_name: &str,
    pkg_index: &MiniPackageIndex,
    version: &str,
) -> Result<bool> {
    let Some(vulns) = &pkg_index.vuln else {
        return Ok(true);
    };

    let target_version = semver::Version::parse(version.trim_start_matches('v'))
        .map_err(|e| anyhow!("Failed to parse version {}: {}", version, e))?;

    let mut affected = Vec::new();

    for vuln in vulns {
        if let Ok(req) = semver::VersionReq::parse(&vuln.affected_range)
            && req.matches(&target_version)
        {
            affected.push(vuln);
        }
    }

    if affected.is_empty() {
        return Ok(true);
    }

    println!("\n{}", "SECURITY WARNING".red().bold());
    for vuln in affected {
        println!(
            "Package {} v{} is known to be vulnerable:",
            pkg_name.cyan().bold(),
            version.red()
        );
        println!(
            "[{}] {} (Severity: {})",
            vuln.id.dimmed(),
            vuln.summary,
            vuln.severity.to_uppercase()
        );
        if let Some(fixed) = &vuln.fixed_in {
            println!("Fixed in version: {}", fixed.green());
        }
        println!();
    }

    Ok(zoi_core::utils::ask_for_confirmation(
        "Do you want to continue with the installation anyway?",
        false,
    ))
}
