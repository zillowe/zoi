use anyhow::{Result, anyhow};
use zoi_core::types;

pub fn create_manifest(
    pkg: &types::Package,
    reason: types::InstallReason,
    installed_dependencies: Vec<String>,
    install_method: Option<String>,
    installed_files: Vec<String>,
    registry_handle: &str,
    repo_type: String,
    chosen_options: &[String],
    chosen_optionals: &[String],
    sub_package: Option<String>,
) -> Result<types::InstallManifest> {
    let platform = zoi_core::utils::get_platform().unwrap_or_default();

    let mut installed_dependencies = installed_dependencies;
    installed_dependencies.sort();

    let mut installed_files = installed_files;
    installed_files.sort();

    let mut chosen_options = chosen_options.to_vec();
    chosen_options.sort();

    let mut chosen_optionals = chosen_optionals.to_vec();
    chosen_optionals.sort();

    let mut bins = pkg.bins.clone();
    if let Some(ref mut b) = bins {
        b.sort();
    }

    let mut conflicts = pkg.conflicts.clone();
    if let Some(ref mut c) = conflicts {
        c.sort();
    }

    let mut replaces = pkg.replaces.clone();
    if let Some(ref mut r) = replaces {
        r.sort();
    }

    let mut provides = pkg.provides.clone();
    if let Some(ref mut p) = provides {
        p.sort();
    }

    let mut backup = pkg.backup.clone();
    if let Some(ref mut b) = backup {
        b.sort();
    }

    let mut dependencies_v2 = pkg.dependencies.clone().map(types::to_dependencies_v2);
    if let Some(ref mut deps) = dependencies_v2 {
        deps.runtime.sort();
        for b in &mut deps.build {
            b.packages.sort();
        }
    }

    Ok(types::InstallManifest {
        name: pkg.name.clone(),
        version: pkg.version.clone().ok_or_else(|| {
            anyhow!(
                "Version should be resolved but was missing for package '{}'",
                pkg.name
            )
        })?,
        revision: pkg.revision.clone(),
        sub_package,
        repo: pkg.repo.clone(),
        repo_type,
        registry_handle: registry_handle.to_string(),
        package_type: pkg.package_type,
        description: pkg.description.clone(),
        reason,
        scope: pkg.scope,
        bins,
        conflicts,
        replaces,
        provides,
        backup,
        installed_dependencies,
        dependencies_v2,
        chosen_options,
        chosen_optionals,
        install_method,
        platform,
        service: pkg.service.clone(),
        installed_files,
        installed_size: pkg.installed_size,
        sandbox: pkg.sandbox.clone(),
    })
}
