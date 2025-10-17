use crate::pkg::{local, types};
use anyhow::Result;

pub fn write_manifest(
    pkg: &types::Package,
    reason: types::InstallReason,
    installed_dependencies: Vec<String>,
    install_method: Option<String>,
    installed_files: Vec<String>,
    registry_handle: &str,
    chosen_options: &[String],
    chosen_optionals: &[String],
) -> Result<()> {
    let manifest = types::InstallManifest {
        name: pkg.name.clone(),
        version: pkg.version.clone().expect("Version should be resolved"),
        repo: pkg.repo.clone(),
        registry_handle: registry_handle.to_string(),
        package_type: pkg.package_type,
        reason,
        scope: pkg.scope,
        bins: pkg.bins.clone(),
        conflicts: pkg.conflicts.clone(),
        installed_dependencies,
        chosen_options: chosen_options.to_vec(),
        chosen_optionals: chosen_optionals.to_vec(),
        install_method,
        installed_files,
    };
    local::write_manifest(&manifest)
}
