use anyhow::{Result, anyhow};
use clap_complete::Shell;
use colored::Colorize;
use crossterm::tty::IsTty;
use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::fs;
use std::io::{Write, stdin, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

/// Creates an HTTP client with Zoi's default configuration.
pub fn get_http_client() -> Result<&'static reqwest::blocking::Client> {
    if crate::offline::is_offline() {
        return Err(anyhow!(
            "Cannot create HTTP client: Zoi is in offline mode."
        ));
    }
    static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    if let Some(client) = HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("zoi")
        .timeout(Duration::from_secs(60))
        .use_rustls_tls()
        .build()
        .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;
    let _ = HTTP_CLIENT.set(client);
    HTTP_CLIENT
        .get()
        .ok_or_else(|| anyhow!("HTTP_CLIENT should be set but was missing"))
}

pub fn build_blocking_http_client(timeout_secs: u64) -> Result<reqwest::blocking::Client> {
    if crate::offline::is_offline() {
        return Err(anyhow!(
            "Cannot create HTTP client: Zoi is in offline mode."
        ));
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("zoi")
        .timeout(Duration::from_secs(timeout_secs))
        .use_rustls_tls()
        .build()?;
    Ok(client)
}

pub fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    if link.exists() || link.is_symlink() {
        if link.is_dir() && !link.is_symlink() {
            fs::remove_dir_all(link)?;
        } else {
            fs::remove_file(link)?;
        }
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_dir(target, link).is_err() {
            if junction::create(target, link).is_err() {
                copy_dir_all(target, link)?;
            }
        }
    }
    Ok(())
}

pub fn command_exists(command: &str) -> bool {
    if cfg!(target_os = "windows") {
        Command::new("where")
            .arg(command)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    } else {
        Command::new("bash")
            .arg("-c")
            .arg(format!("command -v {}", command))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

/// Returns a standard Zoi platform identifier (e.g. "linux-amd64", "windows-arm64").
///
/// This string is used extensively in registries and package definitions to
/// handle platform-specific dependencies and build artifacts.
pub fn get_platform() -> Result<String> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" | "darwin" => "macos",
        "windows" => "windows",
        unsupported_os => return Err(anyhow!("Unsupported operating system: {}", unsupported_os)),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" | "amd64" => "amd64",
        "aarch64" | "arm64" => "arm64",
        "x86" | "i386" | "i686" => "386",
        unsupported_arch => return Err(anyhow!("Unsupported architecture: {}", unsupported_arch)),
    };
    Ok(format!("{}-{}", os, arch))
}

/// Returns the root directory for the package database.
pub fn get_db_root() -> Result<std::path::PathBuf> {
    if let Ok(path) = std::env::var("ZOI_DB_DIR") {
        return Ok(std::path::PathBuf::from(path));
    }
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(crate::sysroot::apply_sysroot(
        home_dir.join(".zoi").join("pkgs").join("db"),
    ))
}

/// Returns the root directory of the package store for a given scope.
///
/// Store Locations:
/// - `User`: `~/.zoi/pkgs/store/`
/// - `System`: `/var/lib/zoi/pkgs/store/` (Linux) or `C:\ProgramData\zoi\pkgs\store` (Windows)
/// - `Project`: `./.zoi/pkgs/store/` (Relative to current project root)
pub fn get_store_base_dir(scope: crate::types::Scope) -> Result<PathBuf> {
    match scope {
        crate::types::Scope::User => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(crate::sysroot::apply_sysroot(
                home_dir.join(".zoi").join("pkgs").join("store"),
            ))
        }
        crate::types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(crate::sysroot::apply_sysroot(PathBuf::from(
                    "C:\\ProgramData\\zoi\\pkgs\\store",
                )))
            } else {
                Ok(crate::sysroot::apply_sysroot(PathBuf::from(
                    "/var/lib/zoi/pkgs/store",
                )))
            }
        }
        crate::types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("store"))
        }
    }
}

/// Generates a unique, origin-aware ID for a package.
///
/// This ID prevents collisions between packages with the same name that reside
/// in different registries or repository tiers.
///
/// ID Format: `#{registry-handle}@{repo-path}/{package-name}`
/// Hashed Result: First 32 characters of the SHA-512 hash of the ID string.
pub fn generate_package_id(registry_handle: &str, repo_path: &str, package_name: &str) -> String {
    let format_string = format!("#{}@{}/{}", registry_handle, repo_path, package_name);
    let mut hasher = Sha512::new();
    hasher.update(format_string.as_bytes());
    let result = hasher.finalize();
    let hex_string = hex::encode(result);
    hex_string[..32].to_string()
}

