use crate::pkg::{install, local, pin, resolve, types};
use crate::utils;
use std::collections::HashSet;
use std::error::Error;

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateResult {
    Updated { from: String, to: String },
    AlreadyUpToDate,
    Pinned,
}

pub fn run(package_name: &str, yes: bool) -> Result<UpdateResult, Box<dyn Error>> {
    let (new_pkg, new_version, _, _) = match resolve::resolve_package_and_version(package_name) {
        Ok(result) => result,
        Err(e) => {
            return Err(format!("Could not resolve package '{}': {}", package_name, e).into());
        }
    };

    if pin::is_pinned(&new_pkg.name)? {
        return Ok(UpdateResult::Pinned);
    }

    let manifest = match local::is_package_installed(&new_pkg.name, types::Scope::User)?.or(
        local::is_package_installed(&new_pkg.name, types::Scope::System)?,
    ) {
        Some(m) => m,
        None => {
            return Err(format!(
                "Package '{}' is not installed. Use 'zoi install' instead.",
                package_name
            )
            .into());
        }
    };

    if manifest.version == new_version {
        return Ok(UpdateResult::AlreadyUpToDate);
    }

    if !utils::ask_for_confirmation(
        &format!("Update from {} to {}?", manifest.version, new_version),
        yes,
    ) {
        return Err("Update aborted by user.".into());
    }

    let mode = if let Some(updater_method) = &new_pkg.updater {
        install::InstallMode::Updater(updater_method.clone())
    } else {
        install::InstallMode::PreferBinary
    };

    let mut processed_deps = HashSet::new();
    install::run_installation(
        package_name,
        mode,
        true,
        types::InstallReason::Direct,
        yes,
        false,
        &mut processed_deps,
        None,
    )?;

    Ok(UpdateResult::Updated {
        from: manifest.version,
        to: new_version,
    })
}
