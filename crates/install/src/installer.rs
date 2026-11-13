//! Main installer logic.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use zoi_core::{cache, config, pgp, pkgdir, recorder, types, utils};
use zoi_db as db;
use zoi_hooks as hooks;
use zoi_resolver::local;

use crate::resolver::InstallNode;
use crate::{manifest, plan, prebuilt, util};

/// Downloads and caches a package archive.
///
/// This function handles:
/// - Checking pkg-dirs and the archive cache.
/// - Downloading from mirrors if not found locally.
/// - Verifying hashes and PGP signatures.
///
/// When `force_redownload` is set, any cached archive, signature, or
/// partial download for this package is discarded and the archive is
/// fetched fresh, skipping the delta-upgrade fast-path as well.
///
/// # Errors
///
/// Returns an error if:
/// - The configuration cannot be read.
/// - The archive cache directory cannot be created.
/// - Zoi is offline and the archive is missing.
/// - The download fails.
/// - Hash verification fails.
/// - Signature verification fails.
pub fn download_and_cache_archive(
    node: &InstallNode,
    details: &plan::PrebuiltDetails,
    pb: Option<&ProgressBar>,
    verbose: bool,
    force_redownload: bool
) -> Result<PathBuf> {
    let config = config::read_config()?;
    let signature_policy = config
        .policy
        .signature_enforcement
        .as_ref()
        .filter(|p| p.enable);

    let archive_cache_root = cache::get_archive_cache_root()?;
    fs::create_dir_all(&archive_cache_root)?;

    // `final_url` comes from registry metadata, so it is untrusted input. The
    // helper only returns a value that is a single inert path component;
    // anything else (empty, or carrying separators/dot segments) falls back to
    // a fixed name rather than being joined onto the cache root.
    //
    // The fallback is unique per URL. A fixed name made every package whose
    // URL was refused share one cached file: the first download was then
    // reused for all of them and failed their hash checks.
    let fallback_filename = format!(
        "archive-{}.zpa",
        zoi_core::hash::calculate_string_hash(
            &details.info.final_url,
            zoi_core::hash::HashAlgorithm::Sha256
        )
        .get(..16)
        .unwrap_or_default()
    );
    let archive_filename = util::get_filename_from_url(&details.info.final_url)
        .unwrap_or(&fallback_filename);

    // Defence in depth: assert the invariant the helper promises, so a future
    // change to that helper cannot silently reintroduce a writable path.
    let cached_archive_path =
        utils::safe_join(&archive_cache_root, Path::new(archive_filename))?;
    let sig_filename = format!("{archive_filename}.sig");
    let cached_sig_path =
        utils::safe_join(&archive_cache_root, Path::new(&sig_filename))?;

    let has_authorities = registry_authorities(&config, &node.registry_handle)
        .is_some_and(|a| !a.is_empty());
    let pgp_identifiers = trusted_identifiers(&config, &node.registry_handle);
    let has_pgp_identifiers = pgp_identifiers
        .as_ref()
        .is_some_and(|keys| !keys.is_empty());
    let is_source_bundle =
        matches!(archive_filename.rsplit('.').next(), Some("zsa"));

    // Track whether the archive was rebuilt from a delta so we can skip
    // the whole-archive checksum (per-pool-entry integrity was already
    // verified during delta application).
    let mut delta_applied = false;

    if force_redownload {
        // Discard anything cached so the archive below is fetched fresh.
        // A stale cached archive is exactly what `--force` must not reuse.
        fs::remove_file(&cached_archive_path).ok();
        fs::remove_file(&cached_sig_path).ok();
        fs::remove_file(
            archive_cache_root.join(format!("{archive_filename}.part"))
        )
        .ok();
        if pb.is_none() {
            println!(
                "Re-downloading archive (--force): {}",
                archive_filename.cyan()
            );
        }
    }

    let archive_path = if let Some(path) =
        pkgdir::find_in_pkg_dirs(archive_filename)
    {
        if pb.is_none() {
            println!("Found archive in pkg-dir: {}", path.display());
        }
        path
    } else if !force_redownload && cached_archive_path.exists() {
        if pb.is_none() {
            println!("Using cached archive: {}", cached_archive_path.display());
        }
        cached_archive_path.clone()
    } else {
        if zoi_core::offline::is_offline() {
            return Err(anyhow!(
                "Archive not found in cache and cannot download: Zoi is in \
                 offline mode. Missing: {archive_filename}"
            ));
        }

        // Delta fast-path: try to fetch and apply `.zdelta` patches
        // sequentially from the cached version up to the target version.
        // Any failure here (including a missing or invalid delta signature,
        // or a rebuilt archive whose embedded signature does not verify)
        // falls back to the full download below. The rebuilt archive is
        // written to `cached_archive_path` and goes through the embedded
        // signature check below like any download. Note: the whole-archive
        // checksum is skipped for delta-rebuilt archives because the
        // tar+zstd byte stream differs from the original while per-pool-entry
        // integrity is already verified during delta application. Skipped
        // entirely on forced re-downloads, which must fetch fresh bytes, and
        // for source bundles, since deltas only exist for pooled `.zpa`s.
        delta_applied = if force_redownload || is_source_bundle {
            false
        } else {
            match try_delta_upgrade_sequence(
                node,
                &archive_cache_root,
                &cached_archive_path,
                pb,
                verbose,
                pgp_identifiers.as_deref(),
                config.max_delta_steps
            ) {
                Ok(applied) => applied,
                Err(e) => {
                    if verbose {
                        println!(
                            "{} {e}; falling back to full download.",
                            "Warning:".yellow()
                        );
                    }
                    false
                }
            }
        };

        // If the delta succeeded, `cached_archive_path` now exists and
        // the normal flow will pick it up. Otherwise continue to download.
        if delta_applied {
            cached_archive_path.clone()
        } else {
            let part_path =
                archive_cache_root.join(format!("{archive_filename}.part"));

            if part_path.exists() && pb.is_none() {
                println!("Resuming partial download: {}", part_path.display());
            }

            let mut last_error = None;
            let candidate_urls =
                cache::mirror_candidate_urls(&details.info.final_url);
            let mut downloaded = false;
            for candidate_url in candidate_urls {
                match util::download_file_with_progress(
                    &candidate_url,
                    &part_path,
                    pb,
                    Some(details.download_size)
                ) {
                    Ok(()) => {
                        downloaded = true;
                        break;
                    }
                    Err(e) => last_error = Some((candidate_url, e))
                }
            }
            if !downloaded {
                let (url, error) = last_error.ok_or_else(|| {
                    anyhow!("archive download failed but no error recorded")
                })?;
                return Err(anyhow!(
                    "Failed to download package archive from {url}: {error}"
                ));
            }

            fs::rename(&part_path, &cached_archive_path)?;
            cached_archive_path.clone()
        }
    };

    // Delta-rebuilt archives already passed per-pool-entry structural
    // verification inside `apply_zpa_delta`, and their tar+zstd byte
    // stream will differ from the original, so skip the whole-archive
    // checksum for them.
    if !delta_applied && let Some(hash_url) = &details.info.hash_url {
        let hash = db::get_package_hash_from_db(
            &node.registry_handle,
            &node.pkg.name,
            node.sub_package.as_deref(),
            &node.pkg.repo
        )
        .unwrap_or(None)
        .filter(|hash| !hash.is_empty());
        let hash = match hash {
            Some(hash) => hash,
            None => util::get_expected_hash(hash_url, Some(archive_filename))
                .map_err(|error| {
                    anyhow!(
                        "Unable to obtain the required checksum for '{}': \
                         {error}",
                        node.pkg.name
                    )
                })?
        };

        if !util::verify_file_hash(&archive_path, &hash, pb)? {
            return Err(anyhow!("Hash verification failed"));
        }
    }

    validate_signature_requirements(
        signature_policy.is_some(),
        details.info.pgp_url.is_some(),
        has_pgp_identifiers
    )?;

    // An embedded signature travels inside the archive itself, so it is
    // preferred over a detached sidecar: it works even when the archive is
    // distributed out-of-band without its `.sig`. Pooled `.zpa` archives
    // carry a `manifest.sig` (which, through the manifest's `.pkg.lua`
    // digest, also covers the bundled definition); `.zsa` source bundles
    // carry a `<name>.pkg.lua.sig`. This runs for delta-rebuilt archives
    // too: the delta container carries the manifest and its signature
    // verbatim.
    let mut embedded_verified = false;
    if has_pgp_identifiers && let Some(ref identifiers) = pgp_identifiers {
        let trusted_certs = pgp::get_certs_by_name_or_fingerprint(identifiers)?;
        let result = if is_source_bundle {
            crate::archive_trust::verify_embedded_bundle_signature(
                &archive_path,
                &trusted_certs
            )
            .map(|verified| verified.is_some())
        } else {
            try_verify_embedded_signature(&archive_path, trusted_certs)
        };
        match result {
            Ok(true) => {
                if verbose {
                    println!(
                        "{}",
                        "Embedded signature verified successfully.".green()
                    );
                }
                embedded_verified = true;
            }
            Ok(false) => {}
            Err(e) => {
                if delta_applied {
                    // Never leave an unverifiable rebuild in the cache, where
                    // the next run would pick it up as a cached download.
                    fs::remove_file(&cached_archive_path).ok();
                }
                return Err(anyhow!(
                    "Embedded signature verification failed: {e}"
                ));
            }
        }
    }

    if delta_applied {
        // Detached `.sig` sidecars sign the publisher's original bytes, which
        // the rebuilt tar+zstd stream differs from by design, so for a
        // delta-rebuilt archive only the embedded signature can vouch for
        // it. `try_delta_upgrade_sequence` already refuses rebuilds that
        // lack one when keys are configured; this keeps that invariant
        // local to the check.
        if has_pgp_identifiers && !embedded_verified {
            fs::remove_file(&cached_archive_path).ok();
            return Err(anyhow!(
                "Delta-rebuilt archive for '{}' carries no verifiable \
                 embedded signature; retry to download the full archive.",
                node.pkg.name
            ));
        }
        if !has_pgp_identifiers && verbose {
            println!(
                "{}",
                "No trusted keys configured; delta-rebuilt archive is trusted \
                 through the delta and pool hashes only."
                    .dimmed()
            );
        }
    } else if !embedded_verified
        && let Some(pgp_url) = &details.info.pgp_url
        && let Some(ref identifiers) = pgp_identifiers
        && !identifiers.is_empty()
    {
        let sig_path = if cached_sig_path.exists() {
            cached_sig_path.clone()
        } else {
            if zoi_core::offline::is_offline() {
                return Err(anyhow!(
                    "Signature not found in cache and cannot download: Zoi is \
                     in offline mode."
                ));
            }
            let temp_dir =
                tempfile::Builder::new().prefix("zoi-sig-dl-").tempdir()?;
            let temp_sig_path = temp_dir.path().join(&sig_filename);
            let mut last_error = None;
            let mut downloaded = false;
            for candidate_url in cache::mirror_candidate_urls(pgp_url) {
                match util::download_file_with_progress(
                    &candidate_url,
                    &temp_sig_path,
                    pb,
                    None
                ) {
                    Ok(()) => {
                        downloaded = true;
                        break;
                    }
                    Err(e) => last_error = Some((candidate_url, e))
                }
            }
            if !downloaded {
                let (url, error) = last_error.ok_or_else(|| {
                    anyhow!("signature download failed but no error recorded")
                })?;
                return Err(anyhow!(
                    "Failed to download signature from {url}: {error}"
                ));
            }
            fs::copy(&temp_sig_path, &cached_sig_path)?;
            cached_sig_path.clone()
        };

        if verbose {
            println!("Verifying signature...");
        }
        let trusted_certs = pgp::get_certs_by_name_or_fingerprint(identifiers)?;
        pgp::verify_detached_signature_multi_key(
            &archive_path,
            &sig_path,
            trusted_certs
        )?;
        if verbose {
            println!("{}", "Signature verified successfully.".green());
        }
    } else if !embedded_verified
        && details.info.pgp_url.is_none()
        && has_authorities
    {
        let msg = format!(
            "Warning: Installing unsigned package '{}' from a registry that \
             claims to be secure.",
            node.pkg.name
        );
        if let Some(p) = pb {
            p.println(msg.yellow().to_string());
        } else {
            println!("{}", msg.yellow());
        }
        if signature_policy.is_some() {
            return Err(anyhow!(
                "Signature enforcement is active, but no PGP URL found for \
                 package"
            ));
        }
    }

    Ok(archive_path)
}

