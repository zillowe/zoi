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
        bins: pkg.bins.clone(),
        conflicts: pkg.conflicts.clone(),
        replaces: pkg.replaces.clone(),
        provides: pkg.provides.clone(),
        backup: pkg.backup.clone(),
        installed_dependencies,
        dependencies_v2: pkg.dependencies.clone().map(types::to_dependencies_v2),
        chosen_options: chosen_options.to_vec(),
        chosen_optionals: chosen_optionals.to_vec(),
        install_method,
        platform,
        service: pkg.service.clone(),
        installed_files,
        installed_size: pkg.installed_size,
        sandbox: pkg.sandbox.clone(),
    })
}
