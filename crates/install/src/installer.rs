use crate::resolver::InstallNode;
use crate::{manifest, plan, post_install, prebuilt, util};
use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::path::{Path, PathBuf};
use zoi_core::{cache, config, pgp, pkgdir, recorder, types};
use zoi_db as db;
use zoi_hooks as hooks;
use zoi_resolver::local;

pub fn download_and_cache_archive(
    _node: &InstallNode,
    details: &plan::PrebuiltDetails,
    pb: Option<&ProgressBar>,
    verbose: bool,
) -> Result<PathBuf> {
    let config = config::read_config()?;
    let signature_policy = config.policy.signature_enforcement.filter(|p| p.enable);

    let archive_cache_root = cache::get_archive_cache_root()?;
    fs::create_dir_all(&archive_cache_root)?;

    let archive_filename = details
        .info
        .final_url
        .split('/')
        .next_back()
        .unwrap_or("archive.pkg.tar.zst");
    let cached_archive_path = archive_cache_root.join(archive_filename);
    let sig_filename = format!("{}.sig", archive_filename);
    let cached_sig_path = archive_cache_root.join(&sig_filename);

    let archive_path = if let Some(path) = pkgdir::find_in_pkg_dirs(archive_filename) {
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
                "Archive not found in cache and cannot download: Zoi is in offline mode. Missing: {}",
                archive_filename
            ));
        }
        let part_path = archive_cache_root.join(format!("{}.part", archive_filename));

        if part_path.exists() && pb.is_none() {
            println!("Resuming partial download: {}", part_path.display());
        }

        let mut last_error = None;
        let candidate_urls = cache::mirror_candidate_urls(&details.info.final_url);
        let mut downloaded = false;
        for candidate_url in candidate_urls {
            match util::download_file_with_progress(
                &candidate_url,
                &part_path,
                pb,
                Some(details.download_size),
            ) {
                Ok(()) => {
                    downloaded = true;
                    break;
                }
                Err(e) => last_error = Some((candidate_url, e)),
            }
        }
        if !downloaded {
            let (url, error) = last_error
                .ok_or_else(|| anyhow!("archive download failed but no error recorded"))?;
            return Err(anyhow!(
                "Failed to download package archive from {}: {}",
                url,
                error
            ));
        }

        fs::rename(&part_path, &cached_archive_path)?;
        cached_archive_path.clone()
    };

    if let Some(hash_url) = &details.info.hash_url {
        let hash = db::get_package_hash_from_db(
            &_node.registry_handle,
            &_node.pkg.name,
            _node.sub_package.as_deref(),
            &_node.pkg.repo,
        )
        .unwrap_or(None)
        .filter(|h| !h.is_empty())
        .or_else(|| util::get_expected_hash(hash_url, Some(archive_filename)).ok());

        if let Some(ref hash) = hash
            && !util::verify_file_hash(&archive_path, hash, pb)?
        {
            return Err(anyhow!("Hash verification failed"));
        }
    }

    let authorities = config
        .default_registry
        .as_ref()
        .filter(|r| r.handle == _node.registry_handle)
        .and_then(|r| r.authorities.as_ref())
        .or_else(|| {
            config
                .added_registries
                .iter()
                .find(|r| r.handle == _node.registry_handle)
                .and_then(|r| r.authorities.as_ref())
        });
    let has_authorities = authorities.is_some_and(|a| !a.is_empty());
    let pgp_identifiers: Option<Vec<String>> = signature_policy
        .as_ref()
        .map(|p| p.trusted_keys.clone())
        .or_else(|| authorities.cloned());

    if let Some(pgp_url) = &details.info.pgp_url {
        if let Some(ref identifiers) = pgp_identifiers
            && !identifiers.is_empty()
        {
            let sig_path = if cached_sig_path.exists() {
                cached_sig_path.clone()
            } else {
                if zoi_core::offline::is_offline() {
                    return Err(anyhow!(
                        "Signature not found in cache and cannot download: Zoi is in offline mode."
                    ));
                }
                let temp_dir = tempfile::Builder::new().prefix("zoi-sig-dl-").tempdir()?;
                let temp_sig_path = temp_dir.path().join(&sig_filename);
                let mut last_error = None;
                let mut downloaded = false;
                for candidate_url in cache::mirror_candidate_urls(pgp_url) {
                    match util::download_file_with_progress(
                        &candidate_url,
                        &temp_sig_path,
                        pb,
                        None,
                    ) {
                        Ok(()) => {
                            downloaded = true;
                            break;
                        }
                        Err(e) => last_error = Some((candidate_url, e)),
                    }
                }
                if !downloaded {
                    let (url, error) = last_error.ok_or_else(|| {
                        anyhow!("signature download failed but no error recorded")
                    })?;
                    return Err(anyhow!(
                        "Failed to download signature from {}: {}",
                        url,
                        error
                    ));
                }
                fs::copy(&temp_sig_path, &cached_sig_path)?;
                cached_sig_path.clone()
            };

            if verbose {
                println!("Verifying signature...");
            }
            let trusted_certs = pgp::get_certs_by_name_or_fingerprint(identifiers)?;
            pgp::verify_detached_signature_multi_key(&archive_path, &sig_path, trusted_certs)?;
            if verbose {
                println!("{}", "Signature verified successfully.".green());
            }
        }
    } else if has_authorities {
        let msg = format!(
            "Warning: Installing unsigned package '{}' from a registry that claims to be secure.",
            _node.pkg.name
        );
        if let Some(p) = pb {
            p.println(msg.yellow().to_string());
        } else {
            println!("{}", msg.yellow());
        }
        if signature_policy.is_some() {
            return Err(anyhow!(
                "Signature enforcement is active, but no PGP URL found for package"
            ));
        }
    }

    Ok(archive_path)
}