/// Downloads and applies a single `.zdelta` patch upgrading `base_path`
/// (`base_version`) to `to_version`, writing the rebuilt archive into
/// `work_dir`.
///
/// Uses explicit delta URLs from repo.yaml (delta section). The `{version}`
/// placeholder in delta URLs refers to the version being upgraded to.
/// Verifies the delta patch file's hash and signature before applying it.
///
/// # Errors
///
/// Returns an error when no delta is configured, the delta cannot be
/// downloaded, its hash or signature fails verification, or the patch
/// cannot be applied. The caller treats any error as "delta unavailable"
/// and falls back to a full download.
fn fetch_and_apply_delta(
    node: &InstallNode,
    base_path: &Path,
    base_version: &str,
    to_version: &str,
    work_dir: &Path,
    pb: Option<&ProgressBar>,
    verbose: bool,
    pgp_identifiers: Option<&[String]>
) -> Result<PathBuf> {
    // Look up explicit delta info from repo.yaml
    // The delta URL is keyed by the version this step upgrades to, which for
    // a multi-step chain is an intermediate version, not the node's target.
    let Some(delta_info) =
        crate::util::find_delta_info_for_node(node, base_version, to_version)?
    else {
        return Err(anyhow!(
            "No delta configuration found for '{}' v{to_version}",
            node.pkg.name
        ));
    };

    if verbose {
        println!(
            "Attempting delta upgrade from v{base_version} to v{to_version}..."
        );
    }

    let delta_path = work_dir.join(format!("upgrade-{to_version}.zdelta"));
    let mut downloaded = false;
    for candidate_url in cache::mirror_candidate_urls(&delta_info.final_url) {
        if util::download_file_with_progress(
            &candidate_url,
            &delta_path,
            pb,
            None
        )
        .is_ok()
        {
            downloaded = true;
            break;
        }
    }
    if !downloaded {
        return Err(anyhow!(
            "No delta available for '{}' v{base_version} -> v{to_version}",
            node.pkg.name
        ));
    }

    // Verify delta patch hash - this is mandatory for integrity.
    let expected_delta_hash =
        delta_info.hash_url.as_ref().ok_or_else(|| {
            anyhow!(
                "Delta configuration missing required hash URL for package \
                 '{}'",
                node.pkg.name
            )
        })?;
    let expected = util::get_expected_hash(expected_delta_hash, None)?;
    if !util::verify_file_hash(&delta_path, &expected, pb)? {
        return Err(anyhow!(
            "Delta hash verification failed for '{}' v{to_version}",
            node.pkg.name
        ));
    }

    // Verify the delta patch signature whenever trusted keys are configured,
    // exactly as a full archive download would be. A missing signature is an
    // error rather than a skip: otherwise stripping the `.sig` from a mirror
    // would turn signed upgrades into hash-only ones. The caller falls back
    // to a full (fully verified) download on any error.
    if let Some(identifiers) = pgp_identifiers
        && !identifiers.is_empty()
    {
        let pgp_url = delta_info.pgp_url.as_ref().ok_or_else(|| {
            anyhow!(
                "Trusted keys are configured but the delta for '{}' \
                 v{to_version} has no signature URL",
                node.pkg.name
            )
        })?;
        let temp_sig_path = work_dir.join(format!("upgrade-{to_version}.sig"));
        let mut downloaded = false;
        for candidate_url in cache::mirror_candidate_urls(pgp_url) {
            if util::download_file_with_progress(
                &candidate_url,
                &temp_sig_path,
                pb,
                None
            )
            .is_ok()
            {
                downloaded = true;
                break;
            }
        }
        if !downloaded {
            return Err(anyhow!(
                "Delta signature for '{}' v{to_version} could not be \
                 downloaded",
                node.pkg.name
            ));
        }
        let trusted_certs = pgp::get_certs_by_name_or_fingerprint(identifiers)?;
        pgp::verify_detached_signature_multi_key(
            &delta_path,
            &temp_sig_path,
            trusted_certs
        )?;
        if verbose {
            println!("{}", "Delta signature verified successfully.".green());
        }
    } else if verbose {
        println!(
            "{} No trusted PGP keys for delta signature verification.",
            "Warning:".yellow()
        );
    }

    let rebuilt_path = work_dir.join(format!("rebuilt-{to_version}.zpa"));
    zoi_package::delta::apply_zpa_delta(base_path, &delta_path, &rebuilt_path)?;
    if verbose {
        println!("{}", "Delta applied successfully.".green());
    }
    Ok(rebuilt_path)
}

