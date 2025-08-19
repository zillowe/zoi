use crate::pkg::{
    config, config_handler, dependencies, library, local, recorder, resolve, rollback, service,
    types,
};
use crate::utils;
use anyhow::Result;
use chrono::Utc;
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use sequoia_openpgp::{
    KeyHandle,
    cert::Cert,
    parse::{
        Parse,
        stream::{DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationHelper},
    },
    policy::StandardPolicy,
};
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashSet;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::PathBuf;
use std::process::Command;
use tar::Archive;
use tempfile::Builder;
use tokio::runtime::Runtime;
use walkdir::WalkDir;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

#[derive(PartialEq, Eq, Clone)]
pub enum InstallMode {
    PreferBinary,
    ForceSource,
    Interactive,
    Updater(String),
}

fn send_telemetry(event: &str, pkg: &types::Package) {
    match crate::pkg::telemetry::posthog_capture_event(event, pkg, env!("CARGO_PKG_VERSION")) {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }
}

fn display_updates(pkg: &types::Package, yes: bool) -> Result<bool, Box<dyn Error>> {
    if let Some(updates) = &pkg.updates {
        if updates.is_empty() {
            return Ok(true);
        }
        println!("\n{}", "Important Updates:".bold().yellow());
        for update in updates {
            let type_str = match update.update_type {
                types::UpdateType::Change => "Change".blue(),
                types::UpdateType::Vulnerability => "Vulnerability".red().bold(),
                types::UpdateType::Update => "Update".green(),
            };
            println!("  - [{}] {}", type_str, update.message);
        }

        if !utils::ask_for_confirmation("\nDo you want to continue?", yes) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn run_installation(
    source: &str,
    mode: InstallMode,
    force: bool,
    reason: types::InstallReason,
    yes: bool,
    processed_deps: &mut HashSet<String>,
) -> Result<(), Box<dyn Error>> {
    let (pkg, version, sharable_manifest) = resolve::resolve_package_and_version(source)?;

    utils::check_license(&pkg.license);

    if !display_updates(&pkg, yes)? {
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

    check_for_conflicts(&pkg, yes)?;

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
            true, // yes = true
            processed_deps,
            &mut installed_deps_list,
        )?;
        dependencies::resolve_and_install_required(
            &sm.chosen_optionals,
            &pkg.name,
            &version,
            pkg.scope,
            true, // yes = true
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
                    processed_deps,
                    &mut installed_deps_list,
                )?;
                dependencies::resolve_and_install_required_options(
                    &runtime_deps.get_required_options(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
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
                    processed_deps,
                    &mut installed_deps_list,
                )?;
                dependencies::resolve_and_install_required_options(
                    &build_deps.get_required_options(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
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
                    processed_deps,
                    &mut installed_deps_list,
                    &mut chosen_optionals,
                    Some("build"),
                )?;
            }
        } else {
            println!("Collection has no dependencies to install.");
        }
        write_manifest(&pkg, reason, installed_deps_list)?;
        write_sharable_manifest(&pkg, chosen_options, chosen_optionals)?;
        if let Err(e) = recorder::record_package(&pkg) {
            eprintln!("Warning: failed to record package installation: {}", e);
        }
        println!("Collection '{}' installed successfully.", pkg.name.green());
        send_telemetry("install", &pkg);
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
                    processed_deps,
                    &mut installed_deps_list,
                )?;
                dependencies::resolve_and_install_required_options(
                    &runtime_deps.get_required_options(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
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
                    processed_deps,
                    &mut installed_deps_list,
                )?;
                dependencies::resolve_and_install_required_options(
                    &build_deps.get_required_options(),
                    &pkg.name,
                    &version,
                    pkg.scope,
                    yes,
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
                    processed_deps,
                    &mut installed_deps_list,
                    &mut chosen_optionals,
                    Some("build"),
                )?;
            }
        }
        write_manifest(&pkg, reason, installed_deps_list)?;
        write_sharable_manifest(&pkg, chosen_options, chosen_optionals)?;
        if let Err(e) = recorder::record_package(&pkg) {
            eprintln!("Warning: failed to record package installation: {}", e);
        }
        println!("Configuration '{}' registered.", pkg.name.green());

        send_telemetry("install", &pkg);

        if utils::ask_for_confirmation("Do you want to run the setup commands now?", yes) {
            config_handler::run_install_commands(&pkg)?;
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
                find_method(&pkg, "source", &platform).is_some()
                    && find_method(&pkg, "binary", &platform).is_none()
                    && find_method(&pkg, "com_binary", &platform).is_none()
            }
        };

        if should_include_build && let Some(build_deps) = &deps.build {
            dependencies::resolve_and_install_required(
                &build_deps.get_required_simple(),
                &pkg.name,
                &version,
                pkg.scope,
                yes,
                processed_deps,
                &mut installed_deps_list,
            )?;
            dependencies::resolve_and_install_required_options(
                &build_deps.get_required_options(),
                &pkg.name,
                &version,
                pkg.scope,
                yes,
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
                processed_deps,
                &mut installed_deps_list,
            )?;
            dependencies::resolve_and_install_required_options(
                &runtime_deps.get_required_options(),
                &pkg.name,
                &version,
                pkg.scope,
                yes,
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
                processed_deps,
                &mut installed_deps_list,
                &mut chosen_optionals,
                Some("runtime"),
            )?;
        }
    }

    let platform = utils::get_platform()?;
    println!("Current platform: {}", &platform);

    let result = match mode {
        InstallMode::ForceSource => run_source_flow(&pkg, &platform),
        InstallMode::PreferBinary => run_default_flow(&pkg, &platform, yes),
        InstallMode::Interactive => run_interactive_flow(&pkg, &platform),
        InstallMode::Updater(ref method_name) => run_updater_flow(&pkg, &platform, method_name),
    };

    if result.is_ok() {
        if pkg.package_type == types::PackageType::Library
            && let Err(e) = library::install_pkg_config_file(&pkg)
        {
            eprintln!("Warning: failed to install pkg-config file: {}", e);
        }
        write_manifest(&pkg, reason, installed_deps_list)?;
        write_sharable_manifest(&pkg, chosen_options, chosen_optionals)?;
        if let Err(e) = recorder::record_package(&pkg) {
            eprintln!("Warning: failed to record package installation: {}", e);
        }
        if let Err(e) = utils::setup_path(pkg.scope) {
            eprintln!("{} Failed to configure PATH: {}", "Warning:".yellow(), e);
        }
        let event_name = match mode {
            InstallMode::ForceSource => "build",
            _ => "install",
        };
        send_telemetry(event_name, &pkg);
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
            && let Err(e) = run_post_install_hooks(&pkg)
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

fn check_for_conflicts(pkg: &types::Package, yes: bool) -> Result<(), Box<dyn Error>> {
    let installed_packages = local::get_installed_packages()?;

    if pkg.conflicts.is_some() || pkg.bins.is_some() {
        let mut conflict_messages = Vec::new();

        if let Some(conflicts_with) = &pkg.conflicts {
            for conflict_pkg_name in conflicts_with {
                let is_zoi_conflict = installed_packages
                    .iter()
                    .any(|p| &p.name == conflict_pkg_name);

                if is_zoi_conflict {
                    conflict_messages.push(format!(
                        "Package '{}' conflicts with installed package '{}'.",
                        pkg.name.cyan(),
                        conflict_pkg_name.cyan()
                    ));
                } else if utils::command_exists(conflict_pkg_name) {
                    conflict_messages.push(format!(
                        "Package '{}' conflicts with existing command '{}' on your system.",
                        pkg.name.cyan(),
                        conflict_pkg_name.cyan()
                    ));
                }
            }
        }

        if let Some(bins_provided) = &pkg.bins {
            for bin in bins_provided {
                for installed_pkg in &installed_packages {
                    if let Some(installed_bins) = &installed_pkg.bins
                        && installed_bins.contains(bin)
                    {
                        conflict_messages.push(format!(
                                "Binary '{}' provided by '{}' is already provided by installed package '{}'.",
                                bin.cyan(),
                                pkg.name.cyan(),
                                installed_pkg.name.cyan()
                            ));
                    }
                }
            }
        }

        let unique_messages: HashSet<String> = conflict_messages.into_iter().collect();
        if !unique_messages.is_empty() {
            println!("{}", "Conflict Detected:".red().bold());
            for msg in unique_messages {
                println!("- {}", msg);
            }
            if !utils::ask_for_confirmation(
                "Do you want to continue with the installation anyway?",
                yes,
            ) {
                return Err("Operation aborted by user due to conflicts.".into());
            }
        }
        return Ok(());
    }

    if utils::command_exists(&pkg.name) {
        println!(
            "Warning: Command '{}' exists but was not installed by Zoi.",
            pkg.name.yellow()
        );
        if !utils::ask_for_confirmation(
            "Do you want to continue and potentially overwrite it?",
            yes,
        ) {
            return Err("Operation aborted by user.".into());
        }
    }

    Ok(())
}

fn run_post_install_hooks(pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    if let Some(hooks) = &pkg.post_install {
        println!("\n{}", "Running post-installation commands...".bold());
        let platform = utils::get_platform()?;
        let version = pkg.version.as_deref().unwrap_or("");

        for hook in hooks {
            if utils::is_platform_compatible(&platform, &hook.platforms) {
                for cmd_str in &hook.commands {
                    let final_cmd = cmd_str
                        .replace("{version}", version)
                        .replace("{name}", &pkg.name);

                    println!("Executing: {}", final_cmd.cyan());

                    let pb = ProgressBar::new_spinner();
                    pb.set_style(
                        ProgressStyle::default_spinner()
                            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                            .template("{spinner:.green} {msg}")?,
                    );
                    pb.set_message(format!("Running: {}", final_cmd));

                    let output = if cfg!(target_os = "windows") {
                        Command::new("pwsh")
                            .arg("-Command")
                            .arg(&final_cmd)
                            .output()?
                    } else {
                        Command::new("bash").arg("-c").arg(&final_cmd).output()?
                    };

                    pb.finish_and_clear();

                    if !output.status.success() {
                        io::stdout().write_all(&output.stdout)?;
                        io::stderr().write_all(&output.stderr)?;
                        return Err(format!("Post-install command failed: '{}'", final_cmd).into());
                    } else {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        if !stdout.trim().is_empty() {
                            println!("{}", stdout.trim());
                        }
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
) -> Result<(), Box<dyn Error>> {
    if let Some(method) = find_method(pkg, method_name, platform) {
        println!("Using '{}' method specified by updater.", method_name);
        return match method_name {
            "binary" => handle_binary_install(method, pkg),
            "com_binary" => handle_com_binary_install(method, pkg),
            "script" => handle_script_install(method, pkg),
            "source" => handle_source_install(method, pkg),
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

fn run_interactive_flow(pkg: &types::Package, platform: &str) -> Result<(), Box<dyn Error>> {
    let mut available_methods = Vec::new();
    for method in &pkg.installation {
        if crate::utils::is_platform_compatible(platform, &method.platforms) {
            available_methods.push(method);
        }
    }

    if available_methods.is_empty() {
        return Err("No compatible installation methods found for your platform.".into());
    }

    let method_names: Vec<&str> = available_methods
        .iter()
        .map(|m| m.install_type.as_str())
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select an installation method")
        .items(&method_names)
        .default(0)
        .interact()?;

    let selected_method = available_methods[selection];

    match selected_method.install_type.as_str() {
        "binary" => handle_binary_install(selected_method, pkg),
        "com_binary" => handle_com_binary_install(selected_method, pkg),
        "script" => handle_script_install(selected_method, pkg),
        "source" => handle_source_install(selected_method, pkg),
        _ => Err("Invalid installation method selected.".into()),
    }
}

fn run_default_flow(pkg: &types::Package, platform: &str, yes: bool) -> Result<(), Box<dyn Error>> {
    if let Some(method) = find_method(pkg, "binary", platform) {
        println!("Found 'binary' method. Installing...");
        return handle_binary_install(method, pkg);
    }

    println!("No binary found, checking for compressed binary...");
    if let Some(method) = find_method(pkg, "com_binary", platform) {
        println!("Found 'com_binary' method. Installing...");
        return handle_com_binary_install(method, pkg);
    }

    println!("No compressed binary found, checking for script...");
    if let Some(method) = find_method(pkg, "script", platform)
        && utils::ask_for_confirmation("Found a 'script' method. Do you want to execute it?", yes)
    {
        return handle_script_install(method, pkg);
    }

    println!("No script found, checking for source...");
    if let Some(method) = find_method(pkg, "source", platform)
        && utils::ask_for_confirmation(
            "Found a 'source' method. Do you want to build from source?",
            yes,
        )
    {
        return handle_source_install(method, pkg);
    }

    Err("No compatible and accepted installation method found for your platform.".into())
}

fn run_source_flow(pkg: &types::Package, platform: &str) -> Result<(), Box<dyn Error>> {
    if let Some(method) = find_method(pkg, "source", platform) {
        return handle_source_install(method, pkg);
    }
    Err("No compatible 'source' installation method found.".into())
}

fn find_method<'a>(
    pkg: &'a types::Package,
    type_name: &str,
    platform: &str,
) -> Option<&'a types::InstallationMethod> {
    pkg.installation.iter().find(|m| {
        m.install_type == type_name && crate::utils::is_platform_compatible(platform, &m.platforms)
    })
}

fn write_manifest(
    pkg: &types::Package,
    reason: types::InstallReason,
    installed_dependencies: Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let manifest = types::InstallManifest {
        name: pkg.name.clone(),
        version: pkg.version.clone().expect("Version should be resolved"),
        repo: pkg.repo.clone(),
        installed_at: Utc::now().to_rfc3339(),
        reason,
        scope: pkg.scope,
        bins: pkg.bins.clone(),
        installed_dependencies,
    };
    local::write_manifest(&manifest)
}

fn write_sharable_manifest(
    pkg: &types::Package,
    chosen_options: Vec<String>,
    chosen_optionals: Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let manifest = types::SharableInstallManifest {
        name: pkg.name.clone(),
        version: pkg.version.clone().expect("Version should be resolved"),
        repo: pkg.repo.clone(),
        scope: pkg.scope,
        chosen_options,
        chosen_optionals,
    };

    let store_dir = home::home_dir()
        .ok_or("No home dir")?
        .join(".zoi/pkgs/store")
        .join(&pkg.name);
    fs::create_dir_all(&store_dir)?;

    let manifest_path = store_dir.join(format!("{}.manifest.yaml", pkg.name));
    let content = serde_yaml::to_string(&manifest)?;
    fs::write(manifest_path, content)?;

    Ok(())
}

fn get_filename_from_url(url: &str) -> &str {
    url.split('/').next_back().unwrap_or("")
}

fn get_expected_checksum(
    checksums: &types::Checksums,
    file_to_verify: &str,
    pkg: &types::Package,
    platform: &str,
) -> Result<Option<(String, String)>, Box<dyn Error>> {
    match checksums {
        types::Checksums::Url(url) => {
            let mut url = url.replace("{version}", pkg.version.as_deref().unwrap_or(""));
            url = url.replace("{name}", &pkg.name);
            url = url.replace("{platform}", platform);

            println!("Downloading checksums from: {}", url.cyan());
            let response = reqwest::blocking::get(&url)?.text()?;
            for line in response.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() == 2 && parts[1] == file_to_verify {
                    return Ok(Some((parts[0].to_string(), "sha512".to_string())));
                }
            }
            if response.lines().count() == 1 && response.split_whitespace().count() == 1 {
                return Ok(Some((response.trim().to_string(), "sha512".to_string())));
            }
            Ok(None)
        }
        types::Checksums::List {
            checksum_type,
            items,
        } => {
            for item in items {
                let mut file_pattern = item
                    .file
                    .replace("{version}", pkg.version.as_deref().unwrap_or(""));
                file_pattern = file_pattern.replace("{name}", &pkg.name);
                file_pattern = file_pattern.replace("{platform}", platform);

                if file_pattern == file_to_verify {
                    if item.checksum.starts_with("http") {
                        println!("Downloading checksum from: {}", item.checksum.cyan());
                        let response = reqwest::blocking::get(&item.checksum)?.text()?;
                        return Ok(Some((response.trim().to_string(), checksum_type.clone())));
                    } else {
                        return Ok(Some((item.checksum.clone(), checksum_type.clone())));
                    }
                }
            }
            Ok(None)
        }
    }
}

fn verify_checksum(
    data: &[u8],
    method: &types::InstallationMethod,
    pkg: &types::Package,
    file_to_verify: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(checksums) = &method.checksums {
        println!("Verifying checksum for {}...", file_to_verify);
        let platform = utils::get_platform()?;
        if let Some((expected_checksum, checksum_type)) =
            get_expected_checksum(checksums, file_to_verify, pkg, &platform)?
        {
            let computed_checksum = match checksum_type.as_str() {
                "sha256" => {
                    let mut hasher = Sha256::new();
                    hasher.update(data);
                    format!("{:x}", hasher.finalize())
                }
                _ => {
                    let mut hasher = Sha512::new();
                    hasher.update(data);
                    format!("{:x}", hasher.finalize())
                }
            };

            if computed_checksum.eq_ignore_ascii_case(&expected_checksum) {
                println!("{}", "Checksum verified successfully.".green());
                Ok(())
            } else {
                Err(format!(
                    "Checksum mismatch for {}.\nExpected: {}\nComputed: {}",
                    file_to_verify, expected_checksum, computed_checksum
                )
                .into())
            }
        } else {
            println!(
                "{} No checksum found for file '{}'. Skipping verification.",
                "Warning:".yellow(),
                file_to_verify
            );
            Ok(())
        }
    } else {
        Ok(())
    }
}

struct Helper {
    certs: Vec<Cert>,
}

impl VerificationHelper for Helper {
    fn get_certs(&mut self, ids: &[KeyHandle]) -> anyhow::Result<Vec<Cert>> {
        let matching_certs: Vec<Cert> = self
            .certs
            .iter()
            .filter(|cert| {
                ids.iter().any(|id| {
                    cert.keys().any(|key| match *id {
                        KeyHandle::KeyID(ref keyid) => key.key().keyid() == *keyid,
                        KeyHandle::Fingerprint(ref fp) => key.key().fingerprint() == *fp,
                    })
                })
            })
            .cloned()
            .collect();
        Ok(matching_certs)
    }

    fn check(&mut self, structure: MessageStructure) -> anyhow::Result<()> {
        if let Some(layer) = structure.into_iter().next() {
            match layer {
                MessageLayer::SignatureGroup { results } => {
                    if results.iter().any(|r| r.is_ok()) {
                        return Ok(());
                    } else {
                        return Err(anyhow::anyhow!("No valid signature found"));
                    }
                }
                _ => return Err(anyhow::anyhow!("Unexpected message structure")),
            }
        }
        Err(anyhow::anyhow!("No signature layer found"))
    }
}

fn verify_signatures(
    data: &[u8],
    method: &types::InstallationMethod,
    pkg: &types::Package,
    file_to_verify: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(sigs) = &method.sigs {
        let sig_info = sigs.iter().find(|s| {
            let platform = utils::get_platform().unwrap_or_default();
            let version = pkg.version.as_deref().unwrap_or("");
            let file_pattern = s
                .file
                .replace("{version}", version)
                .replace("{name}", &pkg.name)
                .replace("{platform}", &platform);
            file_pattern == file_to_verify
        });

        if let Some(sig_info) = sig_info {
            println!("Verifying signature for {}...", file_to_verify);

            let keys = [
                pkg.maintainer.key.as_deref(),
                pkg.author.as_ref().and_then(|a| a.key.as_deref()),
            ]
            .iter()
            .filter_map(|&k| k)
            .collect::<Vec<_>>();

            if keys.is_empty() {
                println!(
                    "{} Signature found for '{}', but no maintainer or author key is defined. Skipping verification.",
                    "Warning:".yellow(),
                    file_to_verify
                );
                return Ok(());
            }

            let rt = Runtime::new()?;
            rt.block_on(async {
                let mut certs = Vec::new();
                for key_source in &keys {
                    let key_bytes_result = if key_source.starts_with("http") {
                        println!("Importing key from URL: {}", key_source.cyan());
                        reqwest::get(*key_source).await?.bytes().await
                    } else if key_source.len() == 40 && key_source.chars().all(|c| c.is_ascii_hexdigit()) {
                        let fingerprint = key_source.to_uppercase();
                        let key_server_url = format!("https://keys.openpgp.org/vks/v1/by-fingerprint/{}", fingerprint);
                        println!("Importing key for fingerprint {} from keyserver...", fingerprint.cyan());
                        reqwest::get(&key_server_url).await?.bytes().await
                    } else {
                        println!("{} Invalid key source: '{}'. Must be a URL or a 40-character GPG fingerprint.", "Warning:".yellow(), key_source);
                        continue;
                    };

                    match key_bytes_result {
                        Ok(key_bytes) => {
                            if let Ok(cert) = Cert::from_bytes(&key_bytes) {
                                certs.push(cert);
                            } else {
                                println!("{} Failed to parse certificate from source: {}", "Warning:".yellow(), key_source);
                            }
                        },
                        Err(e) => {
                             println!("{} Failed to download key from source {}: {}", "Warning:".yellow(), key_source, e);
                        }
                    }
                }

                if certs.is_empty() {
                    return Err(anyhow::anyhow!("No valid public keys found to verify signature."));
                }

                println!("Downloading signature from: {}", sig_info.sig);
                let sig_bytes = reqwest::get(&sig_info.sig).await?.bytes().await?;

                let policy = &StandardPolicy::new();
                let helper = Helper { certs };

                let mut verifier = DetachedVerifierBuilder::from_bytes(&sig_bytes)?
                    .with_policy(policy, None, helper)?;

                verifier.verify_bytes(data)?;

                println!("{}", "Signature verified successfully.".green());
                Ok(())
            })?;
        }
    }
    Ok(())
}

fn handle_com_binary_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    let platform = utils::get_platform()?;
    let target_os = platform.split('-').next().unwrap_or("");
    let os = std::env::consts::OS;

    let com_ext = method
        .platform_com_ext
        .as_ref()
        .and_then(|ext_map| ext_map.get(os))
        .map(|s| s.as_str())
        .unwrap_or(if os == "windows" { "zip" } else { "tar.zst" });

    let mut url = method
        .url
        .replace("{version}", pkg.version.as_deref().unwrap_or(""));
    url = url.replace("{name}", &pkg.name);
    url = url.replace("{platform}", &platform);
    url = url.replace("{git}", &pkg.git);
    url = url.replace("{platformComExt}", com_ext);

    if url.starts_with("http://") {
        println!(
            "{} downloading over insecure HTTP: {}",
            "Warning:".yellow(),
            url
        );
    }
    println!("Downloading from: {url}");

    let client = crate::utils::build_blocking_http_client(60)?;
    let mut attempt = 0u32;
    let response = loop {
        attempt += 1;
        match client.get(&url).send() {
            Ok(resp) => break resp,
            Err(e) => {
                if attempt < 3 {
                    eprintln!(
                        "{}: download failed ({}). Retrying...",
                        "Network".yellow(),
                        e
                    );
                    crate::utils::retry_backoff_sleep(attempt);
                    continue;
                } else {
                    return Err(format!(
                        "Failed to download '{}' after {} attempts: {}",
                        url, attempt, e
                    )
                    .into());
                }
            }
        }
    };
    if !response.status().is_success() {
        return Err(format!("Failed to download (HTTP {}): {}", response.status(), url).into());
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")? 
        .progress_chars("#>- "));

    let mut downloaded_bytes = Vec::new();
    let mut stream = response;
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        downloaded_bytes.extend_from_slice(&buffer[..bytes_read]);
        pb.inc(bytes_read as u64);
    }
    pb.finish_with_message("Download complete.");

    let file_to_verify = get_filename_from_url(&url);
    verify_checksum(&downloaded_bytes, method, pkg, file_to_verify)?;
    verify_signatures(&downloaded_bytes, method, pkg, file_to_verify)?;

    let temp_dir = Builder::new().prefix("zoi-com-binary").tempdir()?;

    if com_ext == "zip" {
        let mut archive = ZipArchive::new(Cursor::new(downloaded_bytes))?;
        archive.extract(temp_dir.path())?;
    } else if com_ext == "tar.zst" {
        let tar = ZstdDecoder::new(Cursor::new(downloaded_bytes))?;
        let mut archive = Archive::new(tar);
        archive.unpack(temp_dir.path())?;
    } else if com_ext == "tar.xz" {
        let tar = XzDecoder::new(Cursor::new(downloaded_bytes));
        let mut archive = Archive::new(tar);
        archive.unpack(temp_dir.path())?;
    } else if com_ext == "tar.gz" {
        let tar = GzDecoder::new(Cursor::new(downloaded_bytes));
        let mut archive = Archive::new(tar);
        archive.unpack(temp_dir.path())?;
    } else {
        return Err(format!("Unsupported compression format: {}", com_ext).into());
    }

    let store_dir = home::home_dir()
        .ok_or("No home dir")?
        .join(".zoi/pkgs/store")
        .join(&pkg.name)
        .join("bin");
    fs::create_dir_all(&store_dir)?;
    let mut dest_filename = pkg.name.clone();
    if let Some(bp) = &method.binary_path
        && bp.ends_with(".exe")
    {
        dest_filename = format!("{}.exe", pkg.name);
    }
    let mut bin_path = store_dir.join(&dest_filename);

    if pkg.package_type == types::PackageType::Library {
        library::install_files(temp_dir.path(), pkg)?;
        println!("{}", "Library files installed successfully.".green());
        return Ok(());
    }

    let binary_name = &pkg.name;
    let binary_name_with_ext = format!("{}.exe", pkg.name);
    let declared_binary_path_normalized: Option<String> = method.binary_path.as_ref().map(|bp| {
        if target_os == "windows" && !bp.ends_with(".exe") {
            format!("{bp}.exe")
        } else {
            bp.clone()
        }
    });
    let declared_binary_path = declared_binary_path_normalized.as_deref();
    let mut found_binary_path = None;
    let mut files_in_archive = Vec::new();

    for entry in WalkDir::new(temp_dir.path())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        let path = entry.path();
        files_in_archive.push(path.to_path_buf());
        if let Some(bp) = declared_binary_path {
            let rel = path
                .strip_prefix(temp_dir.path())
                .unwrap_or(path)
                .to_path_buf();
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let bp_norm = bp.replace('\\', "/");
            let file_name = path.file_name().and_then(|o| o.to_str()).unwrap_or("");
            let mut matched = rel_str == bp_norm;
            if !matched && !bp_norm.contains('/') {
                matched = file_name == bp_norm;
                if !matched && bp_norm == binary_name.as_str() {
                    matched = file_name == binary_name_with_ext.as_str();
                }
            }
            if matched {
                found_binary_path = Some(path.to_path_buf());
            }
        } else {
            let file_name = path.file_name().unwrap_or_default();
            if file_name == binary_name.as_str()
                || (target_os == "windows" && file_name == binary_name_with_ext.as_str())
            {
                found_binary_path = Some(path.to_path_buf());
            }
        }
    }

    if let Some(found_path) = found_binary_path {
        if found_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".exe"))
            .unwrap_or(false)
        {
            dest_filename = format!("{}.exe", pkg.name);
            bin_path = store_dir.join(&dest_filename);
        }
        fs::copy(found_path, &bin_path)?;
    } else if files_in_archive.len() == 1 {
        println!(
            "{}",
            "Could not find binary by package name. Found one file, assuming it's the correct one."
                .yellow()
        );
        let only = &files_in_archive[0];
        if only
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".exe"))
            .unwrap_or(false)
        {
            dest_filename = format!("{}.exe", pkg.name);
            bin_path = store_dir.join(&dest_filename);
        }
        fs::copy(only, &bin_path)?;
    } else {
        eprintln!(
            "Error: Could not find binary '{}' in the extracted archive.",
            binary_name
        );
        eprintln!("Listing contents of the extracted archive:");
        for path in files_in_archive {
            eprintln!("- {}", path.display());
        }
        return Err(format!(
            "Could not find binary '{}' in the extracted archive.",
            binary_name
        )
        .into());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
    }

    #[cfg(unix)]
    {
        let symlink_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
        fs::create_dir_all(&symlink_dir)?;
        let symlink_path = symlink_dir.join(&pkg.name);

        if symlink_path.exists() {
            fs::remove_file(&symlink_path)?;
        }
        std::os::unix::fs::symlink(&bin_path, symlink_path)?;
    }

    #[cfg(windows)]
    {
        println!(
            "{}",
            "Binary installed. Please add ~/.zoi/pkgs/bin to your PATH manually.".yellow()
        );
    }

    println!("{}", "Compressed binary installed successfully.".green());
    Ok(())
}