#[derive(Clone)]
pub struct PreparedNode {
    pub archive_path: PathBuf,
    pub install_method: String,
    pub is_build: bool,
}

pub fn prepare_node(
    node: &InstallNode,
    action: &plan::InstallAction,
    m: Option<&MultiProgress>,
    build_type: Option<&str>,
    verbose: bool,
) -> Result<PreparedNode> {
    let pkg = &node.pkg;
    let version = &node.version;

    let pb_style = ProgressStyle::default_bar()
        .template("{spinner:.green} {msg:30.cyan} [{bar:40.cyan/blue}] {percent}%")?
        .progress_chars("#>-");

    let pb = if let Some(m_inner) = m {
        let pb = m_inner.add(ProgressBar::new(100));
        pb.set_style(pb_style);
        let name = if let Some(sub) = &node.sub_package {
            format!("{}:{}", pkg.name, sub)
        } else {
            pkg.name.clone()
        };
        let version_display = if node.revision != "1" {
            format!("{}-{}", version, node.revision)
        } else {
            version.clone()
        };
        pb.set_message(format!("zoi: @{}:{}", name, version_display));
        Some(pb)
    } else {
        None
    };

    let (archive_path, install_method, is_build) = match action {
        plan::InstallAction::DownloadAndInstall(details) => {
            if let Some(p) = &pb {
                p.set_message("Downloading package...");
            }
            let archive_path = download_and_cache_archive(node, details, pb.as_ref(), verbose)?;
            (archive_path, "pre-compiled".to_string(), false)
        }
        plan::InstallAction::InstallFromArchive(archive_path) => {
            if let Some(p) = &pb {
                p.set_message("Using local archive...");
                p.finish();
            }
            (archive_path.clone(), "pre-compiled".to_string(), false)
        }
        plan::InstallAction::BuildAndInstall => {
            let pkg_lua_path = Path::new(&node.source);
            let archive_path = prebuilt::build_archive(pkg_lua_path, pkg, build_type, pb.as_ref())?;
            (archive_path, "source".to_string(), true)
        }
    };

    if let Some(p) = pb {
        p.finish_and_clear();
    }

    Ok(PreparedNode {
        archive_path,
        install_method,
        is_build,
    })
}