/// Attempts to upgrade a package archive via a sequence of `.zdelta` patches
/// from a cached older version up to the target version.
///
/// When the cached version is more than one release behind the target (e.g.
/// cached `149`, target `151`), each intermediate delta is tried in order:
/// `149 -> 150`, then `150 -> 151`, rebuilding the archive step by step on
/// top of the previously rebuilt output. Intermediate versions are read
/// from the package database. The number of steps is limited by
/// `max_delta_steps` from the user config (default `3`, since Zoidberg only
/// keeps the last three versions as `.zpa`/`.zsa` and `.zdelta`); anything
/// beyond that falls back to a full download.
///
/// Returns `Ok(false)` whenever a delta is unavailable or cannot be applied
/// so the caller can fall back to a full download.
fn try_delta_upgrade_sequence(
    node: &InstallNode,
    archive_cache_root: &Path,
    target_path: &Path,
    pb: Option<&ProgressBar>,
    verbose: bool,
    pgp_identifiers: Option<&[String]>,
    max_delta_steps: u32
) -> Result<bool> {
    let Ok(target_version) = semver::Version::parse(&node.version) else {
        return Ok(false);
    };

    // Find the newest cached archive of the same package on the same
    // platform that is older than the target version. Archive names follow
    // `{name}-{version}-{platform}.zpa` or
    // `{name}-{version}-{platform}.{sub}.zpa`.
    let platform = zoi_core::utils::get_platform().unwrap_or_default();
    let prefix = format!("{}-", node.pkg.name);
    let sub_opt = node
        .sub_package
        .as_deref()
        .or(node.pkg.sub_package.as_deref())
        .or_else(|| {
            if node
                .pkg
                .sub_packages
                .as_ref()
                .is_some_and(|subs| !subs.is_empty())
            {
                node.pkg
                    .main_subs
                    .as_ref()
                    .and_then(|ms| ms.first().map(String::as_str))
                    .or(Some("main"))
            } else {
                None
            }
        });
    let suffix = if let Some(sub) = sub_opt {
        format!("-{platform}.{sub}.zpa")
    } else {
        format!("-{platform}.zpa")
    };
    let mut best: Option<(semver::Version, PathBuf)> = None;
    let entries = std::fs::read_dir(archive_cache_root).ok();
    let Some(entries) = entries else {
        return Ok(false);
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // The length check matters: for a name like `foo-linux-amd64.zpa`
        // the prefix and suffix overlap, and slicing between them would
        // panic.
        if name.len() < prefix.len() + suffix.len()
            || !name.starts_with(&prefix)
            || !name.ends_with(&suffix)
        {
            continue;
        }
        let Some(middle) = name.get(prefix.len()..name.len() - suffix.len())
        else {
            continue;
        };
        let version_str = middle.trim_start_matches('v');
        let Ok(version) = semver::Version::parse(version_str) else {
            continue;
        };
        if version >= target_version {
            continue;
        }
        if best.as_ref().is_none_or(|(v, _)| version > *v) {
            best = Some((version, entry.path()));
        }
    }
    let Some((base_version, base_path)) = best else {
        return Ok(false);
    };

    // Determine the intermediate versions between the cached base and the
    // target from the package database, e.g. base `149` and target `151`
    // give steps [`150`, `151`].
    let mut steps: Vec<(semver::Version, String)> = Vec::new();
    if let Ok(db_versions) = db::get_all_versions(
        &node.registry_handle,
        &node.pkg.name,
        &node.pkg.repo
    ) {
        for version_string in db_versions {
            let parsed =
                semver::Version::parse(&version_string).or_else(|_| {
                    semver::Version::parse(
                        version_string.trim_start_matches('v')
                    )
                });
            let Ok(version) = parsed else {
                continue;
            };
            if version > base_version && version <= target_version {
                steps.push((version, version_string));
            }
        }
        steps.sort_by(|a, b| a.0.cmp(&b.0));
        steps.dedup_by(|a, b| a.0 == b.0);
    }
    // If the database knows nothing about the versions in between, fall
    // back to a single direct delta attempt from the base to the target.
    if steps.is_empty() {
        steps.push((target_version.clone(), node.version.clone()));
    }

    if steps.len() as u32 > max_delta_steps {
        if verbose {
            println!(
                "{} Upgrade spans {} versions but at most {max_delta_steps} \
                 delta steps are configured; falling back to full download.",
                "Warning:".yellow(),
                steps.len()
            );
        }
        return Ok(false);
    }

    let work_dir = tempfile::Builder::new()
        .prefix("zoi-delta-")
        .tempdir_in(archive_cache_root)?;
    let mut current_base = base_path;
    let mut current_version = base_version.to_string();
    for (_, step_string) in &steps {
        match fetch_and_apply_delta(
            node,
            &current_base,
            &current_version,
            step_string,
            work_dir.path(),
            pb,
            verbose,
            pgp_identifiers
        ) {
            Ok(rebuilt) => {
                current_base = rebuilt;
                current_version.clone_from(step_string);
            }
            Err(e) => {
                if verbose {
                    println!(
                        "{} {e}; falling back to full download.",
                        "Warning:".yellow()
                    );
                }
                return Ok(false);
            }
        }
    }

    // Every delta step verified and applied successfully:
    // - Each delta patch was verified (hash, plus signature whenever trusted
    //   keys are configured)
    // - Each delta application does per-pool-entry structural verification
    // We skip the whole-archive hash check since the tar+zstd stream differs
    // from the original but the content is verified.
    //
    // With trusted keys configured, the rebuilt archive must additionally
    // carry an embedded signature that verifies, exactly like a downloaded
    // archive: the cached base may have been tampered with since it was
    // installed, and the base's files are reused verbatim. Detached sidecars
    // cannot apply to a rebuild, so a rebuild without a verifiable embedded
    // signature falls back to the full download.
    if let Some(identifiers) = pgp_identifiers
        && !identifiers.is_empty()
    {
        let trusted_certs = pgp::get_certs_by_name_or_fingerprint(identifiers)?;
        match try_verify_embedded_signature(&current_base, trusted_certs) {
            Ok(true) => {}
            Ok(false) => {
                if verbose {
                    println!(
                        "{} Delta-rebuilt archive has no embedded signature; \
                         falling back to full download.",
                        "Warning:".yellow()
                    );
                }
                return Ok(false);
            }
            Err(e) => {
                if verbose {
                    println!(
                        "{} Delta-rebuilt archive failed signature \
                         verification ({e}); falling back to full download.",
                        "Warning:".yellow()
                    );
                }
                return Ok(false);
            }
        }
    }

    // The work dir lives inside `archive_cache_root` so this persist stays on
    // the same filesystem. The copy fallback inside the helper covers
    // exotic mounts; otherwise a cross-device error would be swallowed by
    // the caller's delta fallback and surface as a confusing full `.zpa`
    // re-download after "Delta applied successfully."
    persist_rebuilt_archive(&current_base, target_path)?;
    if verbose {
        println!("{}", "All delta steps applied successfully.".green());
    }
    Ok(true)
}

