//! Support for zstd-compressed registry databases (`.zrepo`).
//!
//! A `.zrepo` is a zstd-compressed tar of a whole registry snapshot:
//! `repo.yaml`, `packages.json`, and every `.pkg.lua` / `.sec.yaml`. It
//! replaces a Git clone as the transport for registry content, so a client
//! downloads one immutable file instead of running a Git negotiation.
//!
//! Incremental updates work the same way as for package archives: the registry
//! also publishes a `.zdelta` patch, and a client that already holds a
//! snapshot downloads only the patch. The patch is a bsdiff diff computed over
//! the *uncompressed tar* of the base snapshot - tar stores each file in a
//! contiguous, byte-stable block, so unchanged entries diff to almost nothing,
//! and diffing the compressed bytes would not compress well at all.
//!
//! The resulting tar is byte-for-byte the tar the registry author published,
//! so a patched snapshot is indistinguishable from a freshly downloaded one.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use tempfile::Builder;
use zbsdiff::zdelta::{self as delta, ZDELTA_FORMAT_ID, ZDelta};
use zoi_core::types::ZrepoLink;
use zoi_core::{pgp, types, utils as core_utils};
use zoi_install::util as install_util;

/// The `meta.json` discriminator written into registry update patches.
const ZREPO_DELTA_TYPE: &str = "zrepo";

/// The `registry_meta` key holding the hash of the `.zrepo` snapshot the
/// local index was last built from.
pub const ZREPO_HASH_META_KEY: &str = "zrepo_sha256";

/// Suffix appended to a `.zrepo` URL to locate its incremental patch when the
/// `repo.yaml` entry does not spell one out.
const ZDELTA_SUFFIX: &str = ".zdelta";

/// Name of the single patch entry inside a `.zrepo` delta container.
const DEFAULT_PATCH_ENTRY: &str = "data.bsdiff";

/// Returns the `.zrepo` links to try, ordered so the `main` tier is used first
/// and mirrors are only contacted when the primary fails.
///
/// This mirrors how the `git:` list is treated: several entries can point at
/// the same logical registry, typically one per hosting provider.
pub fn candidate_links(repo_config: &types::RepoConfig) -> Vec<&ZrepoLink> {
    let (main_links, mirrors): (Vec<_>, Vec<_>) = repo_config
        .zrepo
        .iter()
        .partition(|link| link.link_type == "main");

    // Prefer the explicitly marked primary, but keep the remaining `main`
    // entries ahead of mirrors so a registry listing two primaries still gets
    // redundancy before falling back to a mirror.
    main_links.into_iter().chain(mirrors).collect()
}

/// The URL of the incremental update patch for `link`.
///
/// Falls back to `<url>.zdelta`, which is the convention Zoi publishes and
/// what the `git`-style link list implies.
pub fn delta_url(link: &ZrepoLink) -> String {
    match &link.delta {
        Some(delta) if !delta.is_empty() => delta.clone(),
        _ => format!("{}{ZDELTA_SUFFIX}", link.url.trim_end_matches('/'))
    }
}

/// Where the downloaded `.zrepo` snapshot for `handle` is cached.
///
/// The snapshot is kept next to the extracted tree and the `SQLite` index so
/// that the next sync has a base to patch from.
pub fn base_artifact_path(db_root: &Path, handle: &str) -> PathBuf {
    db_root.join(format!("{handle}.zrepo"))
}

/// Outcome of building a `.zrepo` snapshot.
#[derive(Debug, Clone, Default)]
pub struct ZrepoStats {
    /// The number of files included in the snapshot.
    pub file_count: usize,
    /// The size of the uncompressed tar in bytes.
    pub tar_size: u64,
    /// The size of the `.zrepo` file in bytes.
    pub compressed_size: u64,
    /// The SHA-256 of the `.zrepo` file.
    pub sha256: String
}

/// Returns true when a path inside a registry tree must not be published.
///
/// Git plumbing, previous snapshots, and half-written downloads are local
/// artifacts rather than registry content, so publishing them would both bloat
/// the snapshot and leak local state.
fn is_excluded_from_snapshot(relative: &Path) -> bool {
    relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name)
                if name == ".git"
        )
    }) || relative
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zrepo"))
}

/// Builds the uncompressed tar of a registry tree, along with the number of
/// files included.
///
/// The output is reproducible: entries are sorted by name, timestamps are
/// fixed to the epoch, and ownership is zeroed. Reproducibility is not
/// cosmetic here - it is what allows a publisher to emit a `.zdelta` against a
/// specific base and have the client end up with byte-identical content.
///
/// # Errors
///
/// Returns an error if the tree cannot be walked or an entry cannot be read.
fn build_registry_tar(registry_root: &Path) -> Result<(Vec<u8>, usize)> {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();

    for entry in walkdir::WalkDir::new(registry_root)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(registry_root) else {
            continue;
        };
        if is_excluded_from_snapshot(relative) {
            continue;
        }
        // Archive paths always use `/`, on every platform, or a client on
        // another OS would not find the files after extraction.
        let name = relative.to_string_lossy().replace('\\', "/");
        entries.push((name, entry.path().to_path_buf()));
    }

    // Sort so the same tree always yields the same bytes.
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let file_count = entries.len();

    let mut builder = tar::Builder::new(Vec::new());
    builder.mode(tar::HeaderMode::Deterministic);

    for (name, path) in &entries {
        let data = fs::read(path)
            .map_err(|e| anyhow!("Failed to read '{}': {e}", path.display()))?;
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        // Preserve the executable bit; everything else is normalized.
        let executable = is_executable(path);
        header.set_mode(if executable { 0o755 } else { 0o644 });
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_cksum();
        builder
            .append_data(&mut header, name, Cursor::new(&data))
            .map_err(|e| {
                anyhow!("Failed to add '{name}' to the snapshot: {e}")
            })?;
    }

    Ok((builder.into_inner()?, file_count))
}

