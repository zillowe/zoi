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
    let lower_package_name = package_name.to_lowercase();

    if pin::is_pinned(&lower_package_name)? {
        return Ok(UpdateResult::Pinned);
    }

    let manifest = local::is_package_installed(&lower_package_name, types::Scope::User)?
        .or(local::is_package_installed(
            &lower_package_name,
            types::Scope::System,
        )?)
        .ok_or(format!(
            "Package '{}' is not installed. Use 'zoi install' instead.",
            package_name
        ))?;

    let (new_pkg, new_version, _, _) = resolve::resolve_package_and_version(&lower_package_name)?;

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
        &lower_package_name,
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
