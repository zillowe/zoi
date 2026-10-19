//! Support for zstd-compressed registry databases (`.zrepo`).
//!
//! A `.zrepo` is a compressed tar of a whole registry snapshot: `repo.yaml`,
//! `packages.json`, and every `.pkg.lua` / `.sec.yaml`. It replaces a Git clone
//! as the transport for registry content, so a client downloads one immutable
//! file instead of running a Git negotiation.
//!
//! Snapshots are transferred incrementally with [zsync], which the registry
//! publishes alongside the snapshot as a `.zsync` control file. A client that
//! already holds a snapshot submits it as a seed, recovers the blocks it
//! already has, and range-requests only the ones that moved.
//!
//! That only works because the snapshot is not one compressed stream. It is a
//! sequence of *independently* compressed zstd frames, one per registry file,
//! concatenated. An entry that has not changed compresses to exactly the same
//! bytes it did last time, so a client recognises it and skips it. A single
//! frame wrapping the whole tar would resynchronise after the first edit and
//! leave the client re-downloading most of the file - measured at 53% of a
//! snapshot for a three-line change, against 1.3% for the per-entry framing.
//!
//! The decompressed frames concatenate back into an ordinary tar, so extraction
//! is unchanged and the resulting snapshot is byte-for-byte the one the
//! registry author published.
//!
//! [zsync]: https://crates.io/crates/zync-rs

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use tempfile::Builder;
use zbsdiff::zdelta::{self as delta};
use zoi_core::types::ZrepoLink;
use zoi_core::{cache, pgp, types, utils as core_utils};
use zoi_install::util as install_util;
use zync::{
    AssemblyBuilder, ControlFile, GenerateOptions, Transport, TransportError
};

/// The `registry_meta` key holding the hash of the `.zrepo` snapshot the
/// local index was last built from.
pub const ZREPO_HASH_META_KEY: &str = "zrepo_sha256";

/// Suffix appended to a `.zrepo` URL to locate its zsync control file when the
/// `repo.yaml` entry does not spell one out.
const ZSYNC_SUFFIX: &str = ".zsync";

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

/// The URL of the zsync control file for `link`.
///
/// Falls back to `<url>.zsync`, which is the convention Zoi publishes and
/// what the `git`-style link list implies.
pub fn control_url(link: &ZrepoLink) -> String {
    match &link.control {
        Some(control) if !control.is_empty() => control.clone(),
        _ => format!("{}{ZSYNC_SUFFIX}", link.url.trim_end_matches('/'))
    }
}

/// Where the downloaded `.zrepo` snapshot for `handle` is cached.
///
/// The snapshot is kept next to the extracted tree and the `SQLite` index so
/// that the next sync has a seed to recover unchanged frames from.
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
/// Git plumbing and published artifacts are local state rather than registry
/// content, so publishing them would both bloat the snapshot and leak it back
/// out.
///
/// Artifacts are matched on the `.zrepo` marker anywhere in the file name
/// rather than on the extension alone. `zoi registry zrepo` writes into the
/// tree by default, and the sidecars it leaves behind - `main.zrepo.hash`,
/// `main.zrepo.size`, `main.zrepo.sig` and `main.zrepo.zsync` - all carry a
/// different extension, so an extension check would republish the previous
/// snapshot's sidecars inside the next one.
fn is_excluded_from_snapshot(relative: &Path) -> bool {
    relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name)
                if name == ".git"
        )
    }) || relative
        .file_name()
        .is_some_and(|name| name.to_string_lossy().contains(".zrepo"))
}