fn handle_binary_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    let platform = utils::get_platform()?;
    let os = std::env::consts::OS;

    let mut binary_type = None;
    if let Some(binary_types) = &method.binary_types {
        if os == "macos" && binary_types.contains(&"dmg".to_string()) {
            binary_type = Some("dmg");
        } else if os == "windows" && binary_types.contains(&"msi".to_string()) {
            binary_type = Some("msi");
        } else if os == "linux" && binary_types.contains(&"appimage".to_string()) {
            binary_type = Some("appimage");
        }
    }

    if let Some(ext) = binary_type {
        if pkg.package_type == types::PackageType::Library {
            return Err("DMG/MSI/AppImage installers are not supported for libraries.".into());
        }
        let mut url = method
            .url
            .replace("{version}", pkg.version.as_deref().unwrap_or(""));
        url = url.replace("{name}", &pkg.name);
        url = url.replace("{platform}", &platform);
        url = url.replace("{git}", &pkg.git);

        if !url.ends_with(ext) {
            url = format!("{}.{}", url, ext);
        }

        if url.starts_with("http://") {
            println!(
                "{} downloading over insecure HTTP: {}",
                "Warning:".yellow(),
                url
            );
        }
        println!("Downloading from: {url}");

        let client = crate::utils::build_blocking_http_client(60)?;
        let mut attempt = 0u32;
        let response = loop {
            attempt += 1;
            match client.get(&url).send() {
                Ok(resp) => break resp,
                Err(e) => {
                    if attempt < 3 {
                        eprintln!(
                            "{}: download failed ({}). Retrying...",
                            "Network".yellow(),
                            e
                        );
                        crate::utils::retry_backoff_sleep(attempt);
                        continue;
                    } else {
                        return Err(format!(
                            "Failed to download '{}' after {} attempts: {}",
                            url, attempt, e
                        )
                        .into());
                    }
                }
            }
        };
        if !response.status().is_success() {
            return Err(format!(
                "Failed to download binary (HTTP {}): {}",
                response.status(),
                url
            )
            .into());
        }

        let total_size = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")? 
        .progress_chars("#>- "));

        let mut downloaded_bytes = Vec::new();
        let mut stream = response;
        let mut buffer = [0; 8192];
        loop {
            let bytes_read = stream.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            downloaded_bytes.extend_from_slice(&buffer[..bytes_read]);
            pb.inc(bytes_read as u64);
        }
        pb.finish_with_message("Download complete.");

        let file_to_verify = get_filename_from_url(&url);
        verify_checksum(&downloaded_bytes, method, pkg, file_to_verify)?;
        verify_signatures(&downloaded_bytes, method, pkg, file_to_verify)?;

        let temp_dir = Builder::new()
            .prefix(&format!("zoi-install-{}", pkg.name))
            .tempdir()?;
        let file_name = get_filename_from_url(&url);
        let temp_file_path = temp_dir.path().join(file_name);
        fs::write(&temp_file_path, downloaded_bytes)?;

        println!("Installing {}...", file_name.cyan());

        if ext == "dmg" {
            let output = Command::new("hdiutil")
                .arg("attach")
                .arg(&temp_file_path)
                .output()?;
            if !output.status.success() {
                return Err(format!(
                    "Failed to mount DMG: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
                .into());
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mount_path_line = stdout.lines().last().unwrap_or("");
            let mount_path_parts: Vec<&str> = mount_path_line.split('\t').collect();
            let mount_path = PathBuf::from(mount_path_parts.last().unwrap_or(&"").trim());

            if mount_path.as_os_str().is_empty() {
                return Err("Could not determine mount path for DMG.".into());
            }

            let app_path = fs::read_dir(&mount_path)?
                .filter_map(Result::ok)
                .find(|entry| entry.path().extension().is_some_and(|ext| ext == "app"))
                .map(|entry| entry.path());

            if let Some(app_path) = app_path {
                let app_name = app_path.file_name().unwrap().to_str().unwrap();
                let app_dest_dir = if pkg.scope == types::Scope::System {
                    PathBuf::from("/Applications")
                } else {
                    home::home_dir().ok_or("No home dir")?.join("Applications")
                };
                fs::create_dir_all(&app_dest_dir)?;
                let dest_path = app_dest_dir.join(app_name);

                println!(
                    "Copying {} to {}...",
                    app_name.cyan(),
                    app_dest_dir.display()
                );
                let cp_status = Command::new("cp")
                    .arg("-R")
                    .arg(&app_path)
                    .arg(&app_dest_dir)
                    .status()?;
                if !cp_status.success() {
                    return Err("Failed to copy .app from DMG.".into());
                }

                let bin_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
                fs::create_dir_all(&bin_dir)?;
                let symlink_path = bin_dir.join(&pkg.name);

                let executable_name = app_path.file_stem().unwrap().to_str().unwrap();
                let app_executable = dest_path.join("Contents/MacOS").join(executable_name);

                if app_executable.exists() {
                    if symlink_path.exists() {
                        fs::remove_file(&symlink_path)?;
                    }
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(&app_executable, &symlink_path)?;
                } else {
                    println!(
                        "{} Could not find executable inside .app bundle to create a symlink.",
                        "Warning:".yellow()
                    );
                }
            } else {
                return Err("Could not find an .app file in the mounted DMG.".into());
            }

            Command::new("hdiutil")
                .arg("detach")
                .arg(&mount_path)
                .status()?;
        } else if ext == "msi" {
            let status = Command::new("msiexec")
                .arg("/i")
                .arg(&temp_file_path)
                .arg("/qn")
                .status()?;
            if !status.success() {
                return Err("Failed to run MSI installer.".into());
            }
        } else if ext == "appimage" {
            let store_dir = home::home_dir()
                .ok_or("No home dir")?
                .join(".zoi/pkgs/store")
                .join(&pkg.name)
                .join("bin");
            fs::create_dir_all(&store_dir)?;

            let bin_path = store_dir.join(&pkg.name);
            fs::copy(&temp_file_path, &bin_path)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
            }

            let symlink_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
            fs::create_dir_all(&symlink_dir)?;
            let symlink_path = symlink_dir.join(&pkg.name);

            if symlink_path.exists() {
                fs::remove_file(&symlink_path)?;
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(&bin_path, symlink_path)?;
        }

        println!("{}", "Binary installed successfully.".green());
        return Ok(());
    }

    let mut url = method
        .url
        .replace("{version}", pkg.version.as_deref().unwrap_or(""));
    url = url.replace("{name}", &pkg.name);
    url = url.replace("{platform}", &platform);
    url = url.replace("{git}", &pkg.git);

    if url.starts_with("http://") {
        println!(
            "{} downloading over insecure HTTP: {}",
            "Warning:".yellow(),
            url
        );
    }
    println!("Downloading from: {url}");

    let client = crate::utils::build_blocking_http_client(60)?;
    let mut attempt = 0u32;
    let response = loop {
        attempt += 1;
        match client.get(&url).send() {
            Ok(resp) => break resp,
            Err(e) => {
                if attempt < 3 {
                    eprintln!(
                        "{}: download failed ({}). Retrying...",
                        "Network".yellow(),
                        e
                    );
                    crate::utils::retry_backoff_sleep(attempt);
                    continue;
                } else {
                    return Err(format!(
                        "Failed to download '{}' after {} attempts: {}",
                        url, attempt, e
                    )
                    .into());
                }
            }
        }
    };
    if !response.status().is_success() {
        return Err(format!(
            "Failed to download binary (HTTP {}): {}",
            response.status(),
            url
        )
        .into());
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")? 
        .progress_chars("#>- "));

    let mut downloaded_bytes = Vec::new();
    let mut stream = response;
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        downloaded_bytes.extend_from_slice(&buffer[..bytes_read]);
        pb.inc(bytes_read as u64);
    }
    pb.finish_with_message("Download complete.");

    let file_to_verify = get_filename_from_url(&url);
    verify_checksum(&downloaded_bytes, method, pkg, file_to_verify)?;
    verify_signatures(&downloaded_bytes, method, pkg, file_to_verify)?;

    if pkg.package_type == types::PackageType::Library {
        let lib_dir = library::get_lib_dir(pkg.scope)?;
        fs::create_dir_all(&lib_dir)?;
        let dest_path = lib_dir.join(get_filename_from_url(&url));
        fs::write(&dest_path, downloaded_bytes)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o755))?;
        }

        if pkg.scope == types::Scope::System && cfg!(target_os = "linux") {
            println!("Running ldconfig...");
            let status = Command::new("sudo").arg("ldconfig").status()?;
            if !status.success() {
                println!("Warning: ldconfig failed.");
            }
        }

        println!("{}", "Library file installed successfully.".green());
        return Ok(());
    }

    let store_dir = home::home_dir()
        .ok_or("No home dir")?
        .join(".zoi/pkgs/store")
        .join(&pkg.name)
        .join("bin");
    fs::create_dir_all(&store_dir)?;

    let binary_filename = if cfg!(target_os = "windows") {
        format!("{}.exe", pkg.name)
    } else {
        pkg.name.clone()
    };
    let bin_path = store_dir.join(&binary_filename);
    let mut dest = File::create(&bin_path)?;
    dest.write_all(&downloaded_bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
    }

    #[cfg(unix)]
    {
        let symlink_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
        fs::create_dir_all(&symlink_dir)?;
        let symlink_path = symlink_dir.join(&pkg.name);

        if symlink_path.exists() {
            fs::remove_file(&symlink_path)?;
        }
        std::os::unix::fs::symlink(&bin_path, symlink_path)?;
    }

    #[cfg(windows)]
    {
        println!(
            "{}",
            "Binary installed. Please add ~/.zoi/pkgs/bin to your PATH manually.".yellow()
        );
    }

    println!("{}", "Binary installed successfully.".green());
    Ok(())
}