/// Generates a unique ID for a package including its version.
pub fn generate_versioned_package_id(
    registry_handle: &str,
    repo_path: &str,
    package_name: &str,
    version: &str,
) -> String {
    let format_string = format!(
        "#{}@{}/{}@{}",
        registry_handle, repo_path, package_name, version
    );
    let mut hasher = Sha512::new();
    hasher.update(format_string.as_bytes());
    let result = hasher.finalize();
    let hex_string = hex::encode(result);
    hex_string[..32].to_string()
}

/// Creates the directory name for the package in the store.
/// Format: `{hash}-{name}`
pub fn get_package_dir_name(package_id: &str, package_name: &str) -> String {
    format!("{}-{}", package_id, package_name)
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Performs a jittered exponential backoff sleep.
///
/// Used during network retries to prevent thundering herd problems and
/// improve reliability on unstable connections.
pub fn retry_backoff_sleep(attempt: u32) {
    let base_ms = 500u64.saturating_mul(1u64 << (attempt.saturating_sub(1)));
    let jitter = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .subsec_millis()
        % 200) as u64;
    let sleep_ms = (base_ms + jitter).min(8000);
    std::thread::sleep(Duration::from_millis(sleep_ms));
}

pub fn get_linux_distribution_info() -> Option<HashMap<String, String>> {
    if let Ok(contents) = fs::read_to_string("/etc/os-release") {
        let info: HashMap<String, String> = contents
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, '=');
                let key = parts.next()?;
                let value = parts.next()?.trim_matches('"').to_string();
                if key.is_empty() {
                    None
                } else {
                    Some((key.to_string(), value))
                }
            })
            .collect();
        if info.is_empty() { None } else { Some(info) }
    } else {
        None
    }
}

/// Detects the general family of a Linux distribution (e.g. "debian", "arch", "fedora").
///
/// This is used to map specific distributions to their primary package manager
/// and standard filesystem locations.
///
/// Strategy:
/// - ID_LIKE Check: We first check the `ID_LIKE` field in `/etc/os-release`.
///   This is the most reliable way to identify derivatives (e.g. Ubuntu is `debian`).
/// - Direct ID Match: If `ID_LIKE` is missing, we fall back to the primary `ID`.
/// - Normalization: We group similar distros under a common "family" key
///   to simplify downstream logic (e.g. Rocky, Alma, and CentOS all map to `fedora`
///   because they share the DNF/RPM ecosystem).
pub fn get_linux_distro_family() -> Option<String> {
    if let Some(info) = get_linux_distribution_info() {
        if let Some(id_like) = info.get("ID_LIKE") {
            let families: Vec<&str> = id_like.split_whitespace().collect();
            if families.contains(&"debian") {
                return Some("debian".to_string());
            }
            if families.contains(&"arch") {
                return Some("arch".to_string());
            }
            if families.contains(&"fedora") {
                return Some("fedora".to_string());
            }
            if families.contains(&"rhel") {
                return Some("fedora".to_string());
            }
            if families.contains(&"suse") {
                return Some("suse".to_string());
            }
            if families.contains(&"gentoo") {
                return Some("gentoo".to_string());
            }
        }
        if let Some(id) = info.get("ID") {
            return match id.as_str() {
                "debian" | "ubuntu" | "linuxmint" | "pop" | "kali" | "kubuntu" | "lubuntu"
                | "xubuntu" | "zorin" | "elementary" => Some("debian".to_string()),
                "arch" | "manjaro" | "cachyos" | "endeavouros" | "garuda" => {
                    Some("arch".to_string())
                }
                "fedora" | "centos" | "rhel" | "rocky" | "almalinux" => Some("fedora".to_string()),
                "opensuse" | "opensuse-tumbleweed" | "opensuse-leap" => Some("suse".to_string()),
                "gentoo" => Some("gentoo".to_string()),
                "alpine" => Some("alpine".to_string()),
                "void" => Some("void".to_string()),
                "solus" => Some("solus".to_string()),
                "guix" => Some("guix".to_string()),
                _ => None,
            };
        }
    }
    None
}

pub fn get_linux_distribution() -> Option<String> {
    get_linux_distribution_info().and_then(|info| info.get("ID").cloned())
}

