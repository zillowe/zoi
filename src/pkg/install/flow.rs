use super::{manifest, post_install, prebuilt, util};
use crate::pkg::{config, config_handler, dependencies, local, recorder, resolve, types};
use crate::utils;
use anyhow::Result;
use colored::*;
use std::collections::HashSet;
use std::error::Error;

use std::process::Command;

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
    processed_deps: &mut HashSet<String>,
    scope_override: Option<types::Scope>,
) -> Result<(), Box<dyn Error>> {
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
        return Err("This package is an 'app' template. Use 'zoi create pkg <source> <appName>' to create an app from it.".into());
    }

    if pkg.scope == types::Scope::System {
        if !utils::is_admin() {
            return Err("System-wide installation requires administrative privileges. Please run with sudo or as an administrator.".into());
        }
        if !utils::ask_for_confirmation(
            "This package will be installed system-wide. Are you sure you want to continue?",
            yes,
        ) {
            return Err("Operation aborted by user.".into());
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
    } else if pkg.package_type == types::PackageType::Config {
        println!("Installing configuration '{}'...", pkg.name.bold());
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
                )?;
            }
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

        println!("Configuration '{}' registered.", pkg.name.green());

        util::send_telemetry("install", &pkg);

        if utils::ask_for_confirmation("Do you want to run the setup commands now?", yes) {
            config_handler::run_install_commands(&pkg)?;
        }
        return Ok(());
    } else if pkg.package_type == types::PackageType::Script {
        println!("Running script '{}'...", pkg.name.bold());
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
                )?;
            }
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
        println!("Script '{}' registered.", pkg.name.green());

        util::send_telemetry("install", &pkg);

        if utils::ask_for_confirmation("Do you want to run the script now?", yes) {
            run_script_install_commands(&pkg)?;
        }
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
            )?;
        }
    }

    let platform = utils::get_platform()?;
    println!("Current platform: {}", &platform);

    let mut install_manual = true;

    let result = run_default_flow(
        &pkg,
        &pkg_lua_path,
        &platform,
        &mut install_manual,
        mode,
        handle,
    );

    match result {
        Ok(installed_files) => {
            let version_dir =
                local::get_package_version_dir(pkg.scope, handle, &pkg.repo, &pkg.name, &version)?;
            if let types::InstallReason::Dependency { ref parent } = reason {
                let package_dir = version_dir.parent().ok_or("Invalid version dir")?;
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

            util::send_telemetry("install", &pkg);

            if pkg.post_install.is_some()
                && utils::ask_for_confirmation(
                    "This package has post-installation commands. Do you want to run them?",
                    yes,
                )
                && let Err(e) = post_install::run_post_install_hooks(&pkg)
            {
                eprintln!(
                    "{} Post-installation commands failed: {}",
                    "Warning:".yellow(),
                    e
                );
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn run_script_install_commands(pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    if let Some(hooks) = &pkg.script {
        println!("\n{}", "Running script...".bold());
        let platform = utils::get_platform()?;
        let _version = pkg.version.as_deref().unwrap_or("");

        for hook in hooks {
            if utils::is_platform_compatible(&platform, &hook.platforms) {
                for cmd_str in &hook.install {
                    println!("Executing: {}", cmd_str.cyan());

                    let output = if cfg!(target_os = "windows") {
                        Command::new("pwsh").arg("-Command").arg(cmd_str).output()?
                    } else {
                        Command::new("bash").arg("-c").arg(cmd_str).output()?
                    };

                    if !output.status.success() {
                        return Err(format!("Script command failed: '{}'", cmd_str).into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn run_default_flow(
    pkg: &types::Package,
    pkg_lua_path: &std::path::Path,
    platform: &str,
    install_manual: &mut bool,
    mode: InstallMode,
    registry_handle: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    if mode == InstallMode::PreferPrebuilt {
        let db_path = resolve::get_db_root()?;
        if let Ok(repo_config) = config::read_repo_config(&db_path) {
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

                let archive_filename = format!("{}.pkg.tar.zst", pkg.name);
                let final_url = format!("{}/{}", url_dir.trim_end_matches('/'), archive_filename);

                println!(
                    "Attempting to download pre-built package from: {}",
                    final_url.cyan()
                );

                if let Ok(downloaded_data) = util::download_file_with_progress(&final_url) {
                    let temp_dir = tempfile::Builder::new().prefix("zoi-prebuilt").tempdir()?;
                    let temp_archive_path = temp_dir.path().join(&archive_filename);
                    std::fs::write(&temp_archive_path, downloaded_data)?;

                    println!("Successfully downloaded pre-built package.");
                    if let Ok(installed_files) = crate::pkg::package::install::run(
                        &temp_archive_path,
                        Some(pkg.scope),
                        registry_handle,
                    ) {
                        println!("Successfully installed pre-built package.");
                        *install_manual = false;
                        return Ok(installed_files);
                    } else {
                        println!("Failed to install downloaded package. Trying next source.");
                    }
                } else {
                    println!(
                        "Failed to download from {}. Trying next mirror if available.",
                        final_url
                    );
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