/// Persists a delta-rebuilt archive to its final cache location.
///
/// Uses an atomic rename when possible, falling back to copy + remove for
/// robustness (e.g. exotic mounts where the staging dir and the cache live
/// on different filesystems). Without the fallback, a cross-device rename
/// error would be swallowed by the caller's delta fallback and surface as
/// a confusing full `.zpa` re-download after "Delta applied successfully."
fn persist_rebuilt_archive(src: &Path, dst: &Path) -> Result<()> {
    if fs::rename(src, dst).is_err() {
        fs::copy(src, dst)?;
        fs::remove_file(src).ok();
    }
    Ok(())
}

/// Attempts to verify an embedded `manifest.sig` entry inside a downloaded
/// archive against the pooled `manifest.json` it signs (and, through the
/// manifest's recorded digest, the bundled `.pkg.lua`).
///
/// Entries are located with the same rules extraction uses (see
/// [`crate::archive_trust`]): archives with duplicate or nested manifests or
/// signatures are rejected rather than verified against one copy and
/// installed from another.
///
/// Returns `Ok(false)` when the archive carries no embedded signature pair,
/// letting the caller fall back to the detached sidecar flow.
///
/// # Errors
///
/// Returns an error if the archive cannot be read, is ambiguous, or the
/// signature does not verify.
fn try_verify_embedded_signature(
    archive_path: &Path,
    trusted_certs: Vec<pgp::sequoia_openpgp::Cert>
) -> Result<bool> {
    crate::archive_trust::verify_embedded_manifest_signature(
        archive_path,
        trusted_certs
    )
}