pub fn get_desktop_environment() -> Option<String> {
    if cfg!(target_os = "windows") {
        return Some("windows".to_string());
    }
    if let Ok(de) = std::env::var("XDG_CURRENT_DESKTOP")
        && !de.is_empty()
    {
        return Some(de.to_lowercase());
    }
    if let Ok(ds) = std::env::var("DESKTOP_SESSION")
        && !ds.is_empty()
    {
        return Some(ds.to_lowercase());
    }
    None
}

pub fn get_display_server() -> Option<String> {
    if cfg!(target_os = "windows") {
        return Some("windows".to_string());
    }
    if cfg!(target_os = "macos") {
        return Some("quartz".to_string());
    }
    if let Ok(st) = std::env::var("XDG_SESSION_TYPE")
        && !st.is_empty()
    {
        return Some(st.to_lowercase());
    }
    None
}

pub fn get_kernel_version() -> Option<String> {
    if cfg!(unix) {
        let output = Command::new("uname").arg("-r").output().ok()?;
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    } else if cfg!(target_os = "windows") {
        let output = Command::new("pwsh")
            .arg("-Command")
            .arg("(Get-CimInstance Win32_OperatingSystem).Version")
            .output()
            .ok()?;
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }
    None
}

pub fn get_distro_version() -> Option<String> {
    if let Some(info) = get_linux_distribution_info()
        && let Some(vid) = info.get("VERSION_ID")
    {
        return Some(vid.clone());
    }
    if cfg!(target_os = "macos") {
        let output = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()?;
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    } else if cfg!(target_os = "windows") {
        let output = Command::new("pwsh")
            .arg("-Command")
            .arg("(Get-CimInstance Win32_OperatingSystem).Version")
            .output()
            .ok()?;
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }
    None
}

