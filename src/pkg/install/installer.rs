use crate::pkg::{
    cache, config, hooks,
    install::{flow::InstallMode, manifest, post_install, prebuilt, resolver::InstallNode, util},
    local, pgp, recorder, resolve, types,
};
use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::MultiProgress;
use std::fs;
use std::path::Path;

pub fn install_node(
    node: &InstallNode,
    mode: InstallMode,
    m: Option<&MultiProgress>,
    build_type: Option<&str>,
    yes: bool,
) -> Result<types::InstallManifest> {
    let pkg = &node.pkg;
    let version = &node.version;
    let handle = &node.registry_handle;

    if let Some(hooks) = &pkg.hooks
        && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PreInstall)
    {
        return Err(anyhow!("Pre-install hook failed for '{}': {}", pkg.name, e));
    }

    let installed_files = run_install_flow(node, mode, m, build_type, yes)?;

    if let types::InstallReason::Dependency { ref parent } = node.reason {
        let package_dir = local::get_package_dir(pkg.scope, handle, &pkg.repo, &pkg.name)?;
        local::add_dependent(&package_dir, parent)?;
    }

    if let Err(e) = post_install::install_manual_if_available(pkg, version, handle) {
        eprintln!(
            "Warning: failed to install manual for '{}': {}",
            pkg.name, e
        );
    }

    let manifest = manifest::create_manifest(
        pkg,
        node.reason.clone(),
        vec![],
        Some("prebuilt-archive".to_string()),
        installed_files,
        handle,
        &node.chosen_options,
        &node.chosen_optionals,
    )?;

    local::write_manifest(&manifest)?;

    if let Err(e) = recorder::record_package(
        pkg,
        &node.reason,
        &[],
        handle,
        &node.chosen_options,
        &node.chosen_optionals,
    ) {
        eprintln!(
            "Warning: failed to record package installation for '{}': {}",
            pkg.name, e
        );
    }

    if let Some(hooks) = &pkg.hooks
        && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PostInstall)
    {
        return Err(anyhow!(
            "Post-install hook failed for '{}': {}",
            pkg.name,
            e
        ));
    }

    util::send_telemetry("install", pkg, handle);

    Ok(manifest)
}

