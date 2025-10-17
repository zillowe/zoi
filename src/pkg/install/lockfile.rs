use crate::pkg::types;
use anyhow::Result;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

pub fn process_lockfile(
    lockfile_path: &str,
    sources_to_process: &mut Vec<String>,
    temp_files: &mut Vec<NamedTempFile>,
) -> Result<()> {
    println!("=> Installing packages from lockfile: {}", lockfile_path);
    let content = fs::read_to_string(lockfile_path)?;
    let lockfile: types::Lockfile = serde_json::from_str(&content)?;

    for (_, pkg) in lockfile.packages {
        let manifest = types::SharableInstallManifest {
            name: pkg.name,
            version: pkg.version,
            repo: pkg.repo,
            registry_handle: pkg.registry,
            scope: pkg.scope,
            chosen_options: pkg.chosen_options,
            chosen_optionals: pkg.chosen_optionals,
        };

        let mut temp_file = NamedTempFile::new()?;
        let yaml_content = serde_yaml::to_string(&manifest)?;
        temp_file.write_all(yaml_content.as_bytes())?;

        sources_to_process.push(temp_file.path().to_str().unwrap().to_string());
        temp_files.push(temp_file);
    }

    Ok(())
}
