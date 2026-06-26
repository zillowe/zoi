use crate::pkg::{local, resolve, types};
use anyhow::{Result, anyhow};
use colored::*;

pub fn run(package_name: &str) -> Result<()> {
    let request = resolve::parse_source_string(package_name)?;
    let mut candidates = Vec::new();
    for scope in [
        types::Scope::User,
        types::Scope::System,
        types::Scope::Project,
    ] {
        candidates.extend(local::find_installed_manifests_matching(&request, scope)?);
    }
    if candidates.is_empty() {
        return Err(anyhow!("Package '{}' is not installed.", package_name));
    }
    let manifest =
        crate::cmd::installed_select::choose_installed_manifest(package_name, &candidates, false)?;

    let pkg_dir = local::get_package_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
    )?;
    let mut reasons = Vec::new();

    if manifest.reason == types::InstallReason::Direct {
        reasons.push("it was installed directly by the user".to_string());
    }

    let mut dependents = local::get_dependents(&pkg_dir)?;

    if !dependents.is_empty() {
        dependents.sort();
        reasons.push(format!(
            "it is a dependency for: {}",
            dependents.join(", ").cyan()
        ));
    }

    if reasons.is_empty() {
        if matches!(manifest.reason, types::InstallReason::Dependency { .. }) {
            println!(
                "Package '{}' is installed as a dependency, but no packages list it as a requirement. It may be an orphan.",
                local::installed_manifest_source(&manifest).bold()
            );
        } else {
            println!(
                "Package '{}' is installed, but its installation reason is unclear.",
                local::installed_manifest_source(&manifest).bold()
            );
        }
    } else {
        println!(
            "Package '{}' is installed because {}.",
            local::installed_manifest_source(&manifest).bold(),
            reasons.join(" and ")
        );
    }

    Ok(())
}
