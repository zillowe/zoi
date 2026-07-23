use crate::generation::GenerationManager;
use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;
use zoi_core::sysroot;

pub fn initialize_zoios_marker(target: &Path, hostname: Option<&str>, dry_run: bool) -> Result<()> {
    if dry_run {
        println!(
            "  {} Would initialize ZoiOS marker at {}/etc/os-release",
            "[DRY-RUN]".dimmed(),
            target.display()
        );
        return Ok(());
    }

    sysroot::set_sysroot(target.to_path_buf());
    let etc_zoi = target.join("etc/zoi");
    fs::create_dir_all(&etc_zoi)?;

    let os_release = target.join("etc/os-release");
    let marker = format!(
        "ID=zoios\nNAME=ZoiOS\nID_LIKE=zoios\nPRETTY_NAME=\"ZoiOS\"\nHOSTNAME={}\n",
        hostname.unwrap_or("zoios")
    );
    fs::write(os_release, marker)?;
    Ok(())
}

pub fn finalize_first_generation(
    target: &Path,
    packages: Vec<String>,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!(
            "  {} Would create first generation in {}/var/lib/zoi/generations",
            "[DRY-RUN]".dimmed(),
            target.display()
        );
        return Ok(());
    }

    let gen_manager = GenerationManager::with_root(target.join("var/lib/zoi/generations"))?;
    let id = gen_manager.create_generation(packages)?;
    gen_manager.activate_generation(id)?;
    Ok(())
}