pub fn get_cpu_info() -> Option<String> {
    if cfg!(target_os = "linux") {
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            for line in cpuinfo.lines() {
                if line.starts_with("model name")
                    && let Some((_, model)) = line.split_once(':')
                {
                    return Some(model.trim().to_string());
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        let output = Command::new("sysctl")
            .arg("-n")
            .arg("machdep.cpu.brand_string")
            .output()
            .ok()?;
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    } else if cfg!(target_os = "windows") {
        let output = Command::new("pwsh")
            .arg("-Command")
            .arg("(Get-CimInstance Win32_Processor).Name")
            .output()
            .ok()?;
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }
    None
}

pub fn get_gpu_info() -> Option<String> {
    if cfg!(target_os = "linux") {
        if let Ok(output) = Command::new("lspci").output()
            && output.status.success()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if (line.contains("VGA compatible controller") || line.contains("3D controller"))
                    && let Some((_, model)) = line.split_once(": ")
                {
                    return Some(model.trim().to_string());
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        let output = Command::new("system_profiler")
            .arg("SPDisplaysDataType")
            .output()
            .ok()?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.trim().starts_with("Chipset Model:")
                    && let Some((_, model)) = line.split_once(':')
                {
                    return Some(model.trim().to_string());
                }
            }
        }
    } else if cfg!(target_os = "windows") {
        let output = Command::new("pwsh")
            .arg("-Command")
            .arg("(Get-CimInstance Win32_VideoController).Name")
            .output()
            .ok()?;
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }
    None
}

/// Identifies the primary package manager for the current operating system.
///
/// This is used to resolve `native:` dependencies.
pub fn get_native_package_manager() -> Option<String> {
    let os = std::env::consts::OS;
    match os {
        "linux" => get_linux_distro_family()
            .map(|family| {
                match family.as_str() {
                    "debian" => "apt",
                    "arch" => "pacman",
                    "fedora" => "dnf",
                    "suse" => "zypper",
                    "gentoo" => "portage",
                    "alpine" => "apk",
                    "void" => "xbps-install",
                    "solus" => "eopkg",
                    "guix" => "guix",
                    _ => "unknown",
                }
                .to_string()
            })
            .filter(|s| s != "unknown"),
        "macos" => {
            if command_exists("brew") {
                Some("brew".to_string())
            } else if command_exists("port") {
                Some("macports".to_string())
            } else {
                None
            }
        }
        "windows" => {
            if command_exists("scoop") {
                Some("scoop".to_string())
            } else if command_exists("choco") {
                Some("choco".to_string())
            } else if command_exists("winget") {
                Some("winget".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Scans the system for all supported package managers.
///
/// This provides the list of available managers shown in `zoi info` and
/// used to validate `manager:` prefixes in dependency strings.
pub fn get_all_available_package_managers() -> Vec<String> {
    let mut managers = Vec::new();
    let all_possible_managers = [
        "apt",
        "pacman",
        "yay",
        "paru",
        "pikaur",
        "trizen",
        "dnf",
        "yum",
        "zypper",
        "portage",
        "apk",
        "snap",
        "flatpak",
        "nix",
        "brew",
        "port",
        "scoop",
        "choco",
        "winget",
        "pkg",
        "pkg_add",
        "xbps-install",
        "eopkg",
        "guix",
        "mas",
    ];

    for manager in &all_possible_managers {
        if command_exists(manager) {
            managers.push(manager.to_string());
        }
    }
    managers.sort();
    managers.dedup();
    managers
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_size_diff(diff: i64) -> String {
    if diff == 0 {
        return "0 B".to_string();
    }
    let sign = if diff > 0 { "+" } else { "-" };
    let bytes = diff.unsigned_abs();
    format!("{} {}", sign, format_bytes(bytes))
}

/// Verifies that a given path is "Safe" and doesn't attempt to escape the base directory.
///
/// This is a critical security check against "Path Traversal" attacks in
/// package archives or Lua scripts.
pub fn is_safe_path(base: &Path, path: &Path) -> bool {
    let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::Prefix(_) => normalized.push(component),
            std::path::Component::RootDir => normalized.push(component),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return false;
                }
            }
            std::path::Component::Normal(p) => normalized.push(p),
        }
    }
    normalized.starts_with(&base)
}

pub fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
    if link.exists() || link.is_symlink() {
        fs::remove_file(link)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_file(target, link).is_err() {
            if fs::hard_link(target, link).is_err() {
                fs::copy(target, link)?;
            }
        }
        Ok(())
    }
}

pub fn is_admin() -> bool {
    #[cfg(unix)]
    {
        nix::unistd::getuid().is_root()
    }
    #[cfg(windows)]
    {
        false
    }
}

pub fn run_shell_command(command_str: &str) -> anyhow::Result<()> {
    let status = if cfg!(target_os = "windows") {
        Command::new("pwsh")
            .arg("-Command")
            .arg(command_str)
            .status()?
    } else {
        Command::new("bash").arg("-c").arg(command_str).status()?
    };
    if !status.success() {
        return Err(anyhow!("Command failed: {}", command_str));
    }
    Ok(())
}

pub fn run_shell_command_quietly(command_str: &str) -> anyhow::Result<()> {
    let status = if cfg!(target_os = "windows") {
        Command::new("pwsh")
            .arg("-Command")
            .arg(command_str)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?
    } else {
        Command::new("bash")
            .arg("-c")
            .arg(command_str)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?
    };
    if !status.success() {
        return Err(anyhow!("Command failed: {}", command_str));
    }
    Ok(())
}

pub fn is_mini_mode() -> bool {
    std::env::var("ZOI_MINI_MODE").is_ok_and(|v| v == "1")
}

pub fn ask_for_confirmation(prompt: &str, yes: bool) -> bool {
    if yes {
        return true;
    }
    if std::env::var("ZOI_TEST").is_ok() || !stdin().is_tty() {
        return false;
    }
    print!("{} [y/N]: ", prompt);
    let _ = stdout().flush();
    let mut input = String::new();
    if stdin().read_line(&mut input).is_err() {
        return false;
    }
    input.trim().eq_ignore_ascii_case("y")
}

pub fn set_path_read_only(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        let mut perms = fs::metadata(entry.path())?.permissions();
        if !perms.readonly() {
            perms.set_readonly(true);
            fs::set_permissions(entry.path(), perms)?;
        }
    }
    Ok(())
}

pub fn set_path_writable(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        let mut perms = fs::metadata(entry.path())?.permissions();
        if perms.readonly() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = perms.mode();
                perms.set_mode(mode | 0o200);
            }
            #[cfg(not(unix))]
            {
                perms.set_readonly(false);
            }
            fs::set_permissions(entry.path(), perms)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
pub fn set_path_owner(path: &Path, owner: &str, group: &str) -> anyhow::Result<()> {
    use nix::unistd::{Gid, Group, Uid, User, chown};
    let uid = if let Ok(u) = owner.parse::<u32>() {
        Some(Uid::from_raw(u))
    } else if !owner.is_empty() {
        Some(
            User::from_name(owner)
                .map_err(|e| anyhow!("Error looking up user '{}': {}", owner, e))?
                .ok_or_else(|| anyhow!("User not found: {}", owner))?
                .uid,
        )
    } else {
        None
    };
    let gid = if let Ok(g) = group.parse::<u32>() {
        Some(Gid::from_raw(g))
    } else if !group.is_empty() {
        Some(
            Group::from_name(group)
                .map_err(|e| anyhow!("Error looking up group '{}': {}", group, e))?
                .ok_or_else(|| anyhow!("Group not found: {}", group))?
                .gid,
        )
    } else {
        None
    };
    chown(path, uid, gid).map_err(|e| anyhow!("Failed to chown '{}': {}", path.display(), e))?;
    Ok(())
}

pub fn is_platform_compatible(current_platform: &str, allowed_platforms: &[String]) -> bool {
    let os_part = current_platform
        .split('-')
        .next()
        .unwrap_or(current_platform);
    let os = match os_part {
        "darwin" => "macos",
        other => other,
    };
    allowed_platforms.iter().any(|p| {
        if let Some(rest) = p.strip_prefix("ci:") {
            let target = rest.split(':').next().unwrap_or_default();
            target == current_platform || target == os
        } else {
            let p_norm = if p == "darwin" { "macos" } else { p };
            p_norm == "all" || p_norm == os || p_norm == current_platform
        }
    })
}

pub fn check_license(license: &str) {
    if license.is_empty() {
        return;
    }
    if license.eq_ignore_ascii_case("Proprietary") || license.eq_ignore_ascii_case("Unknown") {
        return;
    }
    if let Ok(expr) = spdx::Expression::parse(license)
        && !expr.evaluate(|req| match req.license {
            spdx::LicenseItem::Spdx { id, .. } => id.is_osi_approved(),
            spdx::LicenseItem::Other { .. } => false,
        })
    {}
}

pub fn confirm_untrusted_source(
    source_type: &crate::types::SourceType,
    yes: bool,
) -> anyhow::Result<()> {
    if is_mini_mode() {
        return Ok(());
    }
    if source_type == &crate::types::SourceType::OfficialRepo {
        return Ok(());
    }
    let warning_message = match source_type {
        crate::types::SourceType::UntrustedRepo(repo) => {
            format!(
                "The package from repository '@{}' is not an official Zoi repository.",
                repo
            )
        }
        crate::types::SourceType::LocalFile => "You are installing from a local file.".to_string(),
        crate::types::SourceType::Url => "You are installing from a remote URL. This script will be executed with your user's permissions, which could lead to remote code execution if the source is malicious.".to_string(),
        crate::types::SourceType::GitRepo(repo) => format!("You are installing from an external git repository '{}'. This script will be executed with your user's permissions.", repo),
        _ => return Ok(()),
    };
    println!(
        "\n{}: {}",
        "SECURITY WARNING".yellow().bold(),
        warning_message
    );
    if ask_for_confirmation(
        "This source is not trusted. Are you sure you want to continue?",
        yes,
    ) {
        Ok(())
    } else {
        Err(anyhow!("Operation aborted by user."))
    }
}

pub fn expand_placeholders(
    path: &str,
    version_dir: &Path,
    scope: crate::types::Scope,
) -> Result<String> {
    let mut expanded = path.to_string();
    expanded = expanded.replace("${pkgstore}", &version_dir.to_string_lossy());
    expanded = expanded.replace(
        "${usrroot}",
        &crate::sysroot::apply_sysroot(PathBuf::from("/")).to_string_lossy(),
    );
    if let Some(home_dir) = home::home_dir() {
        expanded = expanded.replace("${usrhome}", &home_dir.to_string_lossy());
    }

    let applications_dir = match scope {
        crate::types::Scope::System => PathBuf::from("/Applications"),
        crate::types::Scope::User => home::home_dir()
            .map(|h| h.join("Applications"))
            .unwrap_or_else(|| PathBuf::from("/Applications")),
        crate::types::Scope::Project => std::env::current_dir()
            .unwrap_or_default()
            .join("Applications"),
    };
    expanded = expanded.replace("${applications}", &applications_dir.to_string_lossy());

    Ok(expanded)
}

pub fn get_current_shell() -> Option<Shell> {
    if cfg!(windows) {
        return Some(Shell::PowerShell);
    }
    if let Ok(shell_path) = std::env::var("SHELL") {
        let shell_name = Path::new(&shell_path).file_name()?.to_str()?;
        match shell_name {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            "elvish" => Some(Shell::Elvish),
            "pwsh" => Some(Shell::PowerShell),
            _ => None,
        }
    } else {
        None
    }
}
