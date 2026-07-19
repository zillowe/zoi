use anyhow::{Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

pub struct EarlyBootManager;

impl EarlyBootManager {
    pub fn get_target_generation() -> Result<Option<u32>> {
        let cmdline = fs::read_to_string("/proc/cmdline")?;
        for part in cmdline.split_whitespace() {
            if let Some(stripped) = part.strip_prefix("zoi.generation=") {
                return Ok(stripped.parse::<u32>().ok());
            }
        }
        Ok(None)
    }

    pub fn prepare_root_mount(generation_id: u32) -> Result<PathBuf> {
        let generations_root = Path::new("/var/lib/zoi/generations");
        let target_gen = generations_root.join(generation_id.to_string());

        if !target_gen.exists() {
            return Err(anyhow!(
                "Target generation {} not found in store",
                generation_id
            ));
        }

        // Return the path that dracut should mount as /newroot
        Ok(target_gen)
    }
}
