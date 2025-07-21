use crate::pkg::{dependencies, local, types};
use crate::utils;
use chrono::Utc;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use tempfile::Builder;

#[derive(PartialEq, Eq)]
pub enum InstallMode {
    PreferBinary,
    ForceSource,
}

pub fn run_installation(
    package_file: &Path,
    mode: InstallMode,
    force: bool,
    reason: types::InstallReason,
) -> Result<(), Box<dyn Error>> {
    let yaml_content = fs::read_to_string(package_file)?;
    let pkg: types::Package = serde_yaml::from_str(&yaml_content)?;

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
            if !utils::ask_for_confirmation("Do you want to continue and potentially overwrite it?")
            {
                return Ok(());
            }
        }
    }

    println!("Installing '{}' version '{}'", pkg.name, pkg.version);

    if let Some(deps) = &pkg.dependencies {
        if mode == InstallMode::ForceSource {
            if let Some(build_deps) = &deps.build {
                dependencies::resolve_and_install(build_deps, &pkg.name)?;
            }
        }
        if let Some(runtime_deps) = &deps.runtime {
            dependencies::resolve_and_install(runtime_deps, &pkg.name)?;
        }
    }

    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    println!("Current platform: {platform}");

    let result = if mode == InstallMode::ForceSource {
        run_source_flow(&pkg, &platform)
    } else {
        run_default_flow(&pkg, &platform)
    };

    if result.is_ok() {
        write_manifest(&pkg, reason)?;
    }

    result
}

fn run_default_flow(pkg: &types::Package, platform: &str) -> Result<(), Box<dyn Error>> {
    if let Some(method) = find_method(pkg, "binary", platform) {
        println!("Found 'binary' method. Installing...");
        return handle_binary_install(method, pkg);
    }
    if let Some(method) = find_method(pkg, "script", platform) {
        if utils::ask_for_confirmation("Found a 'script' method. Do you want to execute it?") {
            return handle_script_install(method, pkg);
        }
    }
    if let Some(method) = find_method(pkg, "source", platform) {
        if utils::ask_for_confirmation("Found a 'source' method. Do you want to build from source?")
        {
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

fn handle_binary_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    let mut url = method.url.replace("{version}", &pkg.version);
    url = url.replace("{name}", &pkg.name);
    url = url.replace("{platforms}", &format!("{}-{}", env::consts::OS, env::consts::ARCH));

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

    let store_dir =
        home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/store").join(&pkg.name).join("bin");
    fs::create_dir_all(&store_dir)?;

    let bin_path = store_dir.join(&pkg.name);
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

    let platform_ext = if cfg!(target_os = "windows") { "ps1" } else { "sh" };

    let resolved_url =
        method.url.replace("{platformExt}", platform_ext).replace("{website}", &pkg.website);

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
    let status = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(&script_path)
            .status()?
    } else {
        Command::new("sh").arg(&script_path).status()?
    };

    if !status.success() {
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
    let store_path = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/store").join(&pkg.name);
    let git_path = store_path.join("git");
    let bin_path = store_path.join("bin");
    fs::create_dir_all(&bin_path)?;

    let repo_url = method.url.replace("{git}", &pkg.git);
    println!("Cloning from {repo_url}...");
    let status =
        std::process::Command::new("git").arg("clone").arg(&repo_url).arg(&git_path).status()?;
    if !status.success() {
        return Err("Failed to clone source repository.".into());
    }

    if let Some(commands) = &method.commands {
        for cmd_str in commands {
            let final_cmd = cmd_str.replace("{store}", bin_path.to_str().unwrap());
            println!("Executing: {}", final_cmd.cyan());
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(&final_cmd)
                .current_dir(&git_path)
                .status()?;
            if !status.success() {
                return Err(format!("Build command failed: '{final_cmd}'").into());
            }
        }
    }

    println!("{}", "Source build and installation completed.".green());
    Ok(())
}
