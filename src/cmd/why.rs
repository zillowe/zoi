use crate::pkg::{local, types};
use colored::*;
use std::error::Error;
use std::fs;

pub fn run(package_name: &str) -> Result<(), Box<dyn Error>> {
    let trimmed_source = package_name.trim();
    let name_only = if let Some(slash_pos) = trimmed_source.rfind('/') {
        &trimmed_source[slash_pos + 1..]
    } else {
        trimmed_source
    };
    let lower_name_only = name_only.to_lowercase();

    let user_manifest = local::is_package_installed(&lower_name_only, types::Scope::User)?;
    let system_manifest = local::is_package_installed(&lower_name_only, types::Scope::System)?;

    let manifest = match (user_manifest, system_manifest) {
        (Some(m), None) => m,
        (None, Some(m)) => m,
        (Some(_), Some(_)) => {
            return Err(format!(
                "Package '{}' is installed in both user and system scopes. This is an ambiguous state.",
                package_name
            )
            .into());
        }
        (None, None) => {
            return Err(format!("Package '{}' is not installed.", package_name).into());
        }
    };

    let pkg_dir = local::get_store_root(manifest.scope)?.join(&lower_name_only);
    let mut reasons = Vec::new();

    if manifest.reason == types::InstallReason::Direct {
        reasons.push("it was installed directly by the user".to_string());
    }

    let dependents_dir = pkg_dir.join("dependents");
    let mut dependents = Vec::new();
    if dependents_dir.exists() {
        for entry in fs::read_dir(dependents_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(pkg_name) = path.file_name().and_then(|n| n.to_str()) {
                    dependents.push(pkg_name.to_string());
                }
            }
        }
    }

    if !dependents.is_empty() {
        dependents.sort();
        reasons.push(format!(
            "it is a dependency for: {}",
            dependents.join(", ").cyan()
        ));
    }

    if reasons.is_empty() {
        if manifest.reason == types::InstallReason::Dependency {
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