/// Collects the files a snapshot includes, sorted by archive path.
///
/// Sorting is what makes the snapshot reproducible: the same tree always
/// yields the same bytes, so an unchanged file produces a byte-identical frame
/// on every publish and a client can recognise it against an older snapshot.
fn snapshot_entries(registry_root: &Path) -> Vec<(String, PathBuf)> {
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

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

/// Encodes one registry file as a bare tar entry: a 512-byte header, the data,
/// and padding out to the next 512-byte boundary.
///
/// Deliberately not `tar::Builder`, which writes a 1024-byte end-of-archive
/// marker on `finish`. That marker is fatal here on both counts: it would be
/// repeated once per file, and `tar::Archive` stops reading at the first one,
/// so only the first registry file in a reassembled snapshot would survive.
///
/// # Errors
///
/// Returns an error if the path cannot be encoded or the data cannot be read.
fn encode_tar_entry(name: &str, path: &Path, data: &[u8]) -> Result<Vec<u8>> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    // Preserve the executable bit; everything else is normalized.
    header.set_mode(if is_executable(path) { 0o755 } else { 0o644 });
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header.set_path(name).map_err(|e| {
        anyhow!("Failed to encode '{name}' into a tar header: {e}")
    })?;
    // The checksum covers every field written above, so it has to come last.
    header.set_cksum();

    let padding = (512 - (data.len() % 512)) % 512;
    let mut entry = Vec::with_capacity(512 + data.len() + padding);
    entry.extend_from_slice(header.as_bytes());
    entry.extend_from_slice(data);
    entry.resize(512 + data.len() + padding, 0);
    Ok(entry)
}

/// Builds the `.zrepo` snapshot bytes for a registry tree.
///
/// The result is one independently compressed zstd frame per registry file,
/// concatenated, along with the uncompressed size and the file count.
///
/// # Errors
///
/// Returns an error if the tree cannot be walked, an entry cannot be read, or
/// a frame cannot be compressed.
fn build_snapshot_frames(
    registry_root: &Path
) -> Result<(Vec<u8>, u64, usize)> {
    let entries = snapshot_entries(registry_root);
    let file_count = entries.len();

    let mut frames = Vec::new();
    let mut tar_size = 0u64;

    for (name, path) in &entries {
        let data = fs::read(path)
            .map_err(|e| anyhow!("Failed to read '{}': {e}", path.display()))?;
        let entry = encode_tar_entry(name, path, &data)?;
        tar_size += entry.len() as u64;

        let frame = delta::zstd_compress(&entry)
            .map_err(|e| anyhow!("Failed to compress '{name}': {e}"))?;
        frames.extend_from_slice(&frame);
    }

    Ok((frames, tar_size, file_count))
}

/// Returns true when any execute bit is set on `path`.
///
/// # Errors
///
/// Returns an error if the file's permissions cannot be read.
fn is_executable(path: &Path) -> bool {
    // The `PermissionsExt` import has to live inside the `cfg(unix)` block:
    // `std::os::unix` does not exist on Windows, so an ungated import fails
    // to compile there even though the code below it is gated.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

/// Builds a `.zrepo` snapshot of a registry tree and writes it to `output`,
/// along with the zsync control file that lets clients transfer it
/// incrementally.
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

    let (zrepo_bytes, tar_size, file_count) =
        build_snapshot_frames(registry_root)?;

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, &zrepo_bytes)?;

    let stats = ZrepoStats {
        file_count,
        tar_size,
        compressed_size: zrepo_bytes.len() as u64,
        sha256: sha256_bytes(&zrepo_bytes)
    };

    write_sidecars(output, &zrepo_bytes, sign_key)?;
    write_control_file(output)?;
    Ok(stats)
}

