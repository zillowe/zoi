use crate::pkg::{local, types};
use anyhow::Result;
use chrono::Utc;
use std::error::Error;

pub fn write_manifest(
    pkg: &types::Package,
    reason: types::InstallReason,
    installed_dependencies: Vec<String>,
    install_method: Option<String>,
    installed_files: Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let manifest = types::InstallManifest {
        name: pkg.name.clone(),
        version: pkg.version.clone().expect("Version should be resolved"),
        repo: pkg.repo.clone(),
        installed_at: Utc::now().to_rfc3339(),
        reason,
        scope: pkg.scope,
        bins: pkg.bins.clone(),
        installed_dependencies,
        install_method,
        installed_files,
    };
    local::write_manifest(&manifest)
}