fn handle_script_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    println!("Using 'script' installation method...");

    let platform_ext = if cfg!(target_os = "windows") {
        "ps1"
    } else {
        "sh"
    };

    let resolved_url = method
        .url
        .replace("{platformExt}", platform_ext)
        .replace("{website}", pkg.website.as_deref().unwrap_or_default())
        .replace("{git}", &pkg.git);

    let temp_dir = Builder::new().prefix("zoi-script-install").tempdir()?;
    let script_filename = format!("install.{platform_ext}");
    let script_path = temp_dir.path().join(script_filename);

    if resolved_url.starts_with("http://") {
        println!(
            "{} downloading over insecure HTTP: {}",
            "Warning:".yellow(),
            resolved_url
        );
    }
    println!("Downloading script from: {}", resolved_url.cyan());
    let response = reqwest::blocking::get(&resolved_url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download script: HTTP {}", response.status()).into());
    }
    let script_bytes = response.bytes()?.to_vec();

    let file_to_verify = get_filename_from_url(&resolved_url);
    verify_checksum(&script_bytes, method, pkg, file_to_verify)?;
    verify_signatures(&script_bytes, method, pkg, file_to_verify)?;

    fs::write(&script_path, script_bytes)?;
    println!("Script downloaded to temporary location.");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        println!("Setting execute permissions...");
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
    }

    println!("Executing installation script...");
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")?,
    );
    pb.set_message("Running script...");

    let mut command = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("powershell");
        cmd.arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(&script_path);
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg(&script_path);
        cmd
    };

    let output = command.output()?;
    pb.finish_and_clear();

    if !output.status.success() {
        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)?;
        return Err("Installation script failed to execute successfully.".into());
    }

    println!("{}", "Script executed successfully.".green());
    Ok(())
}

