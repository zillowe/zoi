use crate::pkg::{config_handler, dependencies, local, resolve, service, types};
use crate::utils;
use chrono::Utc;
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256, Sha512};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::process::Command;
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

#[derive(PartialEq, Eq)]
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
) -> Result<(), Box<dyn Error>> {
    let (pkg, version) = resolve::resolve_package_and_version(source)?;

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

    if pkg.package_type == types::PackageType::Collection {
        println!("Installing package collection '{}'...", pkg.name.bold());
        if let Some(deps) = &pkg.dependencies {
            if let Some(runtime_deps) = &deps.runtime {
                dependencies::resolve_and_install_required(
                    runtime_deps.get_required(),
                    &pkg.name,
                    pkg.scope,
                    yes,
                )?;
                dependencies::resolve_and_install_optional(
                    runtime_deps.get_optional(),
                    &pkg.name,
                    pkg.scope,
                    yes,
                )?;
            }
        } else {
            println!("Collection has no dependencies to install.");
        }
        write_manifest(&pkg, reason)?;
        println!("Collection '{}' installed successfully.", pkg.name.green());
        return Ok(());
    }

    if pkg.package_type == types::PackageType::Config {
        println!("Installing configuration '{}'...", pkg.name.bold());
        if let Some(deps) = &pkg.dependencies {
            if let Some(runtime_deps) = &deps.runtime {
                dependencies::resolve_and_install_required(
                    runtime_deps.get_required(),
                    &pkg.name,
                    pkg.scope,
                    yes,
                )?;
                dependencies::resolve_and_install_optional(
                    runtime_deps.get_optional(),
                    &pkg.name,
                    pkg.scope,
                    yes,
                )?;
            }
        }
        write_manifest(&pkg, reason)?;
        println!("Configuration '{}' registered.", pkg.name.green());

        if utils::ask_for_confirmation("Do you want to run the setup commands now?", yes) {
            config_handler::run_install_commands(&pkg)?;
        }
        return Ok(());
    }

    if let Some(mut manifest) = local::is_package_installed(&pkg.name, pkg.scope)? {
        if manifest.reason == types::InstallReason::Dependency
            && reason == types::InstallReason::Direct
        {
            println!("Updating package '{}' to be directly managed.", pkg.name);
            manifest.reason = types::InstallReason::Direct;
            local::write_manifest(&manifest)?;
        }
    }

    if !force {
        if let Some(manifest) = local::is_package_installed(&pkg.name, pkg.scope)? {
            println!(
                "{}",
                format!(
                    "Package '{}' version {} is already installed.",
                    pkg.name, manifest.version
                )
                .yellow()
            );
            if pkg.package_type == types::PackageType::Service {
                if utils::ask_for_confirmation("Do you want to start the service?", yes) {
                    service::start_service(&pkg)?;
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
                return Ok(());
            }
        }
    }

    println!("Installing '{}' version '{}'", pkg.name, version);

    if let Some(deps) = &pkg.dependencies {
        let mut optional_deps_to_install = Vec::new();

        if mode == InstallMode::ForceSource {
            if let Some(build_deps) = &deps.build {
                dependencies::resolve_and_install_required(
                    build_deps.get_required(),
                    &pkg.name,
                    pkg.scope,
                    yes,
                )?;
                optional_deps_to_install.extend(build_deps.get_optional().clone());
            }
        }

        if let Some(runtime_deps) = &deps.runtime {
            dependencies::resolve_and_install_required(
                runtime_deps.get_required(),
                &pkg.name,
                pkg.scope,
                yes,
            )?;
            optional_deps_to_install.extend(runtime_deps.get_optional().clone());
        }

        if !optional_deps_to_install.is_empty() {
            dependencies::resolve_and_install_optional(
                &optional_deps_to_install,
                &pkg.name,
                pkg.scope,
                yes,
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
        write_manifest(&pkg, reason)?;
        if let Err(e) = utils::setup_path(pkg.scope) {
            eprintln!("{} Failed to configure PATH: {}", "Warning:".yellow(), e);
        }
        if pkg.package_type == types::PackageType::Service {
            if utils::ask_for_confirmation("Do you want to start the service now?", yes) {
                service::start_service(&pkg)?;
            }
        }
        if pkg.post_install.is_some()
            && utils::ask_for_confirmation(
                "This package has post-installation commands. Do you want to run them?",
                yes,
            )
        {
            if let Err(e) = run_post_install_hooks(&pkg) {
                eprintln!(
                    "{} Post-installation commands failed: {}",
                    "Warning:".yellow(),
                    e
                );
            }
        }
    }

    result
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
                        Command::new("cmd").arg("/C").arg(&final_cmd).output()? 
                    } else {
                        Command::new("sh").arg("-c").arg(&final_cmd).output()? 
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
    if let Some(method) = find_method(pkg, "script", platform) {
        if utils::ask_for_confirmation("Found a 'script' method. Do you want to execute it?", yes) {
            return handle_script_install(method, pkg);
        }
    }

    println!("No script found, checking for source...");
    if let Some(method) = find_method(pkg, "source", platform) {
        if utils::ask_for_confirmation(
            "Found a 'source' method. Do you want to build from source?",
            yes,
        ) {
            return handle_source_install(method, pkg);
        }
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
) -> Result<(), Box<dyn Error>> {
    let manifest = types::InstallManifest {
        name: pkg.name.clone(),
        version: pkg.version.clone().expect("Version should be resolved"),
        repo: pkg.repo.clone(),
        installed_at: Utc::now().to_rfc3339(),
        reason,
        scope: pkg.scope,
    };
    local::write_manifest(&manifest)
}

fn get_filename_from_url(url: &str) -> &str {
    url.split('/').last().unwrap_or("")
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
        if let Some((expected_checksum, checksum_type)) =            get_expected_checksum(checksums, file_to_verify, pkg, &platform)?
        {
            let computed_checksum = match checksum_type.as_str() {
                "sha256" => {
                    let mut hasher = Sha256::new();
                    hasher.update(data);
                    format!("{:x}", hasher.finalize())
                }
                "sha512" | _ => {
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

fn handle_com_binary_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    let platform = utils::get_platform()?;
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
    url = url.replace("{platformComExt}", com_ext);

    println!("Downloading from: {url}");

    let response = reqwest::blocking::get(&url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download: HTTP {}", response.status()).into());
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
    let binary_filename = if cfg!(target_os = "windows") {
        format!("{}.exe", pkg.name)
    } else {
        pkg.name.clone()
    };
    let bin_path = store_dir.join(&binary_filename);

    let binary_name = &pkg.name;
    let binary_name_with_ext = format!("{}.exe", pkg.name);
    let mut found_binary_path = None;
    let mut files_in_archive = Vec::new();

    for entry in WalkDir::new(temp_dir.path())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        let path = entry.path();
        files_in_archive.push(path.to_path_buf());
        let file_name = path.file_name().unwrap_or_default();
        if file_name == binary_name.as_str()
            || (cfg!(target_os = "windows") && file_name == binary_name_with_ext.as_str())
        {
            found_binary_path = Some(path.to_path_buf());
        }
    }

    if let Some(found_path) = found_binary_path {
        fs::copy(found_path, &bin_path)?;
    } else if files_in_archive.len() == 1 {
        println!(
            "{}",
            "Could not find binary by package name. Found one file, assuming it's the correct one."
                .yellow()
        );
        fs::copy(&files_in_archive[0], &bin_path)?;
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
    let mut url = method
        .url
        .replace("{version}", pkg.version.as_deref().unwrap_or(""));
    url = url.replace("{name}", &pkg.name);
    let platform = utils::get_platform()?;
    url = url.replace("{platform}", &platform);

    println!("Downloading from: {url}");

    let response = reqwest::blocking::get(&url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download binary: HTTP {}", response.status()).into());
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
        .replace("{website}", pkg.website.as_deref().unwrap_or_default());

    let temp_dir = Builder::new().prefix("zoi-script-install").tempdir()?;
    let script_filename = format!("install.{platform_ext}");
    let script_path = temp_dir.path().join(script_filename);

    println!("Downloading script from: {}", resolved_url.cyan());
    let response = reqwest::blocking::get(&resolved_url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download script: HTTP {}", response.status()).into());
    }
    let script_bytes = response.bytes()?.to_vec();

    let file_to_verify = get_filename_from_url(&resolved_url);
    verify_checksum(&script_bytes, method, pkg, file_to_verify)?;

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
        let mut cmd = Command::new("sh");
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

            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&final_cmd)
                .current_dir(&git_path)
                .output()?;
            pb_cmd.finish_and_clear();

            if !output.status.success() {
                io::stdout().write_all(&output.stdout)?;
                io::stderr().write_all(&output.stderr)?;
                return Err(format!("Build command failed: '{final_cmd}'").into());
            }
        }
    }

    println!("{}", "Source build and installation completed.".green());
    Ok(())
}