/// Returns the PGP authorities a configured registry declares, if any.
fn registry_authorities<'a>(
    config: &'a types::Config,
    registry_handle: &str
) -> Option<&'a Vec<String>> {
    config
        .default_registry
        .as_ref()
        .filter(|r| r.handle == registry_handle)
        .and_then(|r| r.authorities.as_ref())
        .or_else(|| {
            config
                .added_registries
                .iter()
                .find(|r| r.handle == registry_handle)
                .and_then(|r| r.authorities.as_ref())
        })
}

/// Returns the PGP identifiers archives from `registry_handle` are verified
/// against: the signature-enforcement policy's trusted keys when enforcement
/// is enabled, otherwise the registry's declared authorities.
fn trusted_identifiers(
    config: &types::Config,
    registry_handle: &str
) -> Option<Vec<String>> {
    config
        .policy
        .signature_enforcement
        .as_ref()
        .filter(|p| p.enable)
        .map(|p| p.trusted_keys.clone())
        .or_else(|| registry_authorities(config, registry_handle).cloned())
}

/// Ensures that an enforced signature policy has everything needed to verify
/// the downloaded archive before it is installed.
fn validate_signature_requirements(
    enforcement_enabled: bool,
    has_signature_url: bool,
    has_trusted_keys: bool
) -> Result<()> {
    if !enforcement_enabled {
        return Ok(());
    }
    if !has_signature_url {
        return Err(anyhow!(
            "Signature enforcement is active, but no PGP URL was found for \
             the package"
        ));
    }
    if !has_trusted_keys {
        return Err(anyhow!(
            "Signature enforcement is active, but no trusted PGP keys are \
             configured"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{persist_rebuilt_archive, validate_signature_requirements};

    #[test]
    fn signature_enforcement_requires_a_signature_url() {
        assert!(validate_signature_requirements(true, false, true).is_err());
    }

    #[test]
    fn signature_enforcement_requires_trusted_keys() {
        assert!(validate_signature_requirements(true, true, false).is_err());
    }

    #[test]
    fn disabled_signature_enforcement_allows_missing_metadata() {
        assert!(validate_signature_requirements(false, false, false).is_ok());
    }

    #[test]
    fn persist_rebuilt_archive_moves_content_to_target() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("rebuilt-1.6.3.zpa");
        let dst = dir.path().join("zbsdiff-1.6.3-linux-amd64.zpa");
        std::fs::write(&src, b"rebuilt-archive-bytes").expect("write src");
        persist_rebuilt_archive(&src, &dst).expect("persist");
        assert!(!src.exists(), "source should be moved away");
        assert_eq!(
            std::fs::read(&dst).expect("read dst"),
            b"rebuilt-archive-bytes"
        );
    }

    #[test]
    fn persist_rebuilt_archive_overwrites_stale_target() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("rebuilt-1.6.3.zpa");
        let dst = dir.path().join("zbsdiff-1.6.3-linux-amd64.zpa");
        std::fs::write(&src, b"new-bytes").expect("write src");
        std::fs::write(&dst, b"stale-bytes").expect("write dst");
        persist_rebuilt_archive(&src, &dst).expect("persist");
        assert_eq!(std::fs::read(&dst).expect("read dst"), b"new-bytes");
    }
}

/// Information about a package that has been prepared for installation.
#[derive(Clone)]
pub struct PreparedNode {
    /// Path to the downloaded or built archive.
    pub archive_path: PathBuf,
    /// The method used for installation (e.g. "pre-compiled", "source").
    pub install_method: String,
    /// Whether the archive was built from source.
    pub is_build: bool
}

/// Performs the non-destructive first phase of installation: "Preparation".
///
/// Preparation includes:
/// - Downloading pre-built archives from the registry.
/// - Verifying checksums and PGP signatures (Root of Trust).
/// - Or, building the package from source in a temporary sandbox if requested.
///
/// This phase always runs in user-space and does not modify the system state
/// or the package store.
///
/// When `force_redownload` is set, cached archives are discarded and
/// fetched fresh instead of being reused.
///
/// # Errors
///
/// Returns an error if:
/// - The archive cannot be downloaded or built.
/// - The progress bar style cannot be created.
pub fn prepare_node(
    node: &InstallNode,
    action: &plan::InstallAction,
    m: Option<&MultiProgress>,
    build_type: Option<&str>,
    verbose: bool,
    force_redownload: bool
) -> Result<PreparedNode> {
    let pkg = &node.pkg;
    let version = &node.version;

    let pb_style = ProgressStyle::default_bar()
        .template(
            "{spinner:.green} {msg:30.cyan} [{bar:40.cyan/blue}] {percent}%"
        )?
        .progress_chars("#>-");

    let spinner_style = ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg:30.cyan}")?
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ");

    let display_name = if let Some(sub) = &node.sub_package {
        format!("{}:{}", pkg.name, sub)
    } else {
        pkg.name.clone()
    };
    let version_display = if node.revision == "1" {
        version.clone()
    } else {
        format!("{}-{}", version, node.revision)
    };
    let message = format!("zoi:{display_name}@{version_display}");

    let pb = if let Some(m_inner) = m {
        let pb = m_inner.add(ProgressBar::new(100));
        pb.set_style(pb_style);
        pb.set_message(message.clone());
        Some(pb)
    } else {
        None
    };

    let (archive_path, install_method, is_build) = match action {
        plan::InstallAction::DownloadAndInstall(details) => {
            if let Some(p) = &pb {
                p.set_message("Downloading package...");
            }
            let archive_path = download_and_cache_archive(
                node,
                details,
                pb.as_ref(),
                verbose,
                force_redownload
            )?;
            (archive_path, "pre-compiled".to_string(), false)
        }
        plan::InstallAction::InstallFromArchive(archive_path) => {
            if archive_path.to_string_lossy().ends_with(".zsa") {
                verify_local_source_bundle(node, archive_path, pb.as_ref())?;
                if let Some(p) = &pb {
                    p.set_style(spinner_style);
                    p.enable_steady_tick(std::time::Duration::from_millis(100));
                    p.set_message(format!("Building {display_name}..."));
                }
                let archive_path = prebuilt::build_archive(
                    archive_path,
                    pkg,
                    node.sub_package.as_deref(),
                    build_type,
                    pb.as_ref(),
                    !verbose
                )?;
                match archive_path {
                    Some(path) => (path, "source".to_string(), true),
                    None => (PathBuf::new(), "meta".to_string(), false)
                }
            } else {
                if let Some(p) = &pb {
                    p.set_message("Using local archive...");
                    p.finish();
                }
                (archive_path.clone(), "pre-compiled".to_string(), false)
            }
        }
        plan::InstallAction::BuildAndInstall => {
            if let Some(p) = &pb {
                p.set_style(spinner_style);
                p.enable_steady_tick(std::time::Duration::from_millis(100));
                p.set_message(format!("Building {display_name}..."));
            }
            let pkg_lua_path = Path::new(&node.source);
            let archive_path = prebuilt::build_archive(
                pkg_lua_path,
                pkg,
                node.sub_package.as_deref(),
                build_type,
                pb.as_ref(),
                !verbose
            )?;

            match archive_path {
                Some(path) => (path, "source".to_string(), true),
                None => (PathBuf::new(), "meta".to_string(), false)
            }
        }
    };

    if let Some(p) = pb {
        p.finish_and_clear();
    }

    Ok(PreparedNode {
        archive_path,
        install_method,
        is_build
    })
}