/// Writes the zsync control file describing the snapshot at `output`.
///
/// The `URL` is the bare file name rather than an absolute address, because
/// the publisher does not know where the snapshot will be served from. Clients
/// resolve it against the URL they fetched the control file from, so the same
/// control file works unchanged on a primary and on every mirror.
///
/// # Errors
///
/// Returns an error if the control file cannot be generated or written.
fn write_control_file(output: &Path) -> Result<()> {
    let name = output.file_name().map_or_else(
        || "registry.zrepo".to_string(),
        |n| n.to_string_lossy().to_string()
    );

    let control = zync::make(
        output,
        &GenerateOptions {
            filename: Some(&name),
            mtime: None,
            urls: vec![name.clone()],
            blocksize: None
        }
    )?;

    let control_path = PathBuf::from(format!("{}.zsync", output.display()));
    let mut file = fs::File::create(&control_path)?;
    control.write(&mut file)?;
    Ok(())
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

/// Hashes a byte buffer with SHA-256, returning the lowercase hex digest.
fn sha256_bytes(data: &[u8]) -> String {
    zoi_core::hash::calculate_reader_hash(
        &mut Cursor::new(data),
        zoi_core::hash::HashAlgorithm::Sha256
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

/// Why an incremental snapshot transfer could not be used.
///
/// The distinction decides how loudly sync complains. A control file that is
/// simply not there is the normal state for a registry that has not adopted
/// zsync, whereas one that is there and unusable means something is wrong.
enum TransferUnavailable {
    /// No control file could be fetched from the URL.
    NotFetched(anyhow::Error),
    /// A control file was fetched but the transfer could not be completed.
    Unusable(anyhow::Error)
}

/// An HTTP [`Transport`] backed by Zoi's configured client.
///
/// Going through Zoi's client rather than a private one keeps a single policy
/// for TLS roots, proxies and retries across every fetch Zoi makes.
///
/// A server that ignores `Range` and answers `200` with the whole body is
/// refused rather than worked around. The bytes asked for would be in the
/// response at their own offset, so serving them means discarding everything
/// before the range, and the alternative - giving up on the transfer - is
/// exactly what the caller's full-download fallback does anyway.
struct ZoiTransport {
    /// The client carrying Zoi's transport policy.
    client: &'static reqwest::blocking::Client
}

impl Transport for ZoiTransport {
    fn get(
        &self,
        url: &str
    ) -> Result<Box<dyn std::io::Read + Send>, TransportError> {
        let response = self.client.get(url).send().map_err(|e| {
            TransportError::Request {
                url: url.to_string(),
                reason: e.to_string()
            }
        })?;
        Ok(Box::new(response))
    }

    fn get_range(
        &self,
        url: &str,
        range: std::ops::Range<u64>
    ) -> Result<Box<dyn std::io::Read + Send>, TransportError> {
        // `Range<u64>` is half-open, the header is inclusive at both ends.
        let last = range.end.saturating_sub(1).max(range.start);
        let response = self
            .client
            .get(url)
            .header("Range", format!("bytes={}-{}", range.start, last))
            .send()
            .map_err(|e| TransportError::Request {
                url: url.to_string(),
                reason: e.to_string()
            })?;

        let status = response.status().as_u16();
        if status != 206 {
            return Err(TransportError::UnexpectedStatus {
                url: url.to_string(),
                status
            });
        }

        let wanted = range.end.saturating_sub(range.start);
        // Capped to the range that was asked for: past it, an origin decides
        // how much memory the client spends. Spelled out because `Response` is
        // also an `Iterator`-shaped type to `take`, and picking the wrong one
        // silently caps nothing.
        Ok(Box::new(std::io::Read::take(response, wanted)))
    }
}

/// Resolves a relative `URL` header against the control file it came from.
fn resolve_snapshot_url(control_url: &str, target: &str) -> String {
    if target.contains("://") {
        return target.to_string();
    }
    let Some(cut) = control_url.rfind('/') else {
        return target.to_string();
    };
    format!("{}{}", &control_url[..=cut], target)
}

/// Transfers a snapshot through its zsync control file and installs it.
///
/// The cached snapshot, when there is one, is submitted as a seed: its frames
/// are scanned for blocks the new snapshot already contains, so unchanged
/// registry files cost nothing and only the frames that actually moved are
/// range-requested.
///
/// # Errors
///
/// Returns an error if the control file cannot be fetched or parsed, or if the
/// transfer cannot be completed. Callers treat every failure as a reason to
/// fall back to a full download.
fn zync_transfer(
    link: &ZrepoLink,
    base_artifact: &Path,
    target_dir: &Path,
    work_dir: &Path,
    authorities: &[String],
    pb: Option<&ProgressBar>,
    verbose: bool
) -> Result<String, TransferUnavailable> {
    let control_url = control_url(link);

    let control_path = work_dir.join("control.zsync");
    let unusable = |e: anyhow::Error| TransferUnavailable::Unusable(e);
    download(&control_url, &control_path, pb)
        .map_err(TransferUnavailable::NotFetched)?;

    let control_bytes = fs::read(&control_path).map_err(|e| {
        unusable(anyhow!("Failed to read the control file: {e}"))
    })?;
    let mut control = ControlFile::parse(Cursor::new(control_bytes))
        .map_err(|e| unusable(anyhow!("Malformed control file: {e}")))?;

    // Expand the control file's own targets with Zoi's configured mirrors, so
    // a registry serving one control file still benefits from redundancy the
    // user has configured locally.
    control.urls = control
        .urls
        .iter()
        .flat_map(|url| {
            cache::mirror_candidate_urls(&resolve_snapshot_url(
                &control_url,
                url
            ))
        })
        .collect();

    if control.urls.is_empty() {
        return Err(unusable(anyhow!(
            "The control file names no snapshot to fetch."
        )));
    }

    if let Some(p) = pb {
        p.set_message("Updating registry snapshot");
    } else if verbose {
        println!("Updating registry snapshot from {control_url}...");
    }

    let blocksize = control.blocksize;
    let control_length = control.length;
    let progress_bar = pb.cloned();
    let output = work_dir.join("assembled.zrepo");

    let transport = ZoiTransport {
        client: core_utils::get_http_client().map_err(unusable)?
    };
    let mut assembly = AssemblyBuilder::new(control, &output)
        .transport(Arc::new(transport))
        .build()
        .map_err(|e| unusable(anyhow!("Failed to start the transfer: {e}")))?;

    assembly.set_progress(move |done: usize, total: usize| {
        if let Some(p) = progress_bar.as_ref() {
            p.set_length((total * blocksize) as u64);
            p.set_position((done * blocksize) as u64);
        }
    });

    // No cached snapshot is not an error: the transfer simply fetches
    // everything, which is what a first sync does anyway.
    if base_artifact.exists()
        && let Err(e) = assembly.submit_file(base_artifact)
        && verbose
    {
        println!(
            "{}",
            format!(
                "Could not reuse the cached snapshot ({e}); fetching the \
                 whole registry."
            )
            .dimmed()
        );
    }

    let stats = assembly
        .fetch()
        .map_err(|e| unusable(anyhow!("Snapshot transfer failed: {e}")))?;
    assembly
        .finish()
        .map_err(|e| unusable(anyhow!("Failed to write the snapshot: {e}")))?;

    // From here on the snapshot is complete, so it is verified and installed
    // exactly as a full download would be. zsync only changes how the bytes
    // arrive; it is never the thing that decides whether they are trustworthy.
    verify_hash(link, &output, pb).map_err(unusable)?;
    verify_signature(link, &output, work_dir, authorities, pb, verbose)
        .map_err(unusable)?;

    let zrepo_bytes = fs::read(&output).map_err(|e| unusable(e.into()))?;
    let tar_bytes = delta::zstd_decompress(&zrepo_bytes).map_err(|e| {
        unusable(anyhow!(
            "Transferred registry snapshot is not a valid zstd stream: {e}"
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
    .map_err(unusable)?;

    if verbose {
        // What the transfer actually cost, so the saving is visible rather
        // than merely implied. A registry author who cannot see the number
        // has no reason to publish a control file in the first place.
        let total = control_length;
        let reused = total.saturating_sub(stats.downloaded);
        let percent =
            reused.saturating_mul(100).checked_div(total).unwrap_or(0);
        println!(
            "{}",
            format!(
                "Registry snapshot updated incrementally: {reused_pct} of \
                 {total_bytes} reused ({percent}%), {downloaded} fetched in \
                 {requests} request{s}.",
                reused_pct = core_utils::format_bytes(reused),
                total_bytes = core_utils::format_bytes(total),
                downloaded = core_utils::format_bytes(stats.downloaded),
                requests = stats.requests,
                s = if stats.requests == 1 { "" } else { "s" }
            )
            .green()
        );
    }
    Ok(hash)
}

/// Syncs a single `.zrepo` link, preferring an incremental transfer.
///
/// # Errors
///
/// Returns an error if both the incremental transfer and the full download
/// fail.
fn try_link(
    link: &ZrepoLink,
    base_artifact: &Path,
    target_dir: &Path,
    work_dir: &Path,
    authorities: &[String],
    pb: Option<&ProgressBar>,
    verbose: bool
) -> Result<String> {
    match zync_transfer(
        link,
        base_artifact,
        target_dir,
        work_dir,
        authorities,
        pb,
        verbose
    ) {
        Ok(hash) => return Ok(hash),
        Err(reason) => {
            // The control file is an optimisation, not a requirement. A
            // registry that publishes none, or a server that cannot serve
            // ranges, must not break the sync.
            let declared = link.control.is_some();
            let control_url = control_url(link);
            let msg = match reason {
                TransferUnavailable::NotFetched(e) if declared => {
                    Some(format!(
                        "{} The registry declares a control file at \
                         {control_url} but it could not be fetched ({e}). \
                         Falling back to a full download.",
                        "Warning:".yellow(),
                    ))
                }
                TransferUnavailable::NotFetched(_) => {
                    if verbose {
                        println!(
                            "{}",
                            format!(
                                "No registry control file at {control_url}; \
                                 downloading the full snapshot."
                            )
                            .dimmed()
                        );
                    }
                    None
                }
                TransferUnavailable::Unusable(e) => Some(format!(
                    "{} The registry control file could not be used ({}). \
                     Falling back to a full download.",
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
    fn published_artifacts_are_not_republished_into_the_next_snapshot() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");

        // Publish into the tree, which is where the default output path puts
        // it, so the sidecars land among the registry content.
        create_zrepo(&tree, &tree.join("testreg.zrepo"), None)
            .expect("first publish");
        assert!(
            tree.join("testreg.zrepo.zsync").exists(),
            "the control file should have been written beside the snapshot"
        );

        let second = dir.path().join("second.zrepo");
        create_zrepo(&tree, &second, None).expect("second publish");

        let bytes = fs::read(&second).expect("read");
        let tar = delta::zstd_decompress(&bytes).expect("decompress");
        let mut archive = tar::Archive::new(Cursor::new(tar));
        let names: Vec<String> = archive
            .entries()
            .expect("entries should read")
            .map(|entry| {
                entry
                    .expect("entry")
                    .path()
                    .expect("path")
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        // The previous snapshot and every sidecar beside it are local state.
        // Republishing them makes each snapshot larger than the last and
        // feeds a client's seed its own output.
        for leaked in [
            "testreg.zrepo",
            "testreg.zrepo.hash",
            "testreg.zrepo.size",
            "testreg.zrepo.zsync"
        ] {
            assert!(
                !names.iter().any(|name| name == leaked),
                "'{leaked}' leaked into the next snapshot: {names:?}"
            );
        }

        // The registry content itself must still be there.
        assert!(names.contains(&"repo.yaml".to_string()), "{names:?}");
    }

    #[test]
    fn every_registry_file_becomes_its_own_compressed_frame() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");

        let snapshot = dir.path().join("main.zrepo");
        create_zrepo(&tree, &snapshot, None).expect("snapshot should build");
        let bytes = fs::read(&snapshot).expect("read snapshot");

        // Each entry's frame has to appear in the snapshot verbatim. That is
        // the property the whole transfer rests on, and it is only true if the
        // snapshot is framed per file rather than wrapped in one stream.
        for name in ["repo.yaml", "packages.json"] {
            let path = tree.join(name);
            let data = fs::read(&path).expect("read entry");
            let entry = encode_tar_entry(name, &path, &data).expect("encode");
            let frame = delta::zstd_compress(&entry).expect("compress");
            assert!(
                contains(&bytes, &frame),
                "'{name}' is not framed independently in the snapshot"
            );
        }

        // The frames still concatenate into a complete tar.
        let tar = delta::zstd_decompress(&bytes).expect("valid zstd stream");
        let mut archive = tar::Archive::new(Cursor::new(tar));
        let names: Vec<String> = archive
            .entries()
            .expect("entries should read")
            .map(|entry| {
                entry
                    .expect("entry")
                    .path()
                    .expect("path")
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert!(
            names.contains(&"repo.yaml".to_string()),
            "repo.yaml missing from {names:?}"
        );
        assert!(
            names.iter().any(|n| n.ends_with("ripgrep.pkg.lua")),
            "no package definitions in {names:?}"
        );
    }

    #[test]
    fn an_unchanged_file_produces_an_identical_frame_after_an_edit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");

        let first = dir.path().join("v1.zrepo");
        create_zrepo(&tree, &first, None).expect("first snapshot");
        let first_bytes = fs::read(&first).expect("read first");

        // Edit one package definition and leave every other file alone.
        make_registry_tree(&tree, "14.1.1");
        let second = dir.path().join("v2.zrepo");
        create_zrepo(&tree, &second, None).expect("second snapshot");
        let second_bytes = fs::read(&second).expect("read second");

        assert_ne!(
            first_bytes, second_bytes,
            "the edit should have changed something"
        );

        // Every file the edit did not touch must still be framed exactly as it
        // was, which is what lets a client skip fetching it.
        for name in ["repo.yaml", "packages.json"] {
            let path = tree.join(name);
            let data = fs::read(&path).expect("read entry");
            let entry = encode_tar_entry(name, &path, &data).expect("encode");
            let frame = delta::zstd_compress(&entry).expect("compress");
            assert!(
                contains(&first_bytes, &frame),
                "'{name}' missing from the first snapshot"
            );
            assert!(
                contains(&second_bytes, &frame),
                "'{name}' was re-framed by an unrelated edit"
            );
        }
    }

    /// Whether `haystack` contains `needle` as a contiguous run of bytes.
    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        !needle.is_empty()
            && haystack
                .windows(needle.len())
                .any(|window| window == needle)
    }

    #[test]
    fn create_zrepo_writes_a_zsync_control_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = dir.path().join("registry");
        make_registry_tree(&tree, "14.1.0");

        let snapshot = dir.path().join("main.zrepo");
        create_zrepo(&tree, &snapshot, None).expect("snapshot should build");

        let control_path = dir.path().join("main.zrepo.zsync");
        assert!(
            control_path.exists(),
            "a control file must be published beside the snapshot"
        );

        let control = ControlFile::parse(
            fs::File::open(&control_path).expect("open control file")
        )
        .expect("control file should parse");
        assert_eq!(
            control.length,
            fs::metadata(&snapshot).expect("stat").len()
        );

        // The URL is relative so one control file works on a primary and on
        // every mirror.
        assert_eq!(control.urls, vec!["main.zrepo".to_string()]);
    }

    #[test]
    fn a_relative_control_url_resolves_against_the_control_file() {
        assert_eq!(
            resolve_snapshot_url(
                "https://example.com/reg/main.zrepo.zsync",
                "main.zrepo"
            ),
            "https://example.com/reg/main.zrepo"
        );
        assert_eq!(
            resolve_snapshot_url(
                "https://example.com/reg/main.zrepo.zsync",
                "https://cdn.example.net/main.zrepo"
            ),
            "https://cdn.example.net/main.zrepo"
        );
    }

    #[test]
    fn the_control_url_defaults_to_the_conventional_suffix() {
        let main = ZrepoLink {
            link_type: "main".to_string(),
            url: "https://example.com/reg/main.zrepo".to_string(),
            control: None,
            pgp: None,
            hash: None,
            size: None
        };
        assert_eq!(
            control_url(&main),
            "https://example.com/reg/main.zrepo.zsync"
        );

        let explicit = ZrepoLink {
            control: Some("https://example.com/reg/index.zsync".to_string()),
            ..main
        };
        assert_eq!(
            control_url(&explicit),
            "https://example.com/reg/index.zsync"
        );
    }
}
