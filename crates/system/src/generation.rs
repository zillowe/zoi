use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zoi_core::utils;

#[derive(Debug, Serialize, Deserialize, Clone)]
/// A recorded state of the system, linked to the transaction that produced it.
pub struct Generation {
    /// Monotonic id, which is also the directory name.
    pub id: u32,
    /// When the record was created.
    pub created_at: DateTime<Utc>,
    /// Packages this record refers to.
    pub packages: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Id of the transaction that produced this record.
    pub transaction_id: Option<String>,
    #[serde(default)]
    /// Whether this record is exempt from pruning.
    pub pinned: bool
}

/// Reads and writes the on-disk generation records.
pub struct GenerationManager {
    /// Filesystem root these operations apply to.
    pub root: PathBuf
}

impl GenerationManager {
    /// Opens the generation store under the active sysroot.
    ///
    /// # Errors
    ///
    /// Returns an error if the generation store cannot be located.
    pub fn new() -> Result<Self> {
        Self::with_root(PathBuf::from("/var/lib/zoi/generations"))
    }

    /// Opens the generation store under an explicit root, creating it if
    /// absent.
    ///
    /// # Errors
    ///
    /// Returns an error if the generation store cannot be created under
    /// `root`.
    pub fn with_root(root: PathBuf) -> Result<Self> {
        if !root.exists() {
            fs::create_dir_all(&root)?;
        }
        Ok(Self { root })
    }