/// Verifies the embedded `<name>.pkg.lua.sig` of a local `.zsa` source bundle
/// before it is built, whenever trusted keys apply to the node's registry
/// (the signature-enforcement policy's keys, or the registry's
/// authorities).
///
/// A bundle without an embedded signature is still built, with a warning,
/// as before. A signature that is present but does not verify is an error.
///
/// # Errors
///
/// Returns an error if the configuration or keys cannot be loaded, or the
/// bundle's signature does not verify.
fn verify_local_source_bundle(
    node: &InstallNode,
    bundle_path: &Path,
    pb: Option<&ProgressBar>
) -> Result<()> {
    let config = config::read_config()?;
    let Some(identifiers) = trusted_identifiers(&config, &node.registry_handle)
        .filter(|keys| !keys.is_empty())
    else {
        return Ok(());
    };
    let trusted_certs = pgp::get_certs_by_name_or_fingerprint(&identifiers)?;
    match crate::archive_trust::verify_embedded_bundle_signature(
        bundle_path,
        &trusted_certs
    ) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            let msg = format!(
                "Warning: Source bundle '{}' carries no embedded signature; \
                 building it unverified.",
                bundle_path.display()
            );
            if let Some(p) = pb {
                p.println(msg.yellow().to_string());
            } else {
                println!("{}", msg.yellow());
            }
            Ok(())
        }
        Err(e) => Err(anyhow!(
            "Source bundle '{}' failed signature verification: {e}",
            bundle_path.display()
        ))
    }
}

