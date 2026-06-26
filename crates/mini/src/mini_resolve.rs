use anyhow::{Result, anyhow};
use colored::*;
pub use zoi_resolver::mini_resolve::{
    MiniPackageIndex, fetch_registry_config, fetch_registry_index,
};

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

    Ok(zoi_cli::utils::ask_for_confirmation(
        "Do you want to continue with the installation anyway?",
        false,
    ))
}