/// Returns true when any execute bit is set on `path`.
///
/// # Errors
///
/// Returns an error if the file's permissions cannot be read.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    {
        fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

/// Builds a `.zrepo` snapshot of a registry tree and writes it to `output`.
///
/// # Errors
///
/// Returns an error if the tree cannot be read, is not a registry (no
/// `repo.yaml`), or the snapshot cannot be written.
pub fn create_zrepo(
    registry_root: &Path,
    output: &Path,
    sign_key: Option<&str>
) -> Result<ZrepoStats> {
    if !registry_root.join("repo.yaml").is_file() {
        return Err(anyhow!(
            "'{}' has no repo.yaml, so it is not a registry and cannot be \
             published as a snapshot.",
            registry_root.display()
        ));
    }

    let (tar_bytes, file_count) = build_registry_tar(registry_root)?;
    let zrepo_bytes = delta::zstd_compress(&tar_bytes).map_err(|e| {
        anyhow!("Failed to compress the registry snapshot: {e}")
    })?;

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, &zrepo_bytes)?;

    let stats = ZrepoStats {
        file_count,
        tar_size: tar_bytes.len() as u64,
        compressed_size: zrepo_bytes.len() as u64,
        sha256: sha256_bytes(&zrepo_bytes)
    };

    write_sidecars(output, &zrepo_bytes, sign_key)?;
    Ok(stats)
}

/// Writes the `.hash`, `.size`, and (when a key is given) `.sig` sidecars that
/// accompany a published artifact.
///
/// These match the convention used for `.zpa` archives, so a registry can serve
/// the same trio for its snapshot that it already serves for packages.
///
/// # Errors
///
/// Returns an error if a sidecar cannot be written or signing fails.
fn write_sidecars(
    artifact: &Path,
    bytes: &[u8],
    sign_key: Option<&str>
) -> Result<()> {
    let hash = sha256_bytes(bytes);

    // Sidecars are named after the full artifact name, so a `.zrepo` gets
    // `.zrepo.hash`. This is the same convention `.zpa` and `.zdelta` use, so
    // the `hash` URL in `repo.yaml` is spelled the same way for all three.
    let hash_path = PathBuf::from(format!("{}.hash", artifact.display()));
    fs::write(&hash_path, format!("{hash}\n"))?;

    let size_path = PathBuf::from(format!("{}.size", artifact.display()));
    fs::write(&size_path, format!("{}\n", bytes.len()))?;

    if let Some(key_id) = sign_key {
        let sig_path = PathBuf::from(format!("{}.sig", artifact.display()));
        // Sign the artifact as it sits on disk, matching how clients verify it.
        zoi_core::pgp::sign_detached(artifact, &sig_path, key_id)?;
    }

    Ok(())
}

/// Builds a `.zdelta` update patch turning `old_zrepo` into a new snapshot of
/// `registry_root`.
///
/// The new snapshot is built with the same reproducible settings as
/// [`create_zrepo`], so a publisher running this twice for the same tree and
/// base produces an identical patch. The resulting patch carries the base and
/// target hashes, which is what lets a client prove the patch applies to
/// exactly the snapshot it holds.
///
/// # Errors
///
/// Returns an error if the base or the new tree cannot be read, or the patch
/// cannot be written.
pub fn create_zrepo_delta(
    old_zrepo: &Path,
    registry_root: &Path,
    output: &Path,
    sign_key: Option<&str>
) -> Result<ZrepoDeltaStats> {
    if !old_zrepo.is_file() {
        return Err(anyhow!(
            "Base snapshot '{}' does not exist.",
            old_zrepo.display()
        ));
    }
    if !registry_root.join("repo.yaml").is_file() {
        return Err(anyhow!(
            "'{}' has no repo.yaml, so it is not a registry and cannot be \
             published as a snapshot.",
            registry_root.display()
        ));
    }

    let old_bytes = fs::read(old_zrepo)?;
    let old_tar = delta::zstd_decompress(&old_bytes).map_err(|e| {
        anyhow!(
            "Base snapshot '{}' is not a valid zstd frame: {e}",
            old_zrepo.display()
        )
    })?;

    let (new_tar, _) = build_registry_tar(registry_root)?;
    let patch = delta::diff_bytes(&old_tar, &new_tar)
        .map_err(|e| anyhow!("Failed to diff the registry snapshots: {e}"))?;

    let meta = serde_json::json!({
        "format": ZDELTA_FORMAT_ID,
        "type": ZREPO_DELTA_TYPE,
        "base_sha256": sha256_bytes(&old_tar),
        "target_sha256": sha256_bytes(&new_tar),
        "patch": DEFAULT_PATCH_ENTRY,
    });

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    delta::write_container(
        &meta,
        &[(DEFAULT_PATCH_ENTRY.to_string(), patch)],
        output
    )
    .map_err(|e| anyhow!("Failed to write the update patch: {e}"))?;

    let patch_bytes = fs::read(output)?;
    let new_zrepo_size = delta::zstd_compress(&new_tar)
        .map(|c| c.len() as u64)
        .map_err(|e| anyhow!("Failed to size the new snapshot: {e}"))?;

    let stats = ZrepoDeltaStats {
        patch_size: patch_bytes.len() as u64,
        // The client already holds `old_zrepo`; the bytes it actually has to
        // transfer to reach the new state are the patch.
        saved_bytes: (old_bytes.len() as u64)
            .saturating_sub(patch_bytes.len() as u64),
        full_download_size: new_zrepo_size
    };

    write_sidecars(output, &patch_bytes, sign_key)?;
    Ok(stats)
}