/// Returns the dependent ID an installed package records in each of its
/// dependencies' `dependents/` directory.
///
/// This is the single format (`#handle@repo/name[:sub]@version`, i.e.
/// [`local::package_source_string`]) shared by install (here and in the
/// resolver's `InstallReason::Dependency` parent), uninstall and rollback,
/// which remove the link by recomputing the same string from the
/// dependent's manifest (`local::installed_manifest_source`).
#[must_use]
pub fn dependent_id(
    registry_handle: &str,
    repo: &str,
    name: &str,
    sub_package: Option<&str>,
    version: &str
) -> String {
    local::package_source_string(
        registry_handle,
        repo,
        name,
        sub_package,
        version
    )
}

/// Records `node` as a dependent of every installed Zoi package it depends
/// on.
///
/// Links are written from the dependent's side, using its resolved
/// dependency list, so a package required by several parents gets a link
/// from each of them, and dependencies that were already installed (and so
/// pruned from the install plan) are linked as well. This mirrors how
/// uninstall removes the links. Dependencies that cannot be matched to
/// exactly one installed package are skipped, again as uninstall does.
///
/// # Errors
///
/// Returns an error if installed manifests cannot be read or a link cannot
/// be written.
pub fn link_dependents(node: &InstallNode) -> Result<()> {
    let self_id = dependent_id(
        &node.registry_handle,
        &node.pkg.repo,
        &node.pkg.name,
        node.sub_package.as_deref(),
        &node.version
    );
    for dep_str in &node.dependencies {
        // The resolver records resolved Zoi dependencies as `zoi:` followed
        // by a full source string (`#handle@repo/name[:sub]@version`);
        // everything else belongs to an external package manager.
        let Some(source) = dep_str.strip_prefix("zoi:") else {
            continue;
        };
        let Ok(mut dep_req) =
            zoi_resolver::resolve::parse_source_string(source)
        else {
            continue;
        };
        let mut matches =
            local::find_installed_manifests_matching(&dep_req, node.pkg.scope)?;
        if matches.is_empty() && dep_req.version_spec.is_some() {
            // A dependency that was already installed may sit at another
            // version than the one the graph names; link whichever single
            // installation satisfies it, as uninstall's lookup does.
            dep_req.version_spec = None;
            matches = local::find_installed_manifests_matching(
                &dep_req,
                node.pkg.scope
            )?;
        }
        let [dep_manifest] = matches.as_slice() else {
            continue;
        };
        let dep_dir = local::get_package_dir(
            dep_manifest.scope,
            &dep_manifest.registry_handle,
            &dep_manifest.repo,
            &dep_manifest.name
        )?;
        local::add_dependent(&dep_dir, &self_id)?;
    }
    Ok(())
}

