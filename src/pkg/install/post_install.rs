use crate::pkg::types;
use anyhow::Result;
use std::fs;

pub fn install_manual_if_available(
    pkg: &types::Package,
    version: &str,
    registry_handle: &str,
    pb: Option<&indicatif::ProgressBar>,
) -> Result<()> {
    if let Some(url) = &pkg.man {
        let msg = format!("Downloading manual from {}...", url);
        if let Some(p) = pb {
            p.println(msg);
        } else {
            println!("{}", msg);
        }

        let content = reqwest::blocking::get(url)?.bytes()?;

        let version_dir = crate::pkg::local::get_package_version_dir(
            pkg.scope,
            registry_handle,
            &pkg.repo,
            &pkg.name,
            version,
        )?;
        fs::create_dir_all(&version_dir)?;

        let extension = if url.ends_with(".md") { "md" } else { "txt" };
        let man_path = version_dir.join(format!("man.{}", extension));

        fs::write(man_path, &content)?;

        let success_msg = format!("Manual for '{}' installed.", pkg.name);
        if let Some(p) = pb {
            p.println(success_msg);
        } else {
            println!("{}", success_msg);
        }
    }
    Ok(())
}
