use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use zoi_core::utils;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Generation {
    pub id: u32,
    pub created_at: DateTime<Utc>,
    pub packages: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
}

pub struct GenerationManager {
    pub root: PathBuf,
}

impl GenerationManager {
    pub fn new() -> Result<Self> {
        Self::with_root(PathBuf::from("/var/lib/zoi/generations"))
    }

    pub fn with_root(root: PathBuf) -> Result<Self> {
        if !root.exists() {
            fs::create_dir_all(&root)?;
        }
        Ok(Self { root })
    }

    pub fn list_generations(&self) -> Result<Vec<Generation>> {
        let mut generations = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            if path.is_dir() {
                let meta_path = path.join("generation.json");
                if meta_path.exists() {
                    let content = fs::read_to_string(meta_path)?;
                    let generation: Generation = serde_json::from_str(&content)?;
                    generations.push(generation);
                }
            }
        }
        generations.sort_by_key(|g| g.id);
        Ok(generations)
    }

    pub fn next_id(&self) -> Result<u32> {
        let gens = self.list_generations()?;
        Ok(gens.last().map(|g| g.id + 1).unwrap_or(1))
    }

    pub fn create_generation(&self, packages: Vec<String>) -> Result<u32> {
        self.create_generation_with_transaction(packages, None)
    }

    pub fn create_generation_with_transaction(
        &self,
        packages: Vec<String>,
        transaction_id: Option<String>,
    ) -> Result<u32> {
        let id = self.next_id()?;
        let gen_path = self.root.join(id.to_string());
        fs::create_dir_all(&gen_path)?;

        let generation = Generation {
            id,
            created_at: Utc::now(),
            packages,
            transaction_id,
        };

        let meta_path = gen_path.join("generation.json");
        fs::write(meta_path, serde_json::to_string_pretty(&generation)?)?;

        Ok(id)
    }

    pub fn activate_generation(&self, id: u32) -> Result<()> {
        // In the traditional model, activation happens during 'zoi system apply'
        // which installs packages directly to usrroot.
        // We can still maintain a 'current' symlink for status purposes.
        let gen_path = self.root.join(id.to_string());
        if !gen_path.exists() {
            return Err(anyhow!("Generation {} does not exist", id));
        }

        let view_root = PathBuf::from("/var/lib/zoi/pkgs/view");
        fs::create_dir_all(&view_root)?;

        let current_view = view_root.join("current");

        if current_view.exists() || current_view.is_symlink() {
            if current_view.is_dir() && !current_view.is_symlink() {
                fs::remove_dir_all(&current_view)?;
            } else {
                fs::remove_file(&current_view)?;
            }
        }

        utils::symlink_dir(&gen_path, &current_view)?;

        println!("Generation {} recorded as active.", id);

        Ok(())
    }

    pub fn get_current_generation_id(&self) -> Result<Option<u32>> {
        let current_view = PathBuf::from("/var/lib/zoi/pkgs/view/current");
        if !current_view.exists() || !current_view.is_symlink() {
            return Ok(None);
        }

        let target = fs::read_link(current_view)?;
        let id_str = target
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid generation symlink target"))?;

        Ok(id_str.parse::<u32>().ok())
    }

    pub fn find_boot_assets(&self, generation: &Generation) -> Result<(PathBuf, PathBuf)> {
        let gen_path = self.root.join(generation.id.to_string());
        let boot_dir = gen_path.join("usr/boot");

        let mut kernel = None;
        let mut initrd = None;

        if boot_dir.exists() {
            for entry in fs::read_dir(boot_dir)? {
                let path = entry?.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("vmlinuz") || name.starts_with("bzImage") {
                        kernel = Some(path);
                    } else if name.starts_with("initramfs") || name.starts_with("initrd") {
                        initrd = Some(path);
                    }
                }
            }
        }

        match (kernel, initrd) {
            (Some(k), Some(i)) => Ok((k, i)),
            _ => Err(anyhow!(
                "Could not find kernel and initrd in generation {}",
                generation.id
            )),
        }
    }

    pub fn prune_generations(&self, limit: u32) -> Result<()> {
        if limit == 0 {
            return Ok(());
        }

        let mut gens = self.list_generations()?;
        if gens.len() <= limit as usize {
            return Ok(());
        }

        let current_id = self.get_current_generation_id()?.unwrap_or(0);

        // Ensure sorted by ID, oldest first
        gens.sort_by_key(|g| g.id);

        let to_remove_count = gens.len() - limit as usize;
        let mut removed = 0;

        let bootloader = crate::boot::detect_bootloader().ok();

        for generation in gens {
            if removed >= to_remove_count {
                break;
            }

            // Never prune the active generation
            if generation.id == current_id {
                continue;
            }

            println!("Pruning old generation {}...", generation.id);
            let gen_path = self.root.join(generation.id.to_string());
            if let Err(e) = fs::remove_dir_all(&gen_path) {
                eprintln!(
                    "Warning: failed to delete generation directory {}: {}",
                    gen_path.display(),
                    e
                );
            }

            if let Some(ref bl) = bootloader {
                let _ = bl.remove_entry(generation.id);
            }

            removed += 1;
        }

        Ok(())
    }
}