fn handle_source_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    println!("{}", "Building from source...".bold());
    let store_path = home::home_dir()
        .ok_or("No home dir")?
        .join(".zoi/pkgs/store")
        .join(&pkg.name);
    let git_path = store_path.join("git");
    let bin_path = store_path.join("bin");
    fs::create_dir_all(&bin_path)?;

    let repo_url = method.url.replace("{git}", &pkg.git);
    println!("Cloning from {repo_url}...");

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")?,
    );
    pb.set_message(format!("Cloning {}...", pkg.name));

    let output = std::process::Command::new("git")
        .arg("clone")
        .arg(&repo_url)
        .arg(&git_path)
        .output()?;
    pb.finish_and_clear();

    if !output.status.success() {
        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)?;
        return Err("Failed to clone source repository.".into());
    }

    if method.tag.is_some() && method.branch.is_some() {
        return Err(
            "Invalid source method: both 'tag' and 'branch' specified. Use only one.".into(),
        );
    }
    if let Some(tag_tmpl) = &method.tag {
        let version = pkg.version.as_deref().unwrap_or("");
        let tag = tag_tmpl.replace("{version}", version);
        println!("Checking out tag {}...", tag.cyan());
        let out = std::process::Command::new("git")
            .current_dir(&git_path)
            .arg("checkout")
            .arg(format!("tags/{}", tag))
            .output()?;
        if !out.status.success() {
            io::stdout().write_all(&out.stdout)?;
            io::stderr().write_all(&out.stderr)?;
            return Err(format!("Failed to checkout tag '{}'", tag).into());
        }
    } else if let Some(branch_tmpl) = &method.branch {
        let version = pkg.version.as_deref().unwrap_or("");
        let branch = branch_tmpl.replace("{version}", version);
        println!("Checking out branch {}...", branch.cyan());
        let out = std::process::Command::new("git")
            .current_dir(&git_path)
            .arg("checkout")
            .arg(&branch)
            .output()?;
        if !out.status.success() {
            io::stdout().write_all(&out.stdout)?;
            io::stderr().write_all(&out.stderr)?;
            return Err(format!("Failed to checkout branch '{}'", branch).into());
        }
    }

    if let Some(commands) = &method.commands {
        for cmd_str in commands {
            let final_cmd = cmd_str.replace("{store}", bin_path.to_str().unwrap());
            println!("Executing: {}", final_cmd.cyan());

            let pb_cmd = ProgressBar::new_spinner();
            pb_cmd.set_style(
                ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                    .template("{spinner:.green} {msg}")?,
            );
            pb_cmd.set_message(format!("Running: {}", final_cmd));

            let output = std::process::Command::new("bash")
                .arg("-c")
                .arg(&final_cmd)
                .current_dir(&git_path)
                .output()?;
            pb_cmd.finish_and_clear();

            if !output.status.success() {
                io::stdout().write_all(&output.stdout)?;
                io::stderr().write_all(&output.stderr)?;
                return Err(format!("Build command failed: '{}'", final_cmd).into());
            }
        }
    }

    let entries: Vec<PathBuf> = fs::read_dir(&bin_path)
        .map_err(|e| format!("Failed to read store directory at {:?}: {}", bin_path, e))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();

    if entries.is_empty() {
        println!(
            "{}",
            "Build completed, no binaries found in the store directory to link.".yellow()
        );
        println!("{}", "Source build and installation completed.".green());
        return Ok(());
    }

    println!("Build completed, searching for binaries in store directory to link...");

    let mut binaries_to_link: Vec<(String, PathBuf)> = Vec::new();

    if let Some(bin_names) = &pkg.bins {
        for bin_name in bin_names {
            let mut found_bin = false;
            for entry in &entries {
                let file_name = entry.file_name().unwrap().to_string_lossy();
                if file_name == *bin_name
                    || (cfg!(target_os = "windows") && file_name == format!("{}.exe", bin_name))
                {
                    binaries_to_link.push((bin_name.clone(), entry.clone()));
                    found_bin = true;
                    break;
                }
            }
            if !found_bin {
                return Err(format!(
                    "Could not find expected binary '{}' in store directory after build.",
                    bin_name
                )
                .into());
            }
        }
    } else if entries.len() == 1 {
        let bin_path = entries[0].clone();
        let bin_name = bin_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        binaries_to_link.push((bin_name, bin_path));
    } else {
        let os_specific_name = if cfg!(target_os = "windows") {
            format!("{}.exe", pkg.name)
        } else {
            pkg.name.clone()
        };

        for entry in &entries {
            if entry.file_name().unwrap().to_string_lossy() == os_specific_name {
                binaries_to_link.push((pkg.name.clone(), entry.clone()));
                break;
            }
        }
        if binaries_to_link.is_empty() && cfg!(target_os = "windows") {
            for entry in &entries {
                if entry.file_name().unwrap().to_string_lossy() == pkg.name {
                    binaries_to_link.push((pkg.name.clone(), entry.clone()));
                    break;
                }
            }
        }
    }

    if binaries_to_link.is_empty() {
        return Err(format!(
            "Build produced files in the store directory, but could not determine which binary to link for package '{}'. Specify the binary name in the 'bins' field of the package manifest.",
            pkg.name
        ).into());
    }

    for (bin_name, binary_path_in_store) in binaries_to_link {
        println!("Found built binary: {}", binary_path_in_store.display());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&binary_path_in_store, fs::Permissions::from_mode(0o755))?;

            let symlink_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
            fs::create_dir_all(&symlink_dir)?;
            let symlink_path = symlink_dir.join(bin_name);

            if symlink_path.exists() {
                fs::remove_file(&symlink_path)?;
            }
            std::os::unix::fs::symlink(&binary_path_in_store, &symlink_path)?;
        }

        #[cfg(windows)]
        {
            let bin_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
            fs::create_dir_all(&bin_dir)?;
            let dest_path = bin_dir.join(binary_path_in_store.file_name().unwrap());
            if dest_path.exists() {
                fs::remove_file(&dest_path)?;
            }
            fs::copy(&binary_path_in_store, &dest_path)?;
        }
    }

    println!("{}", "Source build and installation completed.".green());
    Ok(())
}