pub fn install_prepared_node(
    node: &InstallNode,
    prepared: &PreparedNode,
    m: Option<&MultiProgress>,
    yes: bool,
    record: bool,
    link_bins: bool,
    _verbose: bool,
) -> Result<types::InstallManifest> {
    let pkg = &node.pkg;
    let version = &node.version;
    let handle = &node.registry_handle;
    let is_direct = matches!(node.reason, types::InstallReason::Direct);

    let pb_style = ProgressStyle::default_bar()
        .template("{spinner:.green} {msg:30.cyan} [{bar:40.cyan/blue}] {percent}%")?
        .progress_chars("#>-");

    let main_pb = if let Some(m_inner) = m {
        if !is_direct {
            let pb = m_inner.add(ProgressBar::new(100));
            pb.set_style(pb_style.clone());
            let name = if let Some(sub) = &node.sub_package {
                format!("{}:{}", pkg.name, sub)
            } else {
                pkg.name.clone()
            };
            let version_display = if node.revision != "1" {
                format!("{}-{}", version, node.revision)
            } else {
                version.clone()
            };
            pb.set_message(format!("zoi: @{}:{}", name, version_display));
            Some(pb)
        } else {
            None
        }
    } else {
        None
    };

    let step_pb = if is_direct && let Some(m_inner) = m {
        let pb = m_inner.add(ProgressBar::new(100));
        pb.set_style(pb_style);
        Some(pb)
    } else {
        None
    };

    if let Some(hooks) = &pkg.hooks {
        if let Some(pb) = &step_pb {
            pb.set_message("Running pre-install hooks...");
        }
        hooks::run_hooks(hooks, hooks::HookType::PreInstall)?;
    }

    let sub_package_to_install = node.sub_package.clone();
    let sub_packages_vec = sub_package_to_install.clone().map(|s| vec![s]);

    let archive_path = &prepared.archive_path;
    let install_method = &prepared.install_method;

    let needs_escalation = pkg.scope == types::Scope::System && !zoi_core::utils::is_admin();

    let manifest = if needs_escalation {
        if let Some(pb) = step_pb.as_ref().or(main_pb.as_ref()) {
            pb.set_message("Waiting for sudo privileges to install system package...");
        }

        let node_json = serde_json::to_string(node)?;
        let mut temp_file = tempfile::NamedTempFile::new()?;
        use std::io::Write;
        temp_file.write_all(node_json.as_bytes())?;
        let temp_path = temp_file.path();

        let mut cmd = std::process::Command::new("sudo");
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
            .map_err(|e| anyhow!("Failed to spawn sudo: {}", e))?;
        if !status.success() {
            return Err(anyhow!("Escalated installation failed."));
        }

        let version_dir = local::get_package_version_dir(
            pkg.scope,
            &node.registry_handle,
            &pkg.repo,
            &pkg.name,
            &node.version,
        )?;
        let manifest_filename = if let Some(sub) = &node.sub_package {
            format!("manifest-{}.yaml", sub)
        } else {
            "manifest.yaml".to_string()
        };
        let manifest_path = version_dir.join(manifest_filename);
        let content = std::fs::read_to_string(&manifest_path)?;
        let manifest: types::InstallManifest = serde_yaml::from_str(&content)?;

        manifest
    } else {
        if let Some(pb) = step_pb.as_ref().or(main_pb.as_ref()) {
            pb.set_message("Installing package...");
            pb.set_position(0);
        }

        let installed_files = crate::pkg_install::run(
            archive_path,
            Some(pkg.scope),
            &node.registry_handle,
            Some(&node.version),
            yes,
            sub_packages_vec,
            link_bins,
            step_pb.as_ref().or(main_pb.as_ref()),
        )?;

        if let types::InstallReason::Dependency { ref parent } = node.reason {
            let package_dir = local::get_package_dir(pkg.scope, handle, &pkg.repo, &pkg.name)?;
            local::add_dependent(&package_dir, parent)?;
        }

        if let Err(e) =
            post_install::install_manual_if_available(pkg, version, handle, step_pb.as_ref())
        {
            let msg = format!(
                "Warning: failed to install manual for '{}': {}",
                pkg.name, e
            );
            if let Some(p) = &step_pb {
                p.println(msg);
            } else if let Some(p) = &main_pb {
                p.println(msg);
            } else {
                eprintln!("{}", msg);
            }
        }

        let manifest = manifest::create_manifest(
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
        )?;

        if record {
            local::write_manifest(&manifest)?;
            local::persist_package_source(&manifest, Path::new(&node.source))?;
        }

        manifest
    };

    if prepared.is_build {
        let _ = fs::remove_file(archive_path);
    }

    if record {
        if let Ok(conn) = db::open_connection("local") {
            let _ = db::update_package(
                &conn,
                pkg,
                handle,
                Some(pkg.scope),
                sub_package_to_install.as_deref(),
                Some(&node.reason),
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
            sub_package_to_install.clone(),
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
        hooks::run_hooks(hooks, hooks::HookType::PostInstall)?;
    }

    if let Some(pb) = main_pb {
        pb.finish();
    }
    if let Some(pb) = step_pb {
        pb.finish();
    }

    util::send_telemetry("install", pkg, handle, Some(install_method));

    Ok(manifest)
}

pub fn install_node(
    node: &InstallNode,
    action: &plan::InstallAction,
    m: Option<&MultiProgress>,
    build_type: Option<&str>,
    yes: bool,
    record: bool,
    link_bins: bool,
    verbose: bool,
) -> Result<types::InstallManifest> {
    let prepared = prepare_node(node, action, m, build_type, verbose)?;
    install_prepared_node(node, &prepared, m, yes, record, link_bins, verbose)
}