    /// Reads every recorded generation, oldest first.
    ///
    /// # Errors
    ///
    /// Returns an error if the generation store cannot be read.
    pub fn list_generations(&self) -> Result<Vec<Generation>> {
        let mut generations = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            if path.is_dir() {
                let meta_path = path.join("generation.json");
                if meta_path.exists() {
                    let content = fs::read_to_string(meta_path)?;
                    let generation: Generation =
                        serde_json::from_str(&content)?;
                    generations.push(generation);
                }
            }
        }
        generations.sort_by_key(|g| g.id);
        Ok(generations)
    }

    /// Returns the id the next created generation will take.
    ///
    /// # Errors
    ///
    /// Returns an error if the id counter cannot be read or advanced.
    pub fn next_id(&self) -> Result<u32> {
        let gens = self.list_generations()?;
        Ok(gens.last().map_or(1, |g| g.id + 1))
    }

    /// Records a generation with no associated transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if the new record cannot be written.
    pub fn create_generation(&self, packages: Vec<String>) -> Result<u32> {
        self.create_generation_with_transaction(packages, None)
    }

    /// Records a generation, linking it to the transaction that produced it.
    ///
    /// # Errors
    ///
    /// Returns an error if the new record cannot be written.
    pub fn create_generation_with_transaction(
        &self,
        packages: Vec<String>,
        transaction_id: Option<String>
    ) -> Result<u32> {
        let id = self.next_id()?;
        let gen_path = self.root.join(id.to_string());
        fs::create_dir_all(&gen_path)?;

        let generation = Generation {
            id,
            created_at: Utc::now(),
            packages,
            transaction_id,
            pinned: false
        };

        let meta_path = gen_path.join("generation.json");
        fs::write(meta_path, serde_json::to_string_pretty(&generation)?)?;

        Ok(id)
    }

    /// Points the active-generation marker at `id`.
    ///
    /// # Errors
    ///
    /// Returns an error if the generation does not exist, or if the active
    /// pointer cannot be updated.
    pub fn activate_generation(&self, id: u32) -> Result<()> {
        // In the traditional model, activation happens during 'zoi system
        // apply' which installs packages directly to usrroot.
        // We can still maintain a 'current' symlink for status purposes.
        let gen_path = self.root.join(id.to_string());
        if !gen_path.exists() {
            return Err(anyhow!("Generation {id} does not exist"));
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

        println!("Generation {id} recorded as active.");

        Ok(())
    }

    /// Removes Zoi's boot entries for every kernel present in `generation`.
    ///
    /// Called when a generation is pruned, so a bootloader menu does not keep
    /// offering kernels whose files were just deleted. A generation with no
    /// kernel in it (a package-only config, say) is left alone.
    ///
    /// This has to run *before* the generation directory is removed: once it is
    /// gone there is no way to tell which kernels belonged to it. That ordering
    /// is why [`Self::prune_generations`] calls it first.
    ///
    /// # Errors
    ///
    /// Returns an error if the bootloader configuration cannot be rewritten.
    pub fn remove_generation_boot_entries(
        &self,
        generation: &Generation
    ) -> Result<()> {
        let Ok(kernels) = crate::kernel::discover_kernels(Path::new("/"))
        else {
            return Ok(());
        };

        let gen_path = self.root.join(generation.id.to_string());
        let gen_boot = gen_path.join("usr/boot");

        for kernel in kernels {
            // Only touch entries whose kernel actually came from this
            // generation. A kernel shared between generations must keep its
            // entry, otherwise pruning an old generation would unboot a newer
            // one.
            let belongs = gen_boot
                .join(
                    kernel
                        .image
                        .file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new(""))
                )
                .exists();
            if !belongs {
                continue;
            }

            if let Ok(bootloader) = crate::boot::detect_bootloader(None)
                && let Err(e) = bootloader.remove_entry(&kernel.version)
            {
                eprintln!(
                    "Warning: failed to remove boot entry for {}: {}",
                    kernel.version, e
                );
            }
        }

        Ok(())
    }

    /// Reads the active-generation marker.
    ///
    /// # Errors
    ///
    /// Returns an error if the active pointer cannot be read.
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

    /// Locates the kernel and initramfs belonging to a generation.
    ///
    /// # Errors
    ///
    /// Returns an error if the generation record cannot be read.
    pub fn find_boot_assets(
        &self,
        generation: &Generation
    ) -> Result<(PathBuf, PathBuf)> {
        let gen_path = self.root.join(generation.id.to_string());
        let boot_dir = gen_path.join("usr/boot");

        let mut kernel = None;
        let mut initrd = None;

        if boot_dir.exists() {
            for entry in fs::read_dir(boot_dir)? {
                let path = entry?.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("vmlinuz")
                        || name.starts_with("bzImage")
                    {
                        kernel = Some(path);
                    } else if name.starts_with("initramfs")
                        || name.starts_with("initrd")
                    {
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
            ))
        }
    }

    /// Deletes all but the `limit` most recent unpinned generations.
    ///
    /// # Errors
    ///
    /// Returns an error if a pruned generation's bootloader entries or its
    /// directory cannot be removed.
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

        for generation in gens {
            if removed >= to_remove_count {
                break;
            }

            // Never prune the active generation or pinned generations
            if generation.id == current_id || generation.pinned {
                continue;
            }

            println!("Pruning old generation {}...", generation.id);

            // Boot entries go first. Once the generation directory is gone
            // there is no way to tell which kernels belonged to it, so a stale
            // entry would survive pointing at files that no longer exist.
            if let Err(e) = self.remove_generation_boot_entries(&generation) {
                eprintln!(
                    "Warning: failed to remove boot entries for generation \
                     {}: {}",
                    generation.id, e
                );
            }

            let gen_path = self.root.join(generation.id.to_string());
            if let Err(e) = fs::remove_dir_all(&gen_path) {
                eprintln!(
                    "Warning: failed to delete generation directory {}: {}",
                    gen_path.display(),
                    e
                );
            }

            removed += 1;
        }

        Ok(())
    }

    /// Marks a generation as exempt from pruning, or unmarks it.
    ///
    /// # Errors
    ///
    /// Returns an error if the generation does not exist, or if its record
    /// cannot be rewritten.
    pub fn pin_generation(&self, id: u32, pinned: bool) -> Result<()> {
        let gen_path = self.root.join(id.to_string());
        let meta_path = gen_path.join("generation.json");
        if !meta_path.exists() {
            return Err(anyhow!("Generation {id} does not exist"));
        }

        let content = fs::read_to_string(&meta_path)?;
        let mut generation: Generation = serde_json::from_str(&content)?;
        generation.pinned = pinned;

        fs::write(meta_path, serde_json::to_string_pretty(&generation)?)?;

        if pinned {
            println!("Generation {id} pinned successfully.");
        } else {
            println!("Generation {id} unpinned successfully.");
        }

        Ok(())
    }
}
