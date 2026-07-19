use crate::config::SystemConfig;
use crate::generation::GenerationManager;
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;
use zoi_core::sysroot;

pub fn prepare_target_filesystems(config: &SystemConfig, dry_run: bool) -> Result<()> {
    for fs_cfg in &config.filesystems {
        println!(
            "  {} Formatting {} as {}...",
            if dry_run {
                "[DRY-RUN]".dimmed().to_string()
            } else {
                "".to_string()
            },
            fs_cfg.device.yellow(),
            fs_cfg.fs_type.cyan()
        );

        if dry_run {
            continue;
        }

        let mkfs_cmd = match fs_cfg.fs_type.as_str() {
            "btrfs" => "mkfs.btrfs",
            "ext4" => "mkfs.ext4",
            "vfat" | "fat32" => "mkfs.vfat",
            _ => return Err(anyhow!("Unsupported filesystem type: {}", fs_cfg.fs_type)),
        };

        let status = Command::new(mkfs_cmd)
            .arg("-f")
            .arg(&fs_cfg.device)
            .status()?;

        if !status.success() {
            return Err(anyhow!("Failed to format {}", fs_cfg.device));
        }

        if fs_cfg.fs_type == "btrfs"
            && let Some(opts) = &fs_cfg.options
            && opts.contains("subvol=")
        {
            let subvol_name = opts
                .split("subvol=")
                .nth(1)
                .and_then(|s| s.split(',').next())
                .ok_or_else(|| anyhow!("Failed to parse subvolume name"))?;

            println!(
                "  Creating Btrfs subvolume {} on {}...",
                subvol_name.cyan(),
                fs_cfg.device
            );
            let temp_mount = tempfile::tempdir()?;
            let mount_status = Command::new("mount")
                .arg(&fs_cfg.device)
                .arg(temp_mount.path())
                .status()?;
            if !mount_status.success() {
                return Err(anyhow!(
                    "Failed to mount {} to temporary directory",
                    fs_cfg.device
                ));
            }

            let subvol_status = Command::new("btrfs")
                .arg("subvolume")
                .arg("create")
                .arg(temp_mount.path().join(subvol_name))
                .status()?;

            let umount_status = Command::new("umount").arg(temp_mount.path()).status()?;

            if !subvol_status.success() {
                return Err(anyhow!(
                    "Failed to create subvolume {} on {}",
                    subvol_name,
                    fs_cfg.device
                ));
            }
            if !umount_status.success() {
                return Err(anyhow!(
                    "Failed to unmount temporary directory {}",
                    temp_mount.path().display()
                ));
            }
        }
    }
    Ok(())
}

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
        "ID=zoios\nNAME=ZoiOS\nID_LIKE=zoios\nPRETTY_NAME=\"ZoiOS (Parlex Foundation)\"\nHOSTNAME={}\n",
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