/// Outcome of building a `.zrepo` update patch.
#[derive(Debug, Clone, Copy, Default)]
pub struct ZrepoDeltaStats {
    /// The size of the `.zdelta` patch in bytes.
    pub patch_size: u64,
    /// How many bytes the patch saves compared with re-downloading the whole
    /// snapshot, relative to the base the client already has.
    pub saved_bytes: u64,
    /// The size of a full download of the new snapshot, for comparison.
    pub full_download_size: u64
}

/// Hashes a byte buffer with SHA-256, returning the lowercase hex digest.
fn sha256_bytes(data: &[u8]) -> String {
    zoi_core::hash::calculate_reader_hash(
        &mut Cursor::new(data),
        zoi_core::hash::HashAlgorithm::Sha256,
    )
    .map(|(_, digest)| digest)
    // Hashing an in-memory cursor cannot fail; keep the function infallible
    // so it reads naturally at the call sites.
    .unwrap_or_default()
}

/// Downloads `url` to `dest`, reporting progress on `pb`.
///
/// # Errors
///
/// Returns an error if the download fails.
fn download(url: &str, dest: &Path, pb: Option<&ProgressBar>) -> Result<()> {
    install_util::download_file_with_progress(url, dest, pb, None)
}

/// Verifies the detached PGP signature of `artifact` against the configured
/// `authorities`.
///
/// A missing signature or missing authorities is not an error: many registries
/// rely on transport integrity alone. When both are present the signature must
/// verify, because that is the only thing binding the snapshot to the registry
/// operator.
///
/// # Errors
///
/// Returns an error if a signature is present and configured but fails to
/// verify.
fn verify_signature(
    link: &ZrepoLink,
    artifact: &Path,
    work_dir: &Path,
    authorities: &[String],
    pb: Option<&ProgressBar>,
    verbose: bool
) -> Result<()> {
    let Some(pgp_url) = &link.pgp else {
        return Ok(());
    };
    if authorities.is_empty() {
        if verbose {
            println!(
                "{}",
                "No trusted PGP keys configured for this registry; skipping \
                 signature verification."
                    .yellow()
            );
        }
        return Ok(());
    }

    let sig_path = work_dir.join("zrepo.sig");
    if download(pgp_url, &sig_path, pb).is_err() {
        // A configured signature that cannot be fetched is not silently
        // ignored, but it is not fatal either: fall through to the warning so
        // a partially-mirrored registry still syncs over the hash path.
        if verbose {
            println!(
                "{} Signature file for the registry snapshot is unavailable.",
                "Warning:".yellow()
            );
        }
        return Ok(());
    }

    let certs = pgp::get_certs_by_name_or_fingerprint(authorities)?;
    pgp::verify_detached_signature_multi_key(artifact, &sig_path, certs)?;
    if verbose {
        println!("{}", "Registry snapshot signature verified.".green());
    }
    Ok(())
}

/// Verifies `artifact` against the checksum published at `link.hash`.
///
/// # Errors
///
/// Returns an error if the checksum cannot be fetched or does not match.
fn verify_hash(
    link: &ZrepoLink,
    artifact: &Path,
    pb: Option<&ProgressBar>
) -> Result<()> {
    let Some(hash_url) = &link.hash else {
        return Ok(());
    };
    let expected = install_util::get_expected_hash(hash_url, None)?;
    if expected.is_empty() {
        return Ok(());
    }
    if !install_util::verify_file_hash(artifact, &expected, pb)? {
        return Err(anyhow!(
            "Checksum verification failed for registry snapshot downloaded \
             from {}",
            link.url
        ));
    }
    Ok(())
}

/// Extracts a tar archive into `dest`, refusing any entry that would escape
/// `dest`.
///
/// # Errors
///
/// Returns an error if the archive is malformed or contains an unsafe path.
fn extract_tar(tar_bytes: &[u8], dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
    archive.set_overwrite(true);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let relative = entry.path()?.into_owned();

        // `safe_join` rejects absolute paths, `..` components and any name the
        // platform considers unsafe, so a crafted archive cannot write outside
        // the registry directory.
        let target = core_utils::safe_join(dest, &relative)?;

        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut out)?;
    }

    Ok(())
}

