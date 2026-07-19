use crate::generation::Generation;
use anyhow::{Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

pub trait BootloaderManager {
    fn name(&self) -> &str;
    fn install_entry(
        &self,
        generation: &Generation,
        kernel_path: &Path,
        initrd_path: &Path,
        cmdline: &str,
    ) -> Result<()>;
    fn remove_entry(&self, generation_id: u32) -> Result<()>;
}

pub struct SystemdBoot;
impl BootloaderManager for SystemdBoot {
    fn name(&self) -> &str {
        "systemd-boot"
    }
    fn install_entry(
        &self,
        generation: &Generation,
        kernel_path: &Path,
        initrd_path: &Path,
        cmdline: &str,
    ) -> Result<()> {
        let entry_content = format!(
            "title ZoiOS Generation {}\nversion {}\nlinux {}\ninitrd {}\noptions zoi.generation={} {}\n",
            generation.id,
            generation.id,
            kernel_path.display(),
            initrd_path.display(),
            generation.id,
            cmdline
        );
        let entry_path = PathBuf::from(format!(
            "/boot/loader/entries/zoios-gen-{}.conf",
            generation.id
        ));
        if let Some(parent) = entry_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(entry_path, entry_content)?;
        Ok(())
    }

    fn remove_entry(&self, generation_id: u32) -> Result<()> {
        let entry_path = PathBuf::from(format!(
            "/boot/loader/entries/zoios-gen-{}.conf",
            generation_id
        ));
        if entry_path.exists() {
            fs::remove_file(entry_path)?;
        }
        Ok(())
    }
}

pub struct Grub2;
impl BootloaderManager for Grub2 {
    fn name(&self) -> &str {
        "grub2"
    }
    fn install_entry(
        &self,
        _generation: &Generation,
        _kernel_path: &Path,
        _initrd_path: &Path,
        _cmdline: &str,
    ) -> Result<()> {
        let script_path = PathBuf::from("/etc/grub.d/15_zoios");

        let script_content = r#"#!/bin/sh
exec tail -n +3 $0
# This file is managed by Zoi. Manual changes will be overwritten.
set -e

# Find all generations and print menuentries
for gen_json in /var/lib/zoi/generations/*/generation.json; do
    gen_dir=$(dirname "$gen_json")
    id=$(basename "$gen_dir")
    
    # Simple extraction via grep/sed to avoid dependencies in shell script
    created_at=$(grep '"created_at"' "$gen_json" | sed 's/.*: "\(.*\)".*/\1/')
    
    # Find assets in the FHS view
    kernel=$(ls "$gen_dir"/usr/boot/vmlinuz* "$gen_dir"/usr/boot/bzImage* 2>/dev/null | head -n 1)
    initrd=$(ls "$gen_dir"/usr/boot/initramfs* "$gen_dir"/usr/boot/initrd* 2>/dev/null | head -n 1)
    
    if [ -n "$kernel" ] && [ -n "$initrd" ]; then
        echo "menuentry 'ZoiOS (Generation $id, $created_at)' {"
        echo "    linux $kernel zoi.generation=$id"
        echo "    initrd $initrd"
        echo "}"
    fi
done
"#;

        if let Some(parent) = script_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&script_path, script_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
        }

        self.update_config()?;
        Ok(())
    }

    fn remove_entry(&self, _generation_id: u32) -> Result<()> {
        // Since the script dynamically lists all generations in /var/lib/zoi/generations,
        // we just need to refresh the config.
        self.update_config()
    }
}

impl Grub2 {
    fn update_config(&self) -> Result<()> {
        println!("Updating GRUB2 configuration...");
        let mut cmd = std::process::Command::new("grub-mkconfig");

        let output_path = if Path::new("/boot/grub2/grub.cfg").exists() {
            "/boot/grub2/grub.cfg"
        } else if Path::new("/boot/grub/grub.cfg").exists() {
            "/boot/grub/grub.cfg"
        } else {
            return Err(anyhow!("Could not locate grub.cfg"));
        };

        let status = cmd.arg("-o").arg(output_path).status()?;
        if !status.success() {
            return Err(anyhow!("grub-mkconfig failed"));
        }
        Ok(())
    }
}

pub fn detect_bootloader() -> Result<Box<dyn BootloaderManager>> {
    if Path::new("/boot/loader/entries").exists() {
        Ok(Box::new(SystemdBoot))
    } else if Path::new("/boot/grub2").exists() || Path::new("/boot/grub").exists() {
        Ok(Box::new(Grub2))
    } else {
        Err(anyhow!("No supported bootloader detected"))
    }
}
