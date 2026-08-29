//! Integrity verification for installed packages and `.zpa` archives.
//!
//! Powers `zoi package verify`. Two modes of operation:
//! - Archive mode: streams a `.zpa` and checks every pooled file against the
//!   content-addressed pool manifest (size + SHA-256).
//! - Installed mode: re-computes SHA-256 digests of installed files against the
//!   per-file digests that were recorded into the install manifest at install
//!   time. Manifests created before digests were tracked are reported as
//!   "unverified" rather than failing.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use serde::Serialize;
use tar::Archive;
use zoi_core::hash::{HashAlgorithm, calculate_file_hash};
use zoi_core::types::InstallManifest;

/// Verification status of a single installed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyStatus {
    /// Content matches the recorded digest.
    Ok,
    /// Content differs from the recorded digest.
    Modified,
    /// The file no longer exists on disk.
    Missing,
    /// No digest was recorded at install time (legacy manifest).
    Unverified
}

impl VerifyStatus {
    /// Short human-readable tag used by the CLI output.
    #[must_use]
    pub fn as_tag(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Modified => "modified",
            Self::Missing => "missing",
            Self::Unverified => "unverified"
        }
    }
}

/// The verification result for a single installed file.
#[derive(Debug, Clone, Serialize)]
pub struct FileStatus {
    /// The manifest path (placeholder form) of the file.
    pub path: String,
    /// The verification outcome.
    pub status: VerifyStatus
}

/// Result of verifying a package archive against its pooled manifest.
#[derive(Debug, Serialize)]
pub struct ArchiveReport {
    /// Path of the verified archive.
    pub archive: String,
    /// Whether an embedded `manifest.sig` signature entry is present.
    pub signed_embed: bool,
    /// Number of pooled files checked.
    pub checked: usize,
    /// Human-readable descriptions of every problem found.
    pub issues: Vec<String>,
    /// Whether the archive passed all checks.
    pub ok: bool
}

/// Verifies a `.zpa` archive's pool contents against its internal manifest.
///
/// # Errors
///
/// Returns an error if the archive cannot be opened or its manifest cannot
/// be parsed.
pub fn verify_archive(archive_path: &Path) -> Result<ArchiveReport> {
    let file = File::open(archive_path)?;
    let decoder = zstd::stream::read::Decoder::new(file)?;
    let mut archive = Archive::new(decoder);

    let mut manifest_json: Option<String> = None;
    let mut signed_embed = false;
    // Pool file name ("sha256-<hex>") to (size, sha256 hex) observed on disk
    // inside the archive.
    let mut observed: BTreeMap<String, (u64, String)> = BTreeMap::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().to_string();
        let Some(file_name) = Path::new(&name)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
        else {
            continue;
        };

        if file_name == "manifest.json" && manifest_json.is_none() {
            let mut json = String::new();
            entry.read_to_string(&mut json)?;
            manifest_json = Some(json);
            continue;
        }
        if file_name == "manifest.sig" {
            signed_embed = true;
            continue;
        }

        let Some(expected_hash) = file_name.strip_prefix("sha256-") else {
            continue;
        };
        // Only canonical top-level pool entries are content-addressed.
        // Reject nested paths like pool/subdir/sha256-<hex> because
        // installation reads only pool/sha256-<hex> and would reject
        // the archive if the canonical entry is missing.
        let trimmed = name.trim_start_matches("./");
        let canonical = format!("pool/sha256-{expected_hash}");
        if trimmed != canonical {
            continue;
        }
        let (len, hash) = zoi_core::hash::calculate_reader_hash(
            &mut entry,
            HashAlgorithm::Sha256
        )?;
        observed.insert(expected_hash.to_string(), (len, hash));
    }

    let manifest_json = manifest_json.ok_or_else(|| {
        anyhow!(
            "No manifest.json found in '{}' - not a pooled ZPA archive",
            archive_path.display()
        )
    })?;
    let manifest: zoi_core::types::PooledZpaManifest =
        serde_json::from_str(&manifest_json)?;

    let mut issues = Vec::new();
    let mut checked = 0usize;

    for (key, entry) in &manifest.pool {
        let expected_hash = key
            .strip_prefix("sha256-")
            .ok_or_else(|| anyhow!("Unsupported pool hash format: {key}"))?;
        let Some((size, hash)) = observed.get(expected_hash) else {
            issues.push(format!("{key}: missing from archive"));
            continue;
        };
        checked += 1;
        if *size != entry.size {
            issues.push(format!(
                "{key}: size mismatch (manifest {}, archive {size})",
                entry.size
            ));
            continue;
        }
        if hash != expected_hash {
            issues.push(format!("{key}: content hash mismatch"));
        }
    }

    let ok = issues.is_empty();
    Ok(ArchiveReport {
        archive: archive_path.to_string_lossy().to_string(),
        signed_embed,
        checked,
        issues,
        ok
    })
}

/// Verifies the files of an installed package against the digests recorded
/// at install time.
///
/// # Errors
///
/// Returns an error if the package store location cannot be determined or a
/// file cannot be hashed.
pub fn verify_installed(manifest: &InstallManifest) -> Result<Vec<FileStatus>> {
    let version_dir = zoi_resolver::local::get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version
    )?;

    let Some(digests) =
        manifest.file_digests.as_ref().filter(|d| !d.is_empty())
    else {
        return Ok(manifest
            .installed_files
            .iter()
            .map(|path| FileStatus {
                path: path.clone(),
                status: VerifyStatus::Unverified
            })
            .collect());
    };

    let mut statuses = Vec::new();
    for path in &manifest.installed_files {
        // Symlinks and directories are tracked in installed_files but carry
        // no digest; only digest-tracked entries can be verified.
        let Some(expected) = digests.get(path) else {
            continue;
        };

        let expanded = zoi_core::utils::expand_placeholders(
            path,
            &version_dir,
            manifest.scope
        )?;
        let fs_path = PathBuf::from(expanded);

        if !fs_path.exists() && !fs_path.is_symlink() {
            statuses.push(FileStatus {
                path: path.clone(),
                status: VerifyStatus::Missing
            });
            continue;
        }
        if fs_path.is_dir() || fs_path.is_symlink() {
            continue;
        }

        let actual = calculate_file_hash(&fs_path, HashAlgorithm::Sha256)?;
        let expected_hex = expected.strip_prefix("sha256-").unwrap_or(expected);
        statuses.push(FileStatus {
            path: path.clone(),
            status: if actual == expected_hex {
                VerifyStatus::Ok
            } else {
                VerifyStatus::Modified
            }
        });
    }
    Ok(statuses)
}