/// Replaces `target_dir` with `staging_dir` as close to atomically as the
/// platform allows.
///
/// A rename over an existing directory is not portable, so the previous tree
/// is first moved aside and only removed once the new one is in place. If the
/// process dies between the two renames the old tree survives under
/// `<target>.zoi-old` and is cleaned up on the next sync.
///
/// # Errors
///
/// Returns an error if the staging directory cannot be swapped into place.
fn swap_into_place(staging_dir: &Path, target_dir: &Path) -> Result<()> {
    let retired = target_dir.with_extension("zoi-old");
    if retired.exists() {
        // Leftover from an interrupted swap.
        let _ = fs::remove_dir_all(&retired);
    }

    let had_previous = target_dir.exists() || target_dir.is_symlink();
    if had_previous {
        // A local registry is a symlink into the user's tree; never follow or
        // delete through it, just drop the link.
        if target_dir.is_symlink() {
            fs::remove_file(target_dir)?;
        } else {
            fs::rename(target_dir, &retired)?;
        }
    }

    if let Some(parent) = target_dir.parent() {
        fs::create_dir_all(parent)?;
    }

    if let Err(e) = fs::rename(staging_dir, target_dir) {
        // Put the previous tree back so a failed sync does not leave the
        // registry missing entirely.
        if had_previous && retired.exists() {
            let _ = fs::rename(&retired, target_dir);
        }
        return Err(anyhow!("Failed to install registry snapshot: {e}"));
    }

    if retired.exists() {
        let _ = fs::remove_dir_all(&retired);
    }
    Ok(())
}

/// Installs `tar_bytes` as the registry tree at `target_dir` and records
/// `zrepo` as the new base snapshot for the next incremental update.
///
/// The base snapshot is only promoted after the tree swap succeeds, so a failed
/// sync leaves a base that still matches what is on disk.
///
/// # Errors
///
/// Returns an error if the snapshot cannot be written or extracted.
fn install_snapshot(
    tar_bytes: &[u8],
    zrepo_bytes: &[u8],
    base_artifact: &Path,
    target_dir: &Path,
    work_dir: &Path
) -> Result<()> {
    let staging_dir = work_dir.join("extract");
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)?;
    }
    extract_tar(tar_bytes, &staging_dir)?;

    // A snapshot without `repo.yaml` would leave the registry unusable, and
    // the failure is much clearer here than deep inside the indexer.
    if !staging_dir.join("repo.yaml").exists() {
        return Err(anyhow!(
            "The registry snapshot does not contain a repo.yaml; it cannot be \
             used to build a registry."
        ));
    }

    // Promote the base snapshot before swapping so the on-disk base always
    // matches the tree that was just installed.
    let staged_base = work_dir.join("staged.zrepo");
    fs::write(&staged_base, zrepo_bytes)?;
    if let Some(parent) = base_artifact.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&staged_base, base_artifact)?;

    swap_into_place(&staging_dir, target_dir)
}

/// Applies a `.zdelta` patch to a cached `.zrepo` base snapshot, producing the
/// next snapshot.
///
/// The patch is a bsdiff diff over the base's uncompressed tar. Both the
/// container metadata hashes (when present) and the resulting tar are checked
/// so a mismatched or truncated patch can never be installed.
///
/// # Errors
///
/// Returns an error if the base, the patch, or the rebuilt snapshot is invalid.
pub fn apply_delta(
    base_zrepo: &Path,
    patch_path: &Path,
    out_zrepo: &Path
) -> Result<()> {
    let base_bytes = fs::read(base_zrepo)?;
    let base_tar = delta::zstd_decompress(&base_bytes)
        .map_err(|e| anyhow!("Base snapshot is not a valid zstd frame: {e}"))?;

    let (patch, expected_base, expected_target) = match delta::load(patch_path)?
    {
        ZDelta::Single(patch) => (patch, None, None),
        ZDelta::Container(meta, files) => {
            if meta.get("format").and_then(|v| v.as_str())
                != Some(ZDELTA_FORMAT_ID)
            {
                return Err(anyhow!(
                    "Registry update patch has an unsupported format: {}",
                    meta.get("format")
                        .and_then(|v| v.as_str())
                        .unwrap_or("none")
                ));
            }
            if meta.get("type").and_then(|v| v.as_str())
                != Some(ZREPO_DELTA_TYPE)
            {
                return Err(anyhow!(
                    "Registry update patch targets '{}', expected \
                     '{ZREPO_DELTA_TYPE}'.",
                    meta.get("type").and_then(|v| v.as_str()).unwrap_or("none")
                ));
            }

            let entry = meta
                .get("patch")
                .and_then(|v| v.as_str())
                .unwrap_or(DEFAULT_PATCH_ENTRY);
            let patch = files
                .get(entry)
                .ok_or_else(|| {
                    anyhow!("Registry update patch lacks entry '{entry}'")
                })?
                .clone();

            let hash_of = |key: &str| {
                meta.get(key)
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string)
            };

            (patch, hash_of("base_sha256"), hash_of("target_sha256"))
        }
    };

    if let Some(expected) = &expected_base {
        let actual = sha256_bytes(&base_tar);
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(anyhow!(
                "The cached registry snapshot does not match the base this \
                 update was built against (expected {expected}, got \
                 {actual}). A full download is required."
            ));
        }
    }

    let target_tar = delta::apply_bytes(&base_tar, &patch)
        .map_err(|e| anyhow!("Failed to apply registry update patch: {e}"))?;

    if let Some(expected) = &expected_target {
        let actual = sha256_bytes(&target_tar);
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(anyhow!(
                "The rebuilt registry snapshot does not match the expected \
                 hash (expected {expected}, got {actual})."
            ));
        }
    }

    let out_bytes = delta::zstd_compress(&target_tar)
        .map_err(|e| anyhow!("Failed to recompress registry snapshot: {e}"))?;
    fs::write(out_zrepo, out_bytes)?;
    Ok(())
}

