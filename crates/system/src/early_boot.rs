use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

/// Reads the early-boot state the running kernel was started with.
pub struct EarlyBootManager;

impl EarlyBootManager {
    /// Reads `zoi.generation` from the running kernel's command line.
    ///
    /// # Errors
    ///
    /// Returns an error if the kernel command line cannot be read.
    pub fn get_target_generation() -> Result<Option<u32>> {
        let cmdline = fs::read_to_string("/proc/cmdline")?;
        for part in cmdline.split_whitespace() {
            if let Some(stripped) = part.strip_prefix("zoi.generation=") {
                return Ok(stripped.parse::<u32>().ok());
            }
        }
        Ok(None)
    }

    /// Resolves the root directory for the generation the kernel was booted
    /// into.
    ///
    /// # Errors
    ///
    /// Returns an error if the kernel command line names no generation, or if
    /// that generation's root cannot be resolved.
    pub fn prepare_root_mount(generation_id: u32) -> Result<PathBuf> {
        let generations_root = Path::new("/var/lib/zoi/generations");
        let target_gen = generations_root.join(generation_id.to_string());

        if !target_gen.exists() {
            return Err(anyhow!(
                "Target generation {generation_id} not found in store"
            ));
        }

        // Return the path that dracut should mount as /newroot
        Ok(target_gen)
    }
}
