use crate::pkg::{dependencies, local, types};
use crate::utils;
use chrono::Utc;
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::Path;
use std::process::Command;
use tar::Archive;
use tempfile::Builder;
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;
use zip::ZipArchive;
use walkdir::WalkDir;

#[derive(PartialEq, Eq)]
pub enum InstallMode {
    PreferBinary,
    ForceSource,
    Interactive,
    Updater(String),
}

pub fn run_installation(
    package_file: &Path,
    mode: InstallMode,
    force: bool,
    reason: types::InstallReason,
    yes: bool,
) -> Result<(), Box<dyn Error>> {
    let yaml_content = fs::read_to_string(package_file)?;
    let pkg: types::Package = serde_yaml::from_str(&yaml_content)?;

    if pkg.package_type == types::PackageType::Collection {
        println!("Installing package collection '{}'...", pkg.name.bold());
        if let Some(deps) = &pkg.dependencies {
            if let Some(runtime_deps) = &deps.runtime {
                if !runtime_deps.is_empty() {
                    dependencies::resolve_and_install(runtime_deps, &pkg.name, yes)?;
                } else {
                    println!("Collection has no runtime dependencies to install.");
                }
            } else {
                println!("Collection has no runtime dependencies to install.");
            }
        } else {
            println!("Collection has no dependencies to install.");
        }
        write_manifest(&pkg, reason)?;
        println!("Collection '{}' installed successfully.", pkg.name.green());
        return Ok(());
    }

    if let Some(mut manifest) = local::is_package_installed(&pkg.name)? {
        if manifest.reason == types::InstallReason::Dependency
            && reason == types::InstallReason::Direct
        {
            println!("Updating package '{}' to be directly managed.", pkg.name);
            manifest.reason = types::InstallReason::Direct;
            local::write_manifest(&manifest)?;
        }
    }

    if !force {
        if let Some(manifest) = local::is_package_installed(&pkg.name)? {
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

    println!("Installing '{}' version '{}'", pkg.name, pkg.version);

    if let Some(deps) = &pkg.dependencies {
        if mode == InstallMode::ForceSource {
            if let Some(build_deps) = &deps.build {
                dependencies::resolve_and_install(build_deps, &pkg.name, yes)?;
            }
        }
        if let Some(runtime_deps) = &deps.runtime {
            dependencies::resolve_and_install(runtime_deps, &pkg.name, yes)?;
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
        if let Err(e) = utils::setup_path() {
            eprintln!("{} Failed to configure PATH: {}", "Warning:".yellow(), e);
        }
    }

    result
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

fn run_default_flow(
    pkg: &types::Package,
    platform: &str,
    yes: bool,
) -> Result<(), Box<dyn Error>> {
    if let Some(method) = find_method(pkg, "binary", platform) {
        println!("Found 'binary' method. Installing...");
        return handle_binary_install(method, pkg);
    }
    if let Some(method) = find_method(pkg, "com_binary", platform) {
        println!("Found 'com_binary' method. Installing...");
        return handle_com_binary_install(method, pkg);
    }
    if let Some(method) = find_method(pkg, "script", platform) {
        if utils::ask_for_confirmation("Found a 'script' method. Do you want to execute it?", yes)
        {
            return handle_script_install(method, pkg);
        }
    }
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
        version: pkg.version.clone(),
        repo: pkg.repo.clone(),
        installed_at: Utc::now().to_rfc3339(),
        reason,
    };
    local::write_manifest(&manifest)
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

    let mut url = method.url.replace("{version}", &pkg.version);
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
        .progress_chars("#>-"));

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
    let mut url = method.url.replace("{version}", &pkg.version);
    url = url.replace("{name}", &pkg.name);
    let platform = utils::get_platform()?;
    url = url.replace("{platform}", &platform);

    println!("Downloading from: {url}");

    let mut response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download binary: HTTP {}", response.status()).into());
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
        .progress_chars("#>-"));

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
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        dest.write_all(&buffer[..bytes_read])?;
        pb.inc(bytes_read as u64);
    }

    pb.finish_with_message("Download complete.");

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
        .replace("{website}", &pkg.website);

    let temp_dir = Builder::new().prefix("zoi-script-install").tempdir()?;
    let script_filename = format!("install.{platform_ext}");
    let script_path = temp_dir.path().join(script_filename);

    println!("Downloading script from: {}", resolved_url.cyan());
    let response = reqwest::blocking::get(&resolved_url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download script: HTTP {}", response.status()).into());
    }
    fs::write(&script_path, response.bytes()?)?;
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