fn run_install_flow(
    node: &InstallNode,
    mode: InstallMode,
    m: Option<&MultiProgress>,
    build_type: Option<&str>,
    yes: bool,
) -> Result<Vec<String>> {
    let pkg = &node.pkg;
    let pkg_lua_path = Path::new(&node.source);
    let platform = crate::utils::get_platform()?;
    let config = config::read_config()?;

    let signature_policy = config.policy.signature_enforcement.filter(|p| p.enable);

    if mode == InstallMode::PreferPrebuilt {
        let db_path = resolve::get_db_root()?;
        let repo_db_path = db_path.join(&node.registry_handle);
        if let Ok(repo_config) = config::read_repo_config(&repo_db_path) {
            let mut pkg_links_to_try = Vec::new();
            if let Some(main_pkg) = repo_config.pkg.iter().find(|p| p.link_type == "main") {
                pkg_links_to_try.push(main_pkg.clone());
            }
            pkg_links_to_try.extend(
                repo_config
                    .pkg
                    .iter()
                    .filter(|p| p.link_type == "mirror")
                    .cloned(),
            );

            for pkg_link in pkg_links_to_try {
                let (os, arch) = (
                    platform.split('-').next().unwrap_or(""),
                    platform.split('-').nth(1).unwrap_or(""),
                );
                let url_dir = pkg_link
                    .url
                    .replace("{os}", os)
                    .replace("{arch}", arch)
                    .replace("{version}", &node.version)
                    .replace("{repo}", &pkg.repo);

                let archive_filename =
                    format!("{}-{}-{}.pkg.tar.zst", pkg.name, &node.version, platform);
                let final_url = format!("{}/{}", url_dir.trim_end_matches('/'), archive_filename);

                let archive_cache_root = cache::get_archive_cache_root()?;
                fs::create_dir_all(&archive_cache_root)?;
                let cached_archive_path = archive_cache_root.join(&archive_filename);

                let temp_dir = tempfile::Builder::new().prefix("zoi-dl-").tempdir()?;

                if let Some(policy) = &signature_policy {
                    println!("Signature enforcement is active.");
                    let pgp_url = match &pkg_link.pgp {
                        Some(url) => url
                            .replace("{os}", os)
                            .replace("{arch}", arch)
                            .replace("{version}", &node.version)
                            .replace("{repo}", &pkg.repo),
                        None => {
                            println!(
                                "Skipping source '{}' as it does not have a PGP signature URL.",
                                pkg_link.url
                            );
                            continue;
                        }
                    };

                    let temp_archive_path = temp_dir.path().join(&archive_filename);
                    if util::download_file_with_progress(&final_url, &temp_archive_path, m).is_err()
                    {
                        println!(
                            "Failed to download archive from {}. Trying next source.",
                            final_url
                        );
                        continue;
                    }

                    let sig_filename = format!("{}.sig", archive_filename);
                    let temp_sig_path = temp_dir.path().join(sig_filename);
                    if util::download_file_with_progress(&pgp_url, &temp_sig_path, m).is_err() {
                        println!(
                            "Failed to download signature from {}. Trying next source.",
                            pgp_url
                        );
                        continue;
                    }

                    println!("Verifying signature...");
                    let trusted_certs =
                        pgp::get_certs_by_name_or_fingerprint(&policy.trusted_keys)?;
                    match pgp::verify_detached_signature_multi_key(
                        &temp_archive_path,
                        &temp_sig_path,
                        trusted_certs,
                    ) {
                        Ok(_) => {
                            println!("{}", "Signature verified successfully.".green());
                            fs::copy(&temp_archive_path, &cached_archive_path)?;
                        }
                        Err(e) => {
                            println!(
                                "{} {}. Trying next source.",
                                "Signature verification failed:".red(),
                                e
                            );
                            continue;
                        }
                    }
                } else {
                    let mut expected_hash: Option<String> = None;
                    if let Some(hash_template) = &pkg_link.hash {
                        let hash_url = hash_template
                            .replace("{os}", os)
                            .replace("{arch}", arch)
                            .replace("{version}", &node.version)
                            .replace("{repo}", &pkg.repo);

                        match util::get_expected_hash(&hash_url) {
                            Ok(h) => {
                                if !h.is_empty() {
                                    expected_hash = Some(h);
                                }
                            }
                            Err(e) => {
                                println!("Warning: could not get hash from {}: {}", hash_url, e)
                            }
                        }
                    }

                    if cached_archive_path.exists() {
                        println!("Found cached archive: {}", cached_archive_path.display());
                        if let Some(hash) = &expected_hash {
                            match util::verify_file_hash(&cached_archive_path, hash) {
                                Ok(true) => {}
                                Ok(false) => {
                                    println!("Cached archive hash is invalid. Re-downloading.");
                                    fs::remove_file(&cached_archive_path)?;
                                }
                                Err(e) => {
                                    println!(
                                        "Could not verify hash of cached archive: {}. Re-downloading.",
                                        e
                                    );
                                    fs::remove_file(&cached_archive_path)?;
                                }
                            }
                        }
                    }

                    if !cached_archive_path.exists() {
                        let temp_download_path = temp_dir.path().join(&archive_filename);
                        if util::download_file_with_progress(&final_url, &temp_download_path, m)
                            .is_ok()
                        {
                            if let Some(hash) = &expected_hash {
                                match util::verify_file_hash(&temp_download_path, hash) {
                                    Ok(true) => {
                                        fs::copy(&temp_download_path, &cached_archive_path)?;
                                    }
                                    Ok(false) => {
                                        println!(
                                            "Downloaded archive hash is invalid. Trying next source."
                                        );
                                        continue;
                                    }
                                    Err(e) => {
                                        println!(
                                            "Could not verify hash of downloaded archive: {}. Trying next source.",
                                            e
                                        );
                                        continue;
                                    }
                                }
                            } else {
                                fs::copy(&temp_download_path, &cached_archive_path)?;
                            }
                        } else {
                            println!(
                                "Failed to download from {}. Trying next mirror if available.",
                                final_url
                            );
                            continue;
                        }
                    }
                }

                println!("Using archive: {}", cached_archive_path.display());
                if let Ok(installed_files) = crate::pkg::package::install::run(
                    &cached_archive_path,
                    Some(pkg.scope),
                    &node.registry_handle,
                    Some(&node.version),
                    yes,
                ) {
                    println!("Successfully installed pre-built package.");
                    return Ok(installed_files);
                } else {
                    println!("Failed to install from archive. Trying next source.");
                }
            }
        }
    }

    if signature_policy.is_some() {
        return Err(anyhow!(
            "Signature enforcement is active and no valid signed pre-built package was found."
        ));
    }

    println!(
        "{}",
        "Could not install pre-built package. Building from source...".yellow()
    );
    prebuilt::try_build_install(pkg_lua_path, pkg, &node.registry_handle, build_type, yes)
}
