//! Main installer logic.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use zoi_core::{cache, config, pgp, pkgdir, recorder, types};
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
    verbose: bool
) -> Result<PathBuf> {
    let config = config::read_config()?;
    let signature_policy =
        config.policy.signature_enforcement.filter(|p| p.enable);

    let archive_cache_root = cache::get_archive_cache_root()?;
    fs::create_dir_all(&archive_cache_root)?;

    let archive_filename = util::get_filename_from_url(&details.info.final_url);
    let archive_filename = if archive_filename.is_empty() {
        "archive.zpa"
    } else {
        archive_filename
    };
    let cached_archive_path = archive_cache_root.join(archive_filename);
    let sig_filename = format!("{archive_filename}.sig");
    let cached_sig_path = archive_cache_root.join(&sig_filename);

    let authorities = config
        .default_registry
        .as_ref()
        .filter(|r| r.handle == node.registry_handle)
        .and_then(|r| r.authorities.as_ref())
        .or_else(|| {
            config
                .added_registries
                .iter()
                .find(|r| r.handle == node.registry_handle)
                .and_then(|r| r.authorities.as_ref())
        });
    let has_authorities = authorities.is_some_and(|a| !a.is_empty());
    let pgp_identifiers: Option<Vec<String>> = signature_policy
        .as_ref()
        .map(|p| p.trusted_keys.clone())
        .or_else(|| authorities.cloned());
    let has_pgp_identifiers = pgp_identifiers
        .as_ref()
        .is_some_and(|keys| !keys.is_empty());

    // Track whether the archive was rebuilt from a delta so we can skip
    // the whole-archive checksum (per-pool-entry integrity was already
    // verified during delta application).
    let mut delta_applied = false;

    let archive_path = if let Some(path) =
        pkgdir::find_in_pkg_dirs(archive_filename)
    {
        if pb.is_none() {
            println!("Found archive in pkg-dir: {}", path.display());
        }
        path
    } else if cached_archive_path.exists() {
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

        // Delta fast-path: if a previous version of this archive is cached,
        // try to fetch and apply a `.zdelta` instead of downloading the full
        // archive. Any failure here silently falls back to the full
        // download below. The rebuilt archive is written to
        // `cached_archive_path` so the normal signature checks still run
        // against it. Note: the whole-archive checksum is skipped for
        // delta-rebuilt archives because the tar+zstd byte stream differs
        // from the original while per-pool-entry integrity is already
        // verified by `apply_zpa_delta`.
        delta_applied = try_delta_upgrade(
            node,
            &archive_cache_root,
            &cached_archive_path,
            pb,
            verbose,
            pgp_identifiers.as_deref()
        )
        .unwrap_or(false);

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

    // An embedded `manifest.sig` entry travels inside the archive itself, so
    // it is preferred over a detached sidecar: it works even when the archive
    // is distributed out-of-band without its `.sig`.
    let mut embedded_verified = false;
    if let Some(ref identifiers) = pgp_identifiers
        && !identifiers.is_empty()
        && !matches!(archive_filename.rsplit('.').next(), Some("zsa"))
    {
        let trusted_certs = pgp::get_certs_by_name_or_fingerprint(identifiers)?;
        match try_verify_embedded_signature(&archive_path, trusted_certs) {
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
                return Err(anyhow!(
                    "Embedded signature verification failed: {e}"
                ));
            }
        }
    }

    if !embedded_verified
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

/// Attempts to upgrade a package archive via a `.zdelta` against a cached
/// previous version.
///
/// Uses explicit delta URLs from repo.yaml (delta section). Verifies the
/// delta patch file's hash and signature, then applies it. Returns
/// `Ok(false)` whenever a delta is unavailable or cannot be applied so the
/// caller can fall back to a full download.
fn try_delta_upgrade(
    node: &InstallNode,
    archive_cache_root: &Path,
    target_path: &Path,
    pb: Option<&ProgressBar>,
    verbose: bool,
    pgp_identifiers: Option<&[String]>
) -> Result<bool> {
    let Ok(target_version) = semver::Version::parse(&node.version) else {
        return Ok(false);
    };

    // Find the newest cached archive of the same package on the same
    // platform that is older than the target version. Archive names follow
    // `{name}-{version}-{platform}.zpa`.
    let platform = zoi_core::utils::get_platform().unwrap_or_default();
    let prefix = format!("{}-", node.pkg.name);
    let suffix = format!("-{platform}.zpa");
    let mut best: Option<(semver::Version, PathBuf)> = None;
    let entries = std::fs::read_dir(archive_cache_root).ok();
    let Some(entries) = entries else {
        return Ok(false);
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) || !name.ends_with(&suffix) {
            continue;
        }
        let middle = &name[prefix.len()..name.len() - suffix.len()];
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

    // Look up explicit delta info from repo.yaml
    let Some(delta_info) =
        crate::util::find_delta_info_for_node(node, &base_version.to_string())?
    else {
        if verbose {
            println!(
                "No delta configuration found; falling back to full download."
            );
        }
        return Ok(false);
    };

    if verbose {
        println!("Attempting delta upgrade from v{base_version}...");
    }

    let temp_dir = tempfile::Builder::new().prefix("zoi-delta-").tempdir()?;
    let delta_path = temp_dir.path().join("upgrade.zdelta");
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
        if verbose {
            println!("No delta available; falling back to full download.");
        }
        return Ok(false);
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
        if verbose {
            println!(
                "{} Delta hash verification failed; falling back to full \
                 download.",
                "Warning:".yellow()
            );
        }
        return Ok(false);
    }

    // Verify delta patch signature if PGP URL and trusted keys available.
    if let Some(pgp_url) = &delta_info.pgp_url
        && let Some(identifiers) = pgp_identifiers
        && !identifiers.is_empty()
    {
        let temp_dir = tempfile::Builder::new()
            .prefix("zoi-delta-sig-")
            .tempdir()?;
        let temp_sig_path = temp_dir.path().join("delta.sig");
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
        if downloaded {
            let trusted_certs =
                pgp::get_certs_by_name_or_fingerprint(identifiers)?;
            pgp::verify_detached_signature_multi_key(
                &delta_path,
                &temp_sig_path,
                trusted_certs
            )?;
            if verbose {
                println!(
                    "{}",
                    "Delta signature verified successfully.".green()
                );
            }
        } else if verbose {
            println!(
                "{} Delta signature file not found; skipping signature \
                 verification.",
                "Warning:".yellow()
            );
        }
    } else if verbose {
        println!(
            "{} No trusted PGP keys for delta signature verification.",
            "Warning:".yellow()
        );
    }

    let rebuilt_path = temp_dir.path().join("rebuilt.zpa");
    if let Err(e) = zoi_package::delta::apply_zpa_delta(
        &base_path,
        &delta_path,
        &rebuilt_path
    ) {
        if verbose {
            println!(
                "{} Delta application failed ({e}); falling back to full \
                 download.",
                "Warning:".yellow()
            );
        }
        return Ok(false);
    }

    // Delta patch verified and applied successfully. The rebuilt archive
    // is trusted because:
    // - The base archive was already verified (full hash + sig on install)
    // - The delta patch was verified (hash + optionally sig)
    // - The delta application does per-pool-entry structural verification
    // We skip the whole-archive hash check since the tar+zstd stream differs
    // from the original but the content is verified.
    fs::rename(&rebuilt_path, target_path)?;
    if verbose {
        println!("{}", "Delta applied successfully.".green());
    }
    Ok(true)
}