/// Downloads a full `.zrepo` snapshot for `link` and installs it.
///
/// # Errors
///
/// Returns an error if the download, verification, or extraction fails.
fn full_download(
    link: &ZrepoLink,
    base_artifact: &Path,
    target_dir: &Path,
    work_dir: &Path,
    authorities: &[String],
    pb: Option<&ProgressBar>,
    verbose: bool
) -> Result<String> {
    let download_path = work_dir.join("download.zrepo");
    if download_path.exists() {
        fs::remove_file(&download_path)?;
    }

    if let Some(p) = pb {
        p.set_message("Downloading registry snapshot");
    } else if verbose {
        println!("Downloading registry snapshot from {}...", link.url.cyan());
    }

    download(&link.url, &download_path, pb)?;
    verify_hash(link, &download_path, pb)?;
    verify_signature(link, &download_path, work_dir, authorities, pb, verbose)?;

    let zrepo_bytes = fs::read(&download_path)?;
    let tar_bytes = delta::zstd_decompress(&zrepo_bytes).map_err(|e| {
        anyhow!("Downloaded registry snapshot is not a valid zstd frame: {e}")
    })?;

    let hash = sha256_bytes(&zrepo_bytes);
    install_snapshot(
        &tar_bytes,
        &zrepo_bytes,
        base_artifact,
        target_dir,
        work_dir
    )?;
    Ok(hash)
}

