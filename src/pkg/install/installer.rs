use crate::pkg::{
    cache, config, hooks,
    install::{manifest, plan, post_install, prebuilt, resolver::InstallNode, util},
    local, pgp, recorder, resolve, types,
};
use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::MultiProgress;
use std::fs;
use std::path::{Path, PathBuf};

pub fn download_and_cache_archive(
    node: &InstallNode,
    details: &plan::PrebuiltDetails,
    m: Option<&MultiProgress>,
) -> Result<PathBuf> {
    let _pkg = &node.pkg;
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

    if cached_archive_path.exists() {
        println!("Using cached archive: {}", cached_archive_path.display());
    } else {
        let temp_dir = tempfile::Builder::new().prefix("zoi-dl-").tempdir()?;
        let temp_archive_path = temp_dir.path().join(archive_filename);

        if util::download_file_with_progress(
            &details.info.final_url,
            &temp_archive_path,
            m,
            Some(details.download_size),
        )
        .is_err()
        {
            return Err(anyhow!("Failed to download archive"));
        }

        if let Some(hash_url) = &details.info.hash_url
            && let Ok(hash) = util::get_expected_hash(hash_url)
            && !hash.is_empty()
            && !util::verify_file_hash(&temp_archive_path, &hash).unwrap_or(false)
        {
            return Err(anyhow!("Hash verification failed"));
        }

        if let Some(policy) = &signature_policy {
            if let Some(pgp_url) = &details.info.pgp_url {
                let temp_sig_path = temp_dir.path().join(&sig_filename);
                if util::download_file_with_progress(pgp_url, &temp_sig_path, m, None).is_err() {
                    return Err(anyhow!("Failed to download signature"));
                }

                println!("Verifying signature...");
                let trusted_certs = pgp::get_certs_by_name_or_fingerprint(&policy.trusted_keys)?;
                if pgp::verify_detached_signature_multi_key(
                    &temp_archive_path,
                    &temp_sig_path,
                    trusted_certs,
                )
                .is_err()
                {
                    return Err(anyhow!("Signature verification failed"));
                }
                println!("{}", "Signature verified successfully.".green());
                fs::copy(&temp_sig_path, &cached_sig_path)?;
            } else {
                return Err(anyhow!(
                    "Signature enforcement is active, but no PGP URL found for package"
                ));
            }
        }

        fs::copy(&temp_archive_path, &cached_archive_path)?;
    }

    Ok(cached_archive_path)
}

pub fn install_node(
    node: &InstallNode,
    action: &plan::InstallAction,
    _m: Option<&MultiProgress>,
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

    let request = resolve::parse_source_string(&node.source)?;
    let sub_package_to_install = request.sub_package;

    let sub_packages_vec = sub_package_to_install.clone().map(|s| vec![s]);

    let installed_files = match action {
        plan::InstallAction::DownloadAndInstall(_) => {
            unreachable!("Download should have been handled before install_node")
        }
        plan::InstallAction::InstallFromArchive(archive_path) => crate::pkg::package::install::run(
            archive_path,
            Some(pkg.scope),
            &node.registry_handle,
            Some(&node.version),
            yes,
            sub_packages_vec,
        )?,
        plan::InstallAction::BuildAndInstall => {
            println!(
                "{}",
                "Could not find a pre-built package. Building from source...".yellow()
            );
            let pkg_lua_path = Path::new(&node.source);
            prebuilt::try_build_install(
                pkg_lua_path,
                pkg,
                &node.registry_handle,
                build_type,
                yes,
                sub_package_to_install.clone(),
            )?
        }
    };

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
        node.dependencies.clone(),
        Some("prebuilt-archive".to_string()),
        installed_files,
        handle,
        &node.chosen_options,
        &node.chosen_optionals,
        sub_package_to_install.clone(),
    )?;

    local::write_manifest(&manifest)?;

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
