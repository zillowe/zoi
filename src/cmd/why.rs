use crate::pkg::{local, resolve, types};
use anyhow::{Result, anyhow};
use colored::*;

pub fn run(package_name: &str) -> Result<()> {
    let request = resolve::parse_source_string(package_name)?;
    let (pkg_meta, _, _, _, _) = resolve::resolve_package_and_version(package_name, false)?;

    let user_manifest = local::is_package_installed(
        &pkg_meta.name,
        request.sub_package.as_deref(),
        types::Scope::User,
    )?;
    let system_manifest = local::is_package_installed(
        &pkg_meta.name,
        request.sub_package.as_deref(),
        types::Scope::System,
    )?;
    let _project_manifest = local::is_package_installed(
        &pkg_meta.name,
        request.sub_package.as_deref(),
        types::Scope::Project,
    )?;

    let manifest = match (user_manifest, system_manifest) {
        (Some(m), None) => m,
        (None, Some(m)) => m,
        (Some(_), Some(_)) => {
            return Err(anyhow!(
                "Package '{}' is installed in both user and system scopes. This is an ambiguous state.",
                package_name
            ));
        }
        (None, None) => {
            return Err(anyhow!("Package '{}' is not installed.", package_name));
        }
    };

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
                package_name.bold()
            );
        } else {
            println!(
                "Package '{}' is installed, but its installation reason is unclear.",
                package_name.bold()
            );
        }
    } else {
        println!(
            "Package '{}' is installed because {}.",
            package_name.bold(),
            reasons.join(" and ")
        );
    }

    Ok(())
}
