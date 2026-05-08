use anyhow::{Result, anyhow};
use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniVulnerability {
    pub id: String,
    pub severity: String,
    pub affected_range: String,
    pub fixed_in: Option<String>,
    pub summary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniPackageIndex {
    pub repo: String,
    pub repo_type: String,
    pub version: String,
    pub description: String,
    pub sub_packages: Option<Vec<String>>,
    pub vuln: Option<Vec<MiniVulnerability>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniRegistryIndex {
    pub packages: HashMap<String, MiniPackageIndex>,
}

pub fn fetch_registry_index() -> Result<MiniRegistryIndex> {
    let url = "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoidberg/-/raw/main/packages.json";
    let client = crate::utils::get_http_client()?;
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

    Ok(crate::utils::ask_for_confirmation(
        "Do you want to continue with the installation anyway?",
        false,
    ))
}

pub fn get_package_lua_url(repo: &str, name: &str) -> String {
    format!(
        "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoidberg/-/raw/main/{}/{}/{}.pkg.lua",
        repo, name, name
    )
}
