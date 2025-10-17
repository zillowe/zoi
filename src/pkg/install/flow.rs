use super::{manifest, post_install, prebuilt, util};
use crate::pkg::{cache, config, dependencies, hooks, local, recorder, resolve, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use indicatif::MultiProgress;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(PartialEq, Eq, Clone)]
pub enum InstallMode {
    PreferPrebuilt,
    ForceBuild,
}

pub fn run_installation(
    source: &str,
    mode: InstallMode,
    force: bool,
    reason: types::InstallReason,
    yes: bool,
    all_optional: bool,
    processed_deps: &Mutex<HashSet<String>>,
    scope_override: Option<types::Scope>,
    m: Option<&MultiProgress>,
) -> Result<()> {
    let (mut pkg, version, sharable_manifest, pkg_lua_path, registry_handle) =
        resolve::resolve_package_and_version(source)?;

    let handle = registry_handle.as_deref().unwrap_or("local");

    if let Some(scope) = scope_override {
        pkg.scope = scope;
    }

    utils::print_repo_warning(&pkg.repo);
    utils::check_license(&pkg.license);

    if !util::display_updates(&pkg, yes)? {
        println!("Operation aborted.");
        return Ok(());
    }

    util::check_for_conflicts(&pkg, yes)?;

    if pkg.package_type == types::PackageType::App {
        return Err(anyhow!(
            "This package is an 'app' template. Use 'zoi create pkg <source> <appName>' to create an app from it."
        ));
    }

    if pkg.scope == types::Scope::System {
        if !utils::is_admin() {
            return Err(anyhow!(
                "System-wide installation requires administrative privileges. Please run with sudo or as an administrator."
            ));
        }
        if !utils::ask_for_confirmation(
            "This package will be installed system-wide. Are you sure you want to continue?",
            yes,
        ) {
            return Err(anyhow!("Operation aborted by user."));
        }
    }

    let mut installed_deps_list = Vec::new();
    let mut chosen_options = Vec::new();
    let mut chosen_optionals = Vec::new();

    if let Some(sm) = &sharable_manifest {
        dependencies::resolve_and_install_required(
            &sm.chosen_options,
            &pkg.name,
            &version,
            pkg.scope,
            true,
            true,
            processed_deps,
            &mut installed_deps_list,
            m,
        )?;
        dependencies::resolve_and_install_required(
            &sm.chosen_optionals,
            &pkg.name,
            &version,
            pkg.scope,
            true,
            true,
            processed_deps,
            &mut installed_deps_list,
            m,
        )?;
        chosen_options = sm.chosen_options.clone();
        chosen_optionals = sm.chosen_optionals.clone();
    } else if pkg.package_type == types::PackageType::Collection {
        println!("Installing package collection '{}'...", pkg.name.bold());
        if let Some(deps) = &pkg.dependencies {
            if let Some(runtime_deps) = &deps.runtime {
                dependencies::resolve_and_install_required(
                    &runtime_deps.get_required_simple(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
                    all_optional,
                    processed_deps,
                    &mut installed_deps_list,
                    m,
                )?;
                dependencies::resolve_and_install_required_options(
                    &runtime_deps.get_required_options(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
                    all_optional,
                    processed_deps,
                    &mut installed_deps_list,
                    &mut chosen_options,
                    m,
                )?;
                dependencies::resolve_and_install_optional(
                    runtime_deps.get_optional(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
                    all_optional,
                    processed_deps,
                    &mut installed_deps_list,
                    &mut chosen_optionals,
                    Some("runtime"),
                    m,
                )?;
            }

            if let Some(build_deps) = &deps.build {
                dependencies::resolve_and_install_required(
                    &build_deps.get_required_simple(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
                    all_optional,
                    processed_deps,
                    &mut installed_deps_list,
                    m,
                )?;
                dependencies::resolve_and_install_required_options(
                    &build_deps.get_required_options(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
                    all_optional,
                    processed_deps,
                    &mut installed_deps_list,
                    &mut chosen_options,
                    m,
                )?;
                dependencies::resolve_and_install_optional(
                    build_deps.get_optional(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
                    all_optional,
                    processed_deps,
                    &mut installed_deps_list,
                    &mut chosen_optionals,
                    Some("build"),
                    m,
                )?;
            }
        } else {
            println!("Collection has no dependencies to install.");
        }
        manifest::write_manifest(
            &pkg,
            reason.clone(),
            installed_deps_list.clone(),
            None,
            Vec::new(),
            handle,
            &chosen_options,
            &chosen_optionals,
        )?;
        if let Err(e) = recorder::record_package(
            &pkg,
            &reason,
            &installed_deps_list,
            handle,
            &chosen_options,
            &chosen_optionals,
        ) {
            eprintln!("Warning: failed to record package installation: {}", e);
        }
        println!("Collection '{}' installed successfully.", pkg.name.green());
        util::send_telemetry("install", &pkg);
        return Ok(());
    }

    if let Some(mut manifest) = local::is_package_installed(&pkg.name, pkg.scope)?
        && matches!(manifest.reason, types::InstallReason::Dependency { .. })
        && reason == types::InstallReason::Direct
    {
        println!("Updating package '{}' to be directly managed.", pkg.name);
        manifest.reason = types::InstallReason::Direct;
        local::write_manifest(&manifest)?;
    }

    if !force && let Some(manifest) = local::is_package_installed(&pkg.name, pkg.scope)? {
        println!(
            "{}",
            format!(
                "Package '{}' version {} is already installed.",
                pkg.name, manifest.version
            )
            .yellow()
        );
        return Ok(());
    }

    println!("Installing '{}' version '{}'", pkg.name, version);

    if sharable_manifest.is_none()
        && let Some(deps) = &pkg.dependencies
    {
        let should_include_build = mode == InstallMode::ForceBuild;

        if should_include_build && let Some(build_deps) = &deps.build {
            dependencies::resolve_and_install_required(
                &build_deps.get_required_simple(),
                &pkg.name,
                &version,
                pkg.scope,
                yes,
                all_optional,
                processed_deps,
                &mut installed_deps_list,
                m,
            )?;
            dependencies::resolve_and_install_required_options(
                &build_deps.get_required_options(),
                &pkg.name,
                &version,
                pkg.scope,
                yes,
                all_optional,
                processed_deps,
                &mut installed_deps_list,
                &mut chosen_options,
                m,
            )?;
            dependencies::resolve_and_install_optional(
                build_deps.get_optional(),
                &pkg.name,
                &version,
                pkg.scope,
                yes,
                all_optional,
                processed_deps,
                &mut installed_deps_list,
                &mut chosen_optionals,
                Some("build"),
                m,
            )?;
        }

        if let Some(runtime_deps) = &deps.runtime {
            dependencies::resolve_and_install_required(
                &runtime_deps.get_required_simple(),
                &pkg.name,
                &version,
                pkg.scope,
                yes,
                all_optional,
                processed_deps,
                &mut installed_deps_list,
                m,
            )?;
            dependencies::resolve_and_install_required_options(
                &runtime_deps.get_required_options(),
                &pkg.name,
                &version,
                pkg.scope,
                yes,
                all_optional,
                processed_deps,
                &mut installed_deps_list,
                &mut chosen_options,
                m,
            )?;
            dependencies::resolve_and_install_optional(
                runtime_deps.get_optional(),
                &pkg.name,
                &version,
                pkg.scope,
                yes,
                all_optional,
                processed_deps,
                &mut installed_deps_list,
                &mut chosen_optionals,
                Some("runtime"),
                m,
            )?;
        }
    }

    let platform = utils::get_platform()?;
    println!("Current platform: {}", &platform);

    if let Some(hooks) = &pkg.hooks
        && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PreInstall)
    {
        return Err(anyhow!("Pre-install hook failed: {}", e));
    }

    let mut install_manual = true;

    let result = run_default_flow(
        &pkg,
        &pkg_lua_path,
        &platform,
        &mut install_manual,
        mode,
        handle,
        m,
    );

    match result {
        Ok(installed_files) => {
            let version_dir =
                local::get_package_version_dir(pkg.scope, handle, &pkg.repo, &pkg.name, &version)?;
            if let types::InstallReason::Dependency { ref parent } = reason {
                let package_dir = version_dir
                    .parent()
                    .ok_or_else(|| anyhow!("Invalid version dir"))?;
                local::add_dependent(package_dir, parent)?;
            }
            if install_manual
                && let Err(e) = post_install::install_manual_if_available(&pkg, &version, handle)
            {
                eprintln!("Warning: failed to install manual: {}", e);
            }

            manifest::write_manifest(
                &pkg,
                reason.clone(),
                installed_deps_list.clone(),
                Some("prebuilt-archive".to_string()),
                installed_files,
                handle,
                &chosen_options,
                &chosen_optionals,
            )?;
            if let Err(e) = recorder::record_package(
                &pkg,
                &reason,
                &installed_deps_list,
                handle,
                &chosen_options,
                &chosen_optionals,
            ) {
                eprintln!("Warning: failed to record package installation: {}", e);
            }
            if let Err(e) = utils::setup_path(pkg.scope) {
                eprintln!("{} Failed to configure PATH: {}", "Warning:".yellow(), e);
            }

            if let Some(hooks) = &pkg.hooks
                && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PostInstall)
            {
                return Err(anyhow!("Post-install hook failed: {}", e));
            }

            util::send_telemetry("install", &pkg);

            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn run_default_flow(
    pkg: &types::Package,
    pkg_lua_path: &std::path::Path,
    platform: &str,
    install_manual: &mut bool,
    mode: InstallMode,
    registry_handle: &str,
    m: Option<&MultiProgress>,
) -> Result<Vec<String>> {
    if mode == InstallMode::PreferPrebuilt {
        let db_path = resolve::get_db_root()?;
        let repo_db_path = db_path.join(registry_handle);
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
                    .replace("{version}", pkg.version.as_deref().unwrap_or(""))
                    .replace("{repo}", &pkg.repo);

                let archive_filename = format!(
                    "{}-{}-{}.pkg.tar.zst",
                    pkg.name,
                    pkg.version.as_deref().unwrap_or(""),
                    platform
                );
                let final_url = format!("{}/{}", url_dir.trim_end_matches('/'), archive_filename);

                let archive_cache_root = cache::get_archive_cache_root()?;
                fs::create_dir_all(&archive_cache_root)?;
                let cached_archive_path = archive_cache_root.join(&archive_filename);

                let mut expected_hash: Option<String> = None;
                if let Some(hash_template) = &pkg_link.hash {
                    let hash_url = hash_template
                        .replace("{os}", os)
                        .replace("{arch}", arch)
                        .replace("{version}", pkg.version.as_deref().unwrap_or(""))
                        .replace("{repo}", &pkg.repo);

                    match util::get_expected_hash(&hash_url) {
                        Ok(h) => {
                            if !h.is_empty() {
                                expected_hash = Some(h);
                            }
                        }
                        Err(e) => println!("Warning: could not get hash from {}: {}", hash_url, e),
                    }
                }

                let mut archive_to_install: Option<PathBuf> = None;

                if cached_archive_path.exists() {
                    println!("Found cached archive: {}", cached_archive_path.display());
                    if let Some(hash) = &expected_hash {
                        match util::verify_file_hash(&cached_archive_path, hash) {
                            Ok(true) => {
                                archive_to_install = Some(cached_archive_path.clone());
                            }
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
                    } else {
                        archive_to_install = Some(cached_archive_path.clone());
                    }
                }

                if archive_to_install.is_none() {
                    let temp_dir = tempfile::Builder::new().prefix("zoi-dl-").tempdir()?;
                    let temp_download_path = temp_dir.path().join(&archive_filename);

                    if util::download_file_with_progress(&final_url, &temp_download_path, m).is_ok()
                    {
                        if let Some(hash) = &expected_hash {
                            match util::verify_file_hash(&temp_download_path, hash) {
                                Ok(true) => {
                                    fs::copy(&temp_download_path, &cached_archive_path)?;
                                    archive_to_install = Some(cached_archive_path.clone());
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
                            archive_to_install = Some(cached_archive_path.clone());
                        }
                    } else {
                        println!(
                            "Failed to download from {}. Trying next mirror if available.",
                            final_url
                        );
                        continue;
                    }
                }

                if let Some(archive_path) = archive_to_install {
                    println!("Using archive: {}", archive_path.display());
                    if let Ok(installed_files) = crate::pkg::package::install::run(
                        &archive_path,
                        Some(pkg.scope),
                        registry_handle,
                    ) {
                        println!("Successfully installed pre-built package.");
                        *install_manual = false;
                        return Ok(installed_files);
                    } else {
                        println!("Failed to install from archive. Trying next source.");
                    }
                }
            }
        }
    }

    println!(
        "{}",
        "Could not install pre-built package. Building from source...".yellow()
    );
    prebuilt::try_build_install(pkg_lua_path, pkg, registry_handle)
}
