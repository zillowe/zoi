use crate::pkg::types;
use anyhow::{Result, anyhow};

pub fn create_manifest(
    pkg: &types::Package,
    reason: types::InstallReason,
    installed_dependencies: Vec<String>,
    install_method: Option<String>,
    installed_files: Vec<String>,
    registry_handle: &str,
    chosen_options: &[String],
    chosen_optionals: &[String],
    sub_package: Option<String>,
) -> Result<types::InstallManifest> {
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
        registry_handle: registry_handle.to_string(),
        package_type: pkg.package_type,
        reason,
        scope: pkg.scope,
        bins: pkg.bins.clone(),
        conflicts: pkg.conflicts.clone(),
        replaces: pkg.replaces.clone(),
        provides: pkg.provides.clone(),
        backup: pkg.backup.clone(),
        installed_dependencies,
        chosen_options: chosen_options.to_vec(),
        chosen_optionals: chosen_optionals.to_vec(),
        install_method,
        service: pkg.service.clone(),
        installed_files,
        installed_size: pkg.installed_size,
        sandbox: pkg.sandbox.clone(),
    })
}
