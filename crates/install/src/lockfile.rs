use anyhow::{Result, anyhow};
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;
use zoi_core::types;

pub fn process_lockfile(
    lockfile_path: &str,
    sources_to_process: &mut Vec<String>,
    temp_files: &mut Vec<NamedTempFile>,
) -> Result<()> {
    println!("=> Installing packages from lockfile: {}", lockfile_path);
    let content = fs::read_to_string(lockfile_path)?;
    let lockfile: types::ZoiLockV2 = serde_json::from_str(&content)?;

    for (pkg_key, pkg) in lockfile.installed_packages {
        let name_with_sub = pkg_key.split('/').next_back().unwrap_or(&pkg_key);
        let name = name_with_sub.split(':').next().unwrap_or(name_with_sub);
        let sub_package = name_with_sub.split(':').nth(1).map(|s| s.to_string());

        let manifest = types::SharableInstallManifest {
            name: name.to_string(),
            version: pkg.version,
            repo: pkg.repo,
            registry_handle: pkg.registry,
            scope: types::Scope::User,
            sub_package,
            chosen_options: Vec::new(),
            chosen_optionals: Vec::new(),
        };

        let mut temp_file = NamedTempFile::new()?;
        let yaml_content = serde_yaml::to_string(&manifest)?;
        temp_file.write_all(yaml_content.as_bytes())?;

        sources_to_process.push(
            temp_file
                .path()
                .to_str()
                .ok_or_else(|| anyhow!("Temporary file path contains invalid UTF-8"))?
                .to_string(),
        );
        temp_files.push(temp_file);
    }

    Ok(())
}
