use crate::pkg::{
    cache, config, db, hooks,
    install::{manifest, plan, post_install, prebuilt, resolver::InstallNode, util},
    local, pgp, pkgdir, recorder, resolve, types,
};
use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::path::{Path, PathBuf};

pub fn download_and_cache_archive(
    _node: &InstallNode,
    details: &plan::PrebuiltDetails,
    pb: Option<&ProgressBar>,
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
        if crate::pkg::offline::is_offline() {
            return Err(anyhow!(
                "Archive not found in cache and cannot download: Zoi is in offline mode. Missing: {}",
                archive_filename
            ));
        }
        let temp_dir = tempfile::Builder::new().prefix("zoi-dl-").tempdir()?;
        let temp_archive_path = temp_dir.path().join(archive_filename);

        let mut last_error = None;
        let candidate_urls = cache::mirror_candidate_urls(&details.info.final_url);
        let mut downloaded = false;
        for candidate_url in candidate_urls {
            match util::download_file_with_progress(
                &candidate_url,
                &temp_archive_path,
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
            let (url, error) = last_error.expect("archive download should produce an error");
            return Err(anyhow!(
                "Failed to download package archive from {}: {}",
                url,
                error
            ));
        }

        fs::copy(&temp_archive_path, &cached_archive_path)?;
        cached_archive_path.clone()
    };

    if let Some(hash_url) = &details.info.hash_url {
        let hash = util::get_expected_hash(hash_url, Some(archive_filename))?;
        if !hash.is_empty() && !util::verify_file_hash(&archive_path, &hash, pb)? {
            return Err(anyhow!("Hash verification failed"));
        }
    }

    if let Some(policy) = &signature_policy {
        if let Some(pgp_url) = &details.info.pgp_url {
            let sig_path = if cached_sig_path.exists() {
                cached_sig_path.clone()
            } else {
                if crate::pkg::offline::is_offline() {
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
                    let (url, error) =
                        last_error.expect("signature download should produce an error");
                    return Err(anyhow!(
                        "Failed to download signature from {}: {}",
                        url,
                        error
                    ));
                }
                fs::copy(&temp_sig_path, &cached_sig_path)?;
                cached_sig_path.clone()
            };

            println!("Verifying signature...");
            let trusted_certs = pgp::get_certs_by_name_or_fingerprint(&policy.trusted_keys)?;
            pgp::verify_detached_signature_multi_key(&archive_path, &sig_path, trusted_certs)?;
            println!("{}", "Signature verified successfully.".green());
        } else {
            return Err(anyhow!(
                "Signature enforcement is active, but no PGP URL found for package"
            ));
        }
    }

    Ok(archive_path)
}

pub fn install_node(
    node: &InstallNode,
    action: &plan::InstallAction,
    m: Option<&MultiProgress>,
    build_type: Option<&str>,
    yes: bool,
    link_bins: bool,
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
            pb.set_message(format!("zoi: @{}:{}", name, version));
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

    let request = resolve::parse_source_string(&node.source)?;
    let sub_package_to_install = request.sub_package;
    let sub_packages_vec = sub_package_to_install.clone().map(|s| vec![s]);

    let (installed_files, install_method) = match action {
        plan::InstallAction::DownloadAndInstall(details) => {
            let pb_for_step = step_pb.as_ref().or(main_pb.as_ref());
            if let Some(pb) = pb_for_step {
                pb.set_message("Downloading package...");
            }
            let archive_path = download_and_cache_archive(node, details, pb_for_step)?;
            if let Some(pb) = pb_for_step {
                pb.set_message("Installing package...");
                pb.set_position(0);
            }
            let files = crate::pkg::package::install::run(
                &archive_path,
                Some(pkg.scope),
                &node.registry_handle,
                Some(&node.version),
                yes,
                sub_packages_vec,
                link_bins,
                pb_for_step,
            )?;
            (files, "pre-compiled".to_string())
        }
        plan::InstallAction::InstallFromArchive(archive_path) => {
            let pb_for_step = step_pb.as_ref().or(main_pb.as_ref());
            if let Some(pb) = pb_for_step {
                pb.set_message("Installing package...");
            }
            let files = crate::pkg::package::install::run(
                archive_path,
                Some(pkg.scope),
                &node.registry_handle,
                Some(&node.version),
                yes,
                sub_packages_vec,
                link_bins,
                pb_for_step,
            )?;
            (files, "pre-compiled".to_string())
        }
        plan::InstallAction::BuildAndInstall => {
            let pb_for_step = step_pb.as_ref().or(main_pb.as_ref());
            if let Some(pb) = pb_for_step {
                pb.set_message("Building package...");
            }
            let pkg_lua_path = Path::new(&node.source);
            let files = prebuilt::try_build_install(
                pkg_lua_path,
                pkg,
                &node.registry_handle,
                build_type,
                yes,
                sub_package_to_install.clone(),
                pb_for_step,
            )?;
            (files, "source".to_string())
        }
    };

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
        &node.chosen_options,
        &node.chosen_optionals,
        sub_package_to_install.clone(),
    )?;

    local::write_manifest(&manifest)?;
    local::persist_package_source(&manifest, Path::new(&node.source))?;

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
        &node.chosen_options,
        &node.chosen_optionals,
        sub_package_to_install.clone(),
    ) {
        eprintln!(
            "Warning: failed to record package installation for '{}': {}",
            pkg.name, e
        );
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

    util::send_telemetry("install", pkg, handle, Some(&install_method));

    Ok(manifest)
}