/// Attempts to verify an embedded `manifest.sig` entry inside a downloaded
/// archive against the pooled `manifest.json` it signs.
///
/// Returns `Ok(false)` when the archive carries no embedded signature pair,
/// letting the caller fall back to the detached sidecar flow.
///
/// # Errors
///
/// Returns an error if the archive cannot be read or the signature does not
/// verify.
fn try_verify_embedded_signature(
    archive_path: &Path,
    trusted_certs: Vec<pgp::sequoia_openpgp::Cert>
) -> Result<bool> {
    use std::io::Read;

    let file = fs::File::open(archive_path)?;
    let decoder = zstd::stream::read::Decoder::new(file)?;
    let mut archive = tar::Archive::new(decoder);

    let mut manifest_bytes: Option<Vec<u8>> = None;
    let mut sig_bytes: Option<Vec<u8>> = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().to_string();
        match Path::new(&name).file_name().and_then(|f| f.to_str()) {
            Some("manifest.json") if manifest_bytes.is_none() => {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                manifest_bytes = Some(buf);
            }
            Some("manifest.sig") => {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                sig_bytes = Some(buf);
            }
            _ => {}
        }
    }

    let (Some(manifest), Some(sig)) = (manifest_bytes, sig_bytes) else {
        return Ok(false);
    };

    // Reuse the existing path-based multi-key verifier on temporary files so
    // we sign-verify the exact bytes that ship inside the archive.
    let temp_dir = tempfile::Builder::new()
        .prefix("zoi-embed-sig-")
        .tempdir()?;
    let manifest_path = temp_dir.path().join("manifest.json");
    let sig_path = temp_dir.path().join("manifest.sig");
    fs::write(&manifest_path, &manifest)?;
    fs::write(&sig_path, &sig)?;
    pgp::verify_detached_signature_multi_key(
        &manifest_path,
        &sig_path,
        trusted_certs
    )?;
    Ok(true)
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
    use super::validate_signature_requirements;

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
    verbose: bool
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
                verbose
            )?;
            (archive_path, "pre-compiled".to_string(), false)
        }
        plan::InstallAction::InstallFromArchive(archive_path) => {
            if archive_path.to_string_lossy().ends_with(".zsa") {
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
        let manifest_filename = if let Some(sub) = &node.sub_package {
            format!("manifest-{sub}.yaml")
        } else {
            "manifest.yaml".to_string()
        };
        let manifest_path = version_dir.join(manifest_filename);
        let content = std::fs::read_to_string(&manifest_path)?;
        let install_manifest: types::InstallManifest =
            serde_yaml::from_str(&content)?;

        install_manifest
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

        if let types::InstallReason::Dependency { ref parent } = node.reason {
            let package_dir = local::get_package_dir(
                pkg.scope, handle, &pkg.repo, &pkg.name
            )?;
            local::add_dependent(&package_dir, parent)?;
        }

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
            sub_package_to_install.as_deref()
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
    verbose: bool
) -> Result<types::InstallManifest> {
    let prepared = prepare_node(node, action, m, build_type, verbose)?;
    install_prepared_node(node, &prepared, m, yes, record, link_bins, verbose)
}