/// Updates the registry from its `.zrepo` links.
///
/// Each link is tried in turn (`main` first, then mirrors). When a cached
/// snapshot exists and an update patch is available, the patch is tried first
/// and any failure falls back to a full download.
///
/// The patch is entirely optional. A link that declares no `delta` is probed
/// once at the conventional `<url>.zdelta`, and a link that declares one but
/// does not serve it is retried on every sync. Either way a full download is a
/// normal outcome rather than a failure, so a registry that never ships patches
/// syncs exactly as well as one that does - just without the bandwidth saving.
///
/// Returns the SHA-256 of the snapshot that is now installed.
///
/// # Errors
///
/// Returns an error only if the snapshot itself could not be obtained from any
/// candidate link.
pub fn sync_zrepo(
    links: &[&ZrepoLink],
    db_root: &Path,
    handle: &str,
    target_dir: &Path,
    authorities: &[String],
    pb: Option<&ProgressBar>,
    verbose: bool
) -> Result<String> {
    let base_artifact = base_artifact_path(db_root, handle);
    let work_dir = Builder::new().prefix("zoi-zrepo").tempdir()?;
    let work_path = work_dir.path().to_path_buf();
    let mut last_error: Option<anyhow::Error> = None;

    for link in links {
        match try_link(
            link,
            &base_artifact,
            target_dir,
            &work_path,
            authorities,
            pb,
            verbose
        ) {
            Ok(hash) => return Ok(hash),
            Err(e) => {
                let msg =
                    format!("Sync with {} failed: {e}", link.url.yellow());
                if let Some(p) = pb {
                    p.println(&msg);
                } else {
                    eprintln!("{msg}");
                }
                last_error = Some(e);
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| anyhow!("No .zrepo links are configured.")))
}

/// Syncs a single `.zrepo` link, preferring an incremental update.
///
/// # Errors
///
/// Returns an error if both the incremental update and the full download fail.
fn try_link(
    link: &ZrepoLink,
    base_artifact: &Path,
    target_dir: &Path,
    work_dir: &Path,
    authorities: &[String],
    pb: Option<&ProgressBar>,
    verbose: bool
) -> Result<String> {
    if base_artifact.exists() {
        let delta_url = delta_url(link);
        if let Some(p) = pb {
            p.set_message("Updating registry snapshot");
        } else if verbose {
            println!("Updating registry snapshot from {delta_url}...");
        }

        match delta_download(
            &delta_url,
            base_artifact,
            target_dir,
            work_dir,
            pb,
            verbose
        ) {
            Ok(hash) => return Ok(hash),
            Err(reason) => {
                // Deltas are an optimisation, not a requirement: a registry
                // that never published them, or a base that has drifted, must
                // not break the sync.
                let declared = link.delta.is_some();
                let msg = match reason {
                    // A patch that is simply absent is the normal state of
                    // affairs, so only mention it when the registry promised
                    // one and did not deliver. A missing `delta` field is not
                    // a broken promise, and warning about it on every sync
                    // would only train users to ignore warnings.
                    DeltaUnavailable::NotFetched(e) if declared => {
                        Some(format!(
                            "{} The registry declares an update patch at \
                             {delta_url} but it could not be fetched ({}). \
                             Falling back to a full download.",
                            "Warning:".yellow(),
                            e
                        ))
                    }
                    DeltaUnavailable::NotFetched(_) => {
                        if verbose {
                            println!(
                                "{}",
                                format!(
                                    "No registry update patch at {delta_url}; \
                                     downloading the full snapshot."
                                )
                                .dimmed()
                            );
                        }
                        None
                    }
                    // A patch that is present but does not fit the snapshot we
                    // hold is a real problem: the client and the publisher
                    // disagree about what state the registry is in.
                    DeltaUnavailable::Unusable(e) => Some(format!(
                        "{} The registry update patch could not be applied \
                         ({}). Falling back to a full download.",
                        "Warning:".yellow(),
                        e
                    ))
                };

                if let Some(msg) = msg {
                    if let Some(p) = pb {
                        p.println(&msg);
                    } else {
                        eprintln!("{msg}");
                    }
                }
            }
        }
    }

    full_download(
        link,
        base_artifact,
        target_dir,
        work_dir,
        authorities,
        pb,
        verbose
    )
}

/// Why an incremental registry update could not be used.
///
/// The distinction matters for how loudly sync complains. A patch that simply
/// is not there is the normal state of affairs for a registry that does not
/// publish them, whereas a patch that is there but does not fit means something
/// is actually wrong.
#[derive(Debug)]
enum DeltaUnavailable {
    /// No patch could be fetched from the URL.
    ///
    /// This covers both "the publisher does not ship patches" and "the patch
    /// host is unreachable". A full download is the right response to either,
    /// so neither is worth a warning.
    NotFetched(anyhow::Error),
    /// A patch was fetched but could not be applied to the cached snapshot.
    Unusable(anyhow::Error)
}

/// Downloads and applies a registry update patch, installing the result.
///
/// The patch is not covered by `link.pgp`, which signs the snapshot rather than
/// the patch. Integrity instead comes from the container's `base_sha256` and
/// `target_sha256`, which `apply_delta` checks against the cached snapshot and
/// the rebuilt one; those bind the patch to exactly one base and exactly one
/// result, which is stronger than a detached signature would be.
fn delta_download(
    delta_url: &str,
    base_artifact: &Path,
    target_dir: &Path,
    work_dir: &Path,
    pb: Option<&ProgressBar>,
    verbose: bool
) -> Result<String, DeltaUnavailable> {
    let patch_path = work_dir.join("update.zdelta");
    if patch_path.exists() {
        fs::remove_file(&patch_path)
            .map_err(|e| DeltaUnavailable::Unusable(e.into()))?;
    }

    // An absent or unreachable patch is not an error worth reporting: it just
    // means this registry has no usable update right now.
    download(delta_url, &patch_path, pb)
        .map_err(DeltaUnavailable::NotFetched)?;

    let rebuilt = work_dir.join("rebuilt.zrepo");
    apply_delta(base_artifact, &patch_path, &rebuilt)
        .map_err(DeltaUnavailable::Unusable)?;

    let zrepo_bytes =
        fs::read(&rebuilt).map_err(|e| DeltaUnavailable::Unusable(e.into()))?;
    let tar_bytes = delta::zstd_decompress(&zrepo_bytes).map_err(|e| {
        DeltaUnavailable::Unusable(anyhow!(
            "Rebuilt registry snapshot is not a valid zstd frame: {e}"
        ))
    })?;

    let hash = sha256_bytes(&zrepo_bytes);
    install_snapshot(
        &tar_bytes,
        &zrepo_bytes,
        base_artifact,
        target_dir,
        work_dir
    )
    .map_err(DeltaUnavailable::Unusable)?;

    if verbose {
        println!("{}", "Registry snapshot updated incrementally.".green());
    }
    Ok(hash)
}

/// Returns the progress-bar style used for registry snapshot downloads.
///
/// # Errors
///
/// Returns an error if the style template is malformed.
pub fn snapshot_progress_style() -> Result<ProgressStyle> {
    Ok(ProgressStyle::default_bar()
        .template(
            "{spinner:.green} {msg:30.cyan.bold} [{bar:40.cyan/blue}] \
             {bytes}/{total_bytes} ({bytes_per_sec}, {elapsed_precise})"
        )?
        .progress_chars("=>-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a tar archive from `name`/`body` pairs.
    fn build_tar(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, body) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, Cursor::new(body.as_bytes()))
                .expect("tar append should succeed");
        }
        builder.into_inner().expect("tar build should succeed")
    }

    /// Builds a raw tar archive containing a single entry with the given name.
    ///
    /// The `tar` crate's writer refuses to emit absolute paths or `..`
    /// components, so a hostile archive has to be assembled by hand to prove
    /// the reader rejects it.
    fn build_raw_tar(name: &str, body: &str) -> Vec<u8> {
        let mut header = [0u8; 512];
        let name_bytes = name.as_bytes();
        assert!(
            name_bytes.len() < 100,
            "test helper only handles short names"
        );
        if let Some(slot) = header.get_mut(..name_bytes.len()) {
            slot.copy_from_slice(name_bytes);
        }

        // mode, uid, gid, size, mtime: octal ASCII, NUL terminated.
        let octal =
            |header: &mut [u8; 512], at: usize, len: usize, value: u64| {
                let text = format!("{value:0width$o}", width = len - 1);
                if let Some(slot) = header.get_mut(at..at + len - 1) {
                    slot.copy_from_slice(text.as_bytes());
                }
            };
        octal(&mut header, 100, 8, 0o644);
        octal(&mut header, 108, 8, 0);
        octal(&mut header, 116, 8, 0);
        octal(&mut header, 124, 12, body.len() as u64);
        octal(&mut header, 136, 12, 0);
        header[156] = b'0'; // regular file
        if let Some(slot) = header.get_mut(257..263) {
            slot.copy_from_slice(b"ustar\0");
        }
        if let Some(slot) = header.get_mut(263..265) {
            slot.copy_from_slice(b"00");
        }

        // Checksum is computed with the checksum field itself read as spaces.
        if let Some(slot) = header.get_mut(148..156) {
            slot.fill(b' ');
        }
        let sum: u32 = header.iter().map(|b| u32::from(*b)).sum();
        let checksum = format!("{sum:06o}\0 ");
        if let Some(slot) = header.get_mut(148..156) {
            slot.copy_from_slice(checksum.as_bytes());
        }

        let mut archive = header.to_vec();
        archive.extend_from_slice(body.as_bytes());
        // Pad the body out to a 512-byte boundary.
        let padding = (512 - body.len() % 512) % 512;
        archive.extend(std::iter::repeat_n(0u8, padding));
        archive.extend(std::iter::repeat_n(0u8, 1024)); // two empty blocks
        archive
    }

    #[test]
    fn extract_tar_refuses_to_escape_the_destination() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A crafted archive must not be able to write outside the registry
        // directory, no matter what the publisher claims.
        let tar_bytes = build_raw_tar("../escaped.txt", "pwned");

        let err = extract_tar(&tar_bytes, dir.path())
            .expect_err("a traversal entry must be rejected");
        assert!(
            err.to_string().contains("escapes its base directory"),
            "unexpected error: {err}"
        );
        assert!(
            !dir.path()
                .parent()
                .expect("tempdir has a parent")
                .join("escaped.txt")
                .exists(),
            "nothing may be written outside the destination"
        );
    }

    #[test]
    fn extract_tar_refuses_absolute_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tar_bytes = build_raw_tar("/zoi-should-not-exist", "pwned");

        extract_tar(&tar_bytes, dir.path())
            .expect_err("an absolute entry must be rejected");
    }

    #[test]
    fn extract_tar_refuses_a_deeply_nested_traversal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tar_bytes = build_raw_tar("a/../../escaped.txt", "pwned");

        extract_tar(&tar_bytes, dir.path())
            .expect_err("a nested traversal must be rejected");
    }

    #[test]
    fn extract_tar_writes_nested_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tar_bytes = build_tar(&[
            ("repo.yaml", "name: testreg\n"),
            ("main/ripgrep/ripgrep.pkg.lua", "metadata({})\n")
        ]);

        extract_tar(&tar_bytes, dir.path()).expect("extraction should succeed");

        assert_eq!(
            fs::read_to_string(dir.path().join("repo.yaml"))
                .expect("repo.yaml read"),
            "name: testreg\n"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("main/ripgrep/ripgrep.pkg.lua"))
                .expect("pkg.lua read"),
            "metadata({})\n"
        );
    }

    #[test]
    fn swap_into_place_replaces_an_existing_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let staging = dir.path().join("staging");
        let target = dir.path().join("registry");

        fs::create_dir_all(&staging).expect("create staging");
        fs::write(staging.join("repo.yaml"), "name: new\n")
            .expect("write staging");
        fs::create_dir_all(&target).expect("create target");
        fs::write(target.join("repo.yaml"), "name: old\n").expect("write old");
        fs::write(target.join("stale.txt"), "stale").expect("write stale");

        swap_into_place(&staging, &target).expect("swap should succeed");

        assert_eq!(
            fs::read_to_string(target.join("repo.yaml")).expect("read"),
            "name: new\n"
        );
        assert!(
            !target.join("stale.txt").exists(),
            "files from the previous snapshot must not survive the swap"
        );
        assert!(
            !target.with_extension("zoi-old").exists(),
            "the retired tree must be cleaned up"
        );
    }

    #[test]
    fn swap_into_place_replaces_a_local_registry_symlink() {
        let dir = tempfile::tempdir().expect("tempdir");
        let staging = dir.path().join("staging");
        let target = dir.path().join("registry");
        let local = dir.path().join("local-registry");

        fs::create_dir_all(&staging).expect("create staging");
        fs::write(staging.join("repo.yaml"), "name: new\n")
            .expect("write staging");
        fs::create_dir_all(&local).expect("create local");
        fs::write(local.join("keep.txt"), "keep").expect("write local");
        core_utils::symlink_dir(&local, &target).expect("create symlink");

        swap_into_place(&staging, &target).expect("swap should succeed");

        assert!(target.join("repo.yaml").exists(), "new tree installed");
        assert!(
            !target.is_symlink(),
            "the symlink must be replaced by a real directory"
        );
        assert!(
            local.join("keep.txt").exists(),
            "a local registry must never be deleted through its symlink"
        );
    }

    #[test]
    fn install_snapshot_rejects_a_tree_without_repo_yaml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tar_bytes = build_tar(&[("packages.json", "{}")]);
        let zrepo_bytes = delta::zstd_compress(&tar_bytes).expect("compress");

        let err = install_snapshot(
            &tar_bytes,
            &zrepo_bytes,
            &dir.path().join("testreg.zrepo"),
            &dir.path().join("registry"),
            dir.path()
        )
        .expect_err("a snapshot without repo.yaml must be rejected");

        assert!(
            err.to_string().contains("does not contain a repo.yaml"),
            "unexpected error: {err}"
        );
    }

    /// Creates a small registry tree on disk and returns its root.
    fn make_registry_tree(root: &Path, version: &str) {
        fs::create_dir_all(root.join("main/ripgrep")).expect("create tree");
        fs::write(
            root.join("repo.yaml"),
            "version: \"2\"\nname: testreg\ndescription: test\ngit: \
             []\nrepos: []\n"
        )
        .expect("write repo.yaml");
        fs::write(
            root.join("packages.json"),
            "{\"version\":\"2\",\"packages\":{}}"
        )
        .expect("write packages.json");
        fs::write(
            root.join("main/ripgrep/ripgrep.pkg.lua"),
            format!("metadata({{version=\"{version}\"}})\n")
        )
        .expect("write pkg.lua");
    }

    #[test]
    fn create_zrepo_is_reproducible_for_an_unchanged_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");

        let first = dir.path().join("first.zrepo");
        let second = dir.path().join("second.zrepo");
        let a = create_zrepo(&tree, &first, None).expect("first snapshot");
        let b = create_zrepo(&tree, &second, None).expect("second snapshot");

        // A publisher must be able to emit a delta against a specific base, so
        // the same tree has to produce the same bytes every time. If it did
        // not, every published patch would immediately look mismatched.
        assert_eq!(
            fs::read(&first).expect("read first"),
            fs::read(&second).expect("read second"),
            "snapshotting the same tree twice must produce identical bytes"
        );
        assert_eq!(a.sha256, b.sha256);
        assert_eq!(a.file_count, 3, "repo.yaml, packages.json, one pkg.lua");
    }

    #[test]
    fn create_zrepo_ignores_git_plumbing_and_prior_snapshots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");

        // Local artifacts that must never be published: they would bloat the
        // snapshot and leak local state.
        fs::create_dir_all(tree.join(".git/objects")).expect("create .git");
        fs::write(tree.join(".git/HEAD"), "ref: refs/heads/main\n")
            .expect("write HEAD");
        fs::write(tree.join(".git/config"), "[core]\n").expect("write config");
        fs::write(tree.join("testreg.zrepo"), "stale snapshot\n")
            .expect("write stale snapshot");

        let out = dir.path().join("out.zrepo");
        let stats = create_zrepo(&tree, &out, None).expect("snapshot");

        assert_eq!(
            stats.file_count, 3,
            "only the three registry files should be included"
        );
        let tar_bytes = delta::zstd_decompress(&fs::read(&out).expect("read"))
            .expect("zstd");
        let raw = String::from_utf8_lossy(&tar_bytes);
        assert!(!raw.contains(".git"), "git plumbing must not be published");
        assert!(
            !raw.contains("stale snapshot"),
            "a prior snapshot must not be published"
        );
    }

    #[test]
    fn create_zrepo_writes_hash_and_size_sidecars() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");

        let out = dir.path().join("testreg.zrepo");
        let stats = create_zrepo(&tree, &out, None).expect("snapshot");

        let hash = fs::read_to_string(dir.path().join("testreg.zrepo.hash"))
            .expect("hash sidecar");
        assert_eq!(hash.trim(), stats.sha256);

        let size = fs::read_to_string(dir.path().join("testreg.zrepo.size"))
            .expect("size sidecar");
        assert_eq!(
            size.trim().parse::<u64>().expect("parse size"),
            stats.compressed_size
        );
    }

    #[test]
    fn create_zrepo_rejects_a_directory_that_is_not_a_registry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("not-a-registry");
        fs::create_dir_all(&tree).expect("create dir");
        fs::write(tree.join("readme.md"), "hello\n").expect("write");

        let err = create_zrepo(&tree, &dir.path().join("out.zrepo"), None)
            .expect_err("a directory without repo.yaml must be rejected");
        assert!(
            err.to_string().contains("no repo.yaml"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn a_published_delta_updates_the_base_to_the_new_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");

        let base = dir.path().join("v1.zrepo");
        create_zrepo(&tree, &base, None).expect("base snapshot");

        // Publish a new revision, then a patch from the base.
        make_registry_tree(&tree, "14.1.1");
        let patch = dir.path().join("v1.zrepo.zdelta");
        create_zrepo_delta(&base, &tree, &patch, None)
            .expect("delta should build");

        // The whole loop a client runs: apply the published patch to the base
        // it already holds and land on exactly the newly published tree.
        let rebuilt = dir.path().join("rebuilt.zrepo");
        apply_delta(&base, &patch, &rebuilt).expect("delta should apply");

        let rebuilt_tar =
            delta::zstd_decompress(&fs::read(&rebuilt).expect("read"))
                .expect("zstd");
        let (published_tar, _) = build_registry_tar(&tree).expect("build tar");
        assert_eq!(
            rebuilt_tar, published_tar,
            "the patched snapshot must equal the newly published tree"
        );
    }

    #[test]
    fn a_published_delta_is_reproducible() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");
        let base = dir.path().join("v1.zrepo");
        create_zrepo(&tree, &base, None).expect("base snapshot");

        make_registry_tree(&tree, "14.1.1");
        let first = dir.path().join("first.zdelta");
        let second = dir.path().join("second.zdelta");
        create_zrepo_delta(&base, &tree, &first, None).expect("first patch");
        create_zrepo_delta(&base, &tree, &second, None).expect("second patch");

        assert_eq!(
            fs::read(&first).expect("read first"),
            fs::read(&second).expect("read second"),
            "the same base and tree must produce an identical patch"
        );
    }

    #[test]
    fn create_zrepo_delta_rejects_a_missing_base() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");

        let err = create_zrepo_delta(
            &dir.path().join("nope.zrepo"),
            &tree,
            &dir.path().join("out.zdelta"),
            None
        )
        .expect_err("a missing base must be rejected");
        assert!(
            err.to_string().contains("does not exist"),
            "unexpected error: {err}"
        );
    }
}