/// Performs the destructive second phase of installation: "Execution".
///
/// This phase takes a `PreparedNode` and:
/// - Unpacks the archive into the versioned store directory.
/// - Creates binary shims in the global Zoi `bin` directory.
/// - Registers the installation in the registry database and lockfile.
///
/// Just-in-Time Escalation: If the target scope is `system`, this function
/// will spawn a privileged sub-process (`sudo zoi helper elevate-install-node`)
/// to perform the final file moves, keeping the main CLI unprivileged.
///
/// # Errors
///
/// Returns an error if:
/// - Hooks fail to run.
/// - Privilege escalation fails.
/// - The archive cannot be unpacked.
/// - The manifest cannot be created or written.
/// - The package cannot be recorded in the database.
pub fn install_prepared_node(
    node: &InstallNode,
    prepared: &PreparedNode,
    m: Option<&MultiProgress>,
    yes: bool,
    record: bool,
    link_bins: bool,
    _verbose: bool
) -> Result<types::InstallManifest> {
    let pkg = &node.pkg;
    let version = &node.version;
    let handle = &node.registry_handle;
    let is_direct = matches!(node.reason, types::InstallReason::Direct);

    let pb_style = ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg:30.cyan}")?
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ");

    let main_pb = if let Some(m_inner) = m {
        if is_direct {
            None
        } else {
            let pb = m_inner.add(ProgressBar::new_spinner());
            pb.set_style(pb_style.clone());
            let name = if let Some(sub) = &node.sub_package {
                format!("{}:{}", pkg.name, sub)
            } else {
                pkg.name.clone()
            };
            let version_display = if node.revision == "1" {
                version.clone()
            } else {
                format!("{}-{}", version, node.revision)
            };
            pb.set_message(format!("zoi:{name}@{version_display}"));
            pb.enable_steady_tick(std::time::Duration::from_millis(100));
            Some(pb)
        }
    } else {
        None
    };

    let step_pb = if is_direct && let Some(m_inner) = m {
        let pb = m_inner.add(ProgressBar::new_spinner());
        pb.set_style(pb_style);
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    if let Some(hooks) = &pkg.hooks {
        if let Some(pb) = &step_pb {
            pb.set_message("Running pre-install hooks...");
        }
        hooks::run_hooks(hooks, hooks::HookType::PreInstall, pkg.scope)?;
    }

    let sub_package_to_install = node.sub_package.clone();
    let sub_packages_vec = sub_package_to_install.clone().map(|s| vec![s]);

    let archive_path = &prepared.archive_path;
    let install_method = &prepared.install_method;

    let needs_escalation =
        pkg.scope == types::Scope::System && !zoi_core::utils::is_admin();

    let install_manifest = if needs_escalation {
        let escalator =
            zoi_core::utils::get_privilege_escalator().ok_or_else(|| {
                anyhow!(
                    "Root privileges required for system scope installation, \
                     but neither 'sudo' nor 'doas' was found."
                )
            })?;

        if let Some(pb) = step_pb.as_ref().or(main_pb.as_ref()) {
            pb.set_message(format!(
                "Waiting for {escalator} privileges to install system \
                 package..."
            ));
        }

        let node_json = serde_json::to_string(node)?;
        let mut temp_file = tempfile::NamedTempFile::new()?;
        temp_file.write_all(node_json.as_bytes())?;
        let temp_path = temp_file.path();

        let mut cmd = std::process::Command::new(escalator);
        cmd.arg(std::env::current_exe()?);
        cmd.arg("helper").arg("elevate-install-node");
        cmd.arg("--node-json").arg(temp_path);
        cmd.arg("--archive").arg(archive_path);
        cmd.arg("--install-method").arg(install_method);
        if yes {
            cmd.arg("--yes");
        }
        if link_bins {
            cmd.arg("--link-bins");
        }

        let status = cmd
            .status()
            .map_err(|e| anyhow!("Failed to spawn privilege escalator: {e}"))?;
        if !status.success() {
            return Err(anyhow!("Escalated installation failed."));
        }

        let version_dir = local::get_package_version_dir(
            pkg.scope,
            &node.registry_handle,
            &pkg.repo,
            &pkg.name,
            &node.version
        )?;
        let manifest_path = local::find_store_manifest(
            &version_dir,
            node.sub_package.as_deref()
        )
        .ok_or_else(|| {
            anyhow!(
                "Escalated install did not leave a manifest in '{}'",
                version_dir.display()
            )
        })?;
        local::read_store_manifest(&manifest_path)?
    } else {
        if let Some(pb) = step_pb.as_ref().or(main_pb.as_ref()) {
            pb.set_message(format!("Installing {}...", pkg.name.cyan()));
        }

        let (installed_files, file_digests) = crate::pkg_install::run(
            archive_path,
            Some(pkg.scope),
            &node.registry_handle,
            Some(&node.version),
            yes,
            sub_packages_vec,
            link_bins,
            step_pb.as_ref().or(main_pb.as_ref())
        )?;

        // The resolver's reason only names one parent (for display); a
        // dependency shared by several parents, or one installed during an
        // earlier transaction, is linked from each dependent's side by
        // `link_dependents` below. The placeholder parent used when no
        // parent is known must never be recorded as a dependent, or it would
        // pin the package forever.
        if let types::InstallReason::Dependency { ref parent } = node.reason
            && parent != "unknown"
        {
            let package_dir = local::get_package_dir(
                pkg.scope, handle, &pkg.repo, &pkg.name
            )?;
            local::add_dependent(&package_dir, parent)?;
        }
        link_dependents(node)?;

        let install_manifest = manifest::create_manifest(
            pkg,
            node.reason.clone(),
            node.dependencies.clone(),
            Some(install_method.clone()),
            installed_files,
            handle,
            node.repo_type.clone(),
            &node.chosen_options,
            &node.chosen_optionals,
            sub_package_to_install.clone(),
            file_digests
        )?;

        if record {
            local::write_manifest(&install_manifest)?;
            local::persist_package_source(
                &install_manifest,
                Path::new(&node.source)
            )?;
        }

        install_manifest
    };

    if prepared.is_build {
        let _ = fs::remove_file(archive_path);
    }

    if record {
        if let Ok(conn) = db::open_connection("local")
            && let Ok(pkg_id) = db::update_package(
                &conn,
                pkg,
                handle,
                Some(pkg.scope),
                sub_package_to_install.as_deref(),
                Some(&node.reason)
            )
        {
            let _ = db::clear_package_files(&conn, pkg_id);
            let _ = db::index_package_files(
                &conn,
                pkg_id,
                &install_manifest.installed_files
            );
        }

        if let Err(e) = recorder::record_package(
            pkg,
            &node.reason,
            &node.dependencies,
            handle,
            &node.repo_type,
            &node.chosen_options,
            &node.chosen_optionals,
            sub_package_to_install.as_deref(),
            &node.source,
            node.git_sha.as_deref()
        ) {
            eprintln!(
                "Warning: failed to record package installation for '{}': {}",
                pkg.name, e
            );
        }
    }

    if let Some(hooks) = &pkg.hooks {
        if let Some(pb) = &step_pb {
            pb.set_message("Running post-install hooks...");
        }
        hooks::run_hooks(hooks, hooks::HookType::PostInstall, pkg.scope)?;
    }

    if let Some(pb) = main_pb {
        pb.finish();
    }
    if let Some(pb) = step_pb {
        pb.finish();
    }

    util::send_telemetry("install", pkg, handle, Some(install_method));

    Ok(install_manifest)
}

/// Performs both preparation and execution phases for an install node.
///
/// When `force_redownload` is set, cached archives are discarded and
/// fetched fresh instead of being reused.
///
/// # Errors
///
/// Returns an error if preparation or execution fails.
pub fn install_node(
    node: &InstallNode,
    action: &plan::InstallAction,
    m: Option<&MultiProgress>,
    build_type: Option<&str>,
    yes: bool,
    record: bool,
    link_bins: bool,
    verbose: bool,
    force_redownload: bool
) -> Result<types::InstallManifest> {
    let prepared =
        prepare_node(node, action, m, build_type, verbose, force_redownload)?;
    install_prepared_node(node, &prepared, m, yes, record, link_bins, verbose)
}
