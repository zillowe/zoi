use super::{manifest, methods, post_install, prebuilt, util, verification};
use crate::pkg::{
    config, config_handler, dependencies, library, local, recorder, resolve, rollback, service,
    types,
};
use crate::utils;
use anyhow::Result;
use colored::*;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use std::collections::HashSet;
use std::error::Error;

use std::process::Command;

#[derive(PartialEq, Eq, Clone)]
pub enum InstallMode {
    PreferBinary,
    ForceSource,
    Interactive,
    Updater(String),
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
    let (mut pkg, version, sharable_manifest, pkg_lua_path) =
        resolve::resolve_package_and_version(source)?;

    if let Some(scope) = scope_override {
        pkg.scope = scope;
    }

    utils::print_repo_warning(&pkg.repo);
    utils::check_license(&pkg.license);

    if !util::display_updates(&pkg, yes)? {
        println!("Operation aborted.");
        return Ok(());
    }

    if let Some(manifest) = local::is_package_installed(&pkg.name, pkg.scope)?
        && manifest.version != version
    {
        let config = config::read_config()?;
        let rollback_enabled = pkg.rollback.unwrap_or(config.rollback_enabled);
        if rollback_enabled {
            rollback::backup_package(&pkg.name, pkg.scope)?;
        }
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
        manifest::write_manifest(&pkg, reason, installed_deps_list, None, Vec::new())?;
        if let Err(e) = recorder::record_package(&pkg, &chosen_options, &chosen_optionals) {
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
        manifest::write_manifest(&pkg, reason, installed_deps_list, None, Vec::new())?;
        if let Err(e) = recorder::record_package(&pkg, &chosen_options, &chosen_optionals) {
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
        manifest::write_manifest(&pkg, reason, installed_deps_list, None, Vec::new())?;
        if let Err(e) = recorder::record_package(&pkg, &chosen_options, &chosen_optionals) {
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
        && manifest.reason == types::InstallReason::Dependency
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
        if pkg.package_type == types::PackageType::Service
            && utils::ask_for_confirmation("Do you want to start the service?", yes)
        {
            service::start_service(&pkg)?;
        }
        return Ok(());
    }

    println!("Installing '{}' version '{}'", pkg.name, version);

    if sharable_manifest.is_none()
        && let Some(deps) = &pkg.dependencies
    {
        let platform = utils::get_platform()?;
        let should_include_build = match mode {
            InstallMode::ForceSource => true,
            InstallMode::PreferBinary | InstallMode::Interactive | InstallMode::Updater(_) => {
                util::find_method(&pkg, "source", &platform).is_some()
                    && util::find_method(&pkg, "binary", &platform).is_none()
                    && util::find_method(&pkg, "com_binary", &platform).is_none()
            }
        };

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

    let mut install_method_name = None;
    let mut installed_files = Vec::new();

    let result = match mode {
        InstallMode::ForceSource => {
            install_method_name = Some("source".to_string());
            run_source_flow(&pkg, &platform)
        }
        InstallMode::PreferBinary => run_default_flow(
            &pkg,
            &pkg_lua_path,
            &platform,
            yes,
            &mut install_manual,
            &mut install_method_name,
            &mut installed_files,
        ),
        InstallMode::Interactive => run_interactive_flow(
            &pkg,
            &platform,
            &mut install_method_name,
            &mut installed_files,
        ),
        InstallMode::Updater(ref method_name) => {
            install_method_name = Some(method_name.clone());
            run_updater_flow(&pkg, &platform, method_name, &mut installed_files)
        }
    };

    if result.is_ok() {
        if install_manual && let Err(e) = post_install::install_manual_if_available(&pkg) {
            eprintln!("Warning: failed to install manual: {}", e);
        }
        if pkg.package_type == types::PackageType::Library
            && let Err(e) = library::install_pkg_config_file(&pkg)
        {
            eprintln!("Warning: failed to install pkg-config file: {}", e);
        }
        manifest::write_manifest(
            &pkg,
            reason,
            installed_deps_list,
            install_method_name,
            installed_files,
        )?;
        if let Err(e) = recorder::record_package(&pkg, &chosen_options, &chosen_optionals) {
            eprintln!("Warning: failed to record package installation: {}", e);
        }
        if let Err(e) = utils::setup_path(pkg.scope) {
            eprintln!("{} Failed to configure PATH: {}", "Warning:".yellow(), e);
        }
        let event_name = match mode {
            InstallMode::ForceSource => "build",
            _ => "install",
        };
        util::send_telemetry(event_name, &pkg);
        if pkg.package_type == types::PackageType::Service
            && utils::ask_for_confirmation("Do you want to start the service now?", yes)
        {
            service::start_service(&pkg)?;
        }
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
    }

    result
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

fn run_updater_flow(
    pkg: &types::Package,
    platform: &str,
    method_name: &str,
    installed_files: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    if let Some(method) = util::find_method(pkg, method_name, platform) {
        println!("Using '{}' method specified by updater.", method_name);
        return match method_name {
            "binary" => methods::binary::handle_binary_install(method, pkg),
            "installer" => {
                methods::installer::handle_installer_install(method, pkg, installed_files)
            }
            "com_binary" => methods::com_binary::handle_com_binary_install(method, pkg),
            "script" => methods::script::handle_script_install(method, pkg),
            "source" => methods::source::handle_source_install(method, pkg),
            _ => Err(format!(
                "Invalid installation method '{}' specified in updater.",
                method_name
            )
            .into()),
        };
    }
    Err(format!(
        "Specified updater method '{}' not found or not compatible.",
        method_name
    )
    .into())
}

fn run_interactive_flow(
    pkg: &types::Package,
    platform: &str,
    install_method_name: &mut Option<String>,
    installed_files: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let mut available_methods = Vec::new();
    for method in &pkg.installation {
        if crate::utils::is_platform_compatible(platform, &method.platforms) {
            available_methods.push(method);
        }
    }

    if available_methods.is_empty() {
        return Err("No compatible installation methods found for your platform.".into());
    }

    let method_names: Vec<String> = available_methods
        .iter()
        .map(|m| {
            if let Some(name) = &m.name {
                format!("{} ({})", name, m.install_type)
            } else {
                m.install_type.clone()
            }
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select an installation method")
        .items(&method_names)
        .default(0)
        .interact()?;

    let selected_method = available_methods[selection];

    *install_method_name = Some(selected_method.install_type.clone());
    match selected_method.install_type.as_str() {
        "binary" => methods::binary::handle_binary_install(selected_method, pkg),
        "installer" => {
            methods::installer::handle_installer_install(selected_method, pkg, installed_files)
        }
        "com_binary" => methods::com_binary::handle_com_binary_install(selected_method, pkg),
        "script" => methods::script::handle_script_install(selected_method, pkg),
        "source" => methods::source::handle_source_install(selected_method, pkg),
        _ => Err("Invalid installation method selected.".into()),
    }
}

fn run_default_flow(
    pkg: &types::Package,
    pkg_lua_path: &std::path::Path,
    platform: &str,
    yes: bool,
    install_manual: &mut bool,
    install_method_name: &mut Option<String>,
    installed_files: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
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
                if let Some(pgp_url_template) = &pkg_link.pgp {
                    let pgp_url = pgp_url_template
                        .replace("{os}", os)
                        .replace("{arch}", arch)
                        .replace("{version}", pkg.version.as_deref().unwrap_or(""))
                        .replace("{repo}", &pkg.repo);

                    println!("Downloading signature from: {}", pgp_url.cyan());
                    match util::download_file_with_progress(&pgp_url) {
                        Ok(sig_bytes) => {
                            if let Err(e) = verification::verify_prebuilt_signature(
                                &downloaded_data,
                                &sig_bytes,
                            ) {
                                return Err(format!(
                                    "Signature verification failed for {}: {}",
                                    final_url, e
                                )
                                .into());
                            }
                        }
                        Err(e) => {
                            return Err(format!(
                                "Failed to download PGP signature from {}: {}",
                                pgp_url, e
                            )
                            .into());
                        }
                    }
                } else {
                    println!(
                        "{}",
                        "No PGP signature configured for this pre-built package source. Skipping verification."
                            .yellow()
                    );
                }

                let temp_dir = tempfile::Builder::new().prefix("zoi-prebuilt").tempdir()?;
                let temp_archive_path = temp_dir.path().join(&archive_filename);
                std::fs::write(&temp_archive_path, downloaded_data)?;

                println!("Successfully downloaded pre-built package.");
                if crate::pkg::package::install::run(&temp_archive_path, Some(pkg.scope)).is_ok() {
                    println!("Successfully installed pre-built package.");
                    *install_manual = false;
                    return Ok(());
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

    println!(
        "{}",
        "Could not install pre-built package. Trying meta-build-install flow...".yellow()
    );
    if let Err(e) = prebuilt::try_meta_build_install(pkg_lua_path, pkg) {
        println!(
            "meta-build-install flow failed: {}. Falling back to other methods.",
            e.to_string().yellow()
        );
    } else {
        println!("{}", "meta-build-install flow successful.".green());
        *install_manual = false;
        return Ok(());
    }

    if let Some(method) = util::find_method(pkg, "installer", platform) {
        println!("Found 'installer' method. Installing...");
        *install_method_name = Some("installer".to_string());
        return methods::installer::handle_installer_install(method, pkg, installed_files);
    }

    if let Some(method) = util::find_method(pkg, "binary", platform) {
        println!("Found 'binary' method. Installing...");
        *install_method_name = Some("binary".to_string());
        return methods::binary::handle_binary_install(method, pkg);
    }

    println!("No binary found, checking for compressed binary...");
    if let Some(method) = util::find_method(pkg, "com_binary", platform) {
        println!("Found 'com_binary' method. Installing...");
        *install_method_name = Some("com_binary".to_string());
        return methods::com_binary::handle_com_binary_install(method, pkg);
    }

    println!("No compressed binary found, checking for script...");
    if let Some(method) = util::find_method(pkg, "script", platform)
        && utils::ask_for_confirmation("Found a 'script' method. Do you want to execute it?", yes)
    {
        *install_method_name = Some("script".to_string());
        return methods::script::handle_script_install(method, pkg);
    }

    println!("No script found, checking for source...");
    if let Some(method) = util::find_method(pkg, "source", platform)
        && utils::ask_for_confirmation(
            "Found a 'source' method. Do you want to build from source?",
            yes,
        )
    {
        *install_method_name = Some("source".to_string());
        return methods::source::handle_source_install(method, pkg);
    }

    Err("No compatible and accepted installation method found for your platform.".into())
}

fn run_source_flow(pkg: &types::Package, platform: &str) -> Result<(), Box<dyn Error>> {
    if let Some(method) = util::find_method(pkg, "source", platform) {
        return methods::source::handle_source_install(method, pkg);
    }
    Err("No compatible 'source' installation method found.".into())
}
