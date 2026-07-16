use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;
use zoi_core::types::SandboxConfig;

/// Wraps a command in a secure Linux sandbox using Bubblewrap (bwrap).
///
/// Default-Deny Security Model:
/// - The environment is completely isolated (empty root, no host files).
/// - Only the package's own store directory is mounted read-only by default.
/// - All other resources (Network, System Libraries, Home Data) must be
///   explicitly requested in the `SandboxConfig` within the package definition.
///
/// This prevents malicious or buggy applications from accessing sensitive
/// user data like SSH keys or personal documents.
pub fn wrap_command(
    original_exe: &Path,
    args: &[String],
    config: &SandboxConfig,
    pkg_store_path: &Path,
) -> Result<Command> {
    if !config.enabled {
        let mut cmd = Command::new(original_exe);
        cmd.args(args);
        return Ok(cmd);
    }

    if !zoi_core::utils::command_exists("bwrap") {
        return Err(anyhow!(
            "Bubblewrap ('bwrap') is required for sandboxing but was not found on your system. Please install it."
        ));
    }

    let mut bwrap = Command::new("bwrap");

    bwrap.arg("--unshare-all");
    bwrap.arg("--new-session");

    if config.network {
        bwrap.arg("--share-net");
    }

    bwrap.arg("--tmpfs").arg("/");

    if config.system {
        for dir in &["/usr", "/lib", "/lib64", "/bin", "/sbin"] {
            let path = Path::new(dir);
            if path.exists() {
                bwrap.arg("--ro-bind").arg(dir).arg(dir);
            }
        }

        for file in &["/etc/resolv.conf", "/etc/hosts", "/etc/localtime"] {
            let path = Path::new(file);
            if path.exists() {
                bwrap.arg("--ro-bind").arg(file).arg(file);
            }
        }

        for dir in &["/etc/ssl", "/etc/pki", "/etc/ca-certificates"] {
            let path = Path::new(dir);
            if path.exists() {
                bwrap.arg("--ro-bind").arg(dir).arg(dir);
            }
        }

        bwrap.arg("--dev").arg("/dev");
        bwrap.arg("--proc").arg("/proc");
        bwrap.arg("--tmpfs").arg("/tmp");
        bwrap.arg("--tmpfs").arg("/var");
        bwrap.arg("--tmpfs").arg("/run");
    }

    bwrap
        .arg("--ro-bind")
        .arg(pkg_store_path)
        .arg(pkg_store_path);

    if config.cwd
        && let Ok(cwd) = std::env::current_dir()
    {
        bwrap.arg("--bind").arg(&cwd).arg(&cwd);
        bwrap.arg("--chdir").arg(&cwd);
    }

    for path_str in &config.read {
        let path = expand_home(path_str)?;
        if path.exists() {
            bwrap.arg("--ro-bind").arg(&path).arg(&path);
        }
    }

    for path_str in &config.write {
        let path = expand_home(path_str)?;
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if !path.exists() {
            if path_str.ends_with('/') {
                let _ = std::fs::create_dir_all(&path);
            } else {
                let _ = std::fs::File::create(&path);
            }
        }
        bwrap.arg("--bind").arg(&path).arg(&path);
    }

    bwrap.arg("--clearenv");

    if let Ok(path) = std::env::var("PATH") {
        bwrap.arg("--setenv").arg("PATH").arg(path);
    }
    if let Some(home) = home::home_dir() {
        bwrap
            .arg("--setenv")
            .arg("HOME")
            .arg(home.to_string_lossy().to_string());
    }
    bwrap
        .arg("--setenv")
        .arg("TERM")
        .arg(std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string()));

    if config.system {
        let passthrough_vars = [
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
            "XDG_SESSION_TYPE",
            "DBUS_SESSION_BUS_ADDRESS",
            "DBUS_SYSTEM_BUS_ADDRESS",
            "LANG",
            "LC_ALL",
            "USER",
        ];
        for var in &passthrough_vars {
            if let Ok(val) = std::env::var(var) {
                bwrap.arg("--setenv").arg(var).arg(val);
            }
        }
    }

    for var in &config.env {
        if let Ok(val) = std::env::var(var) {
            bwrap.arg("--setenv").arg(var).arg(val);
        }
    }

    bwrap.arg("--die-with-parent");

    bwrap.arg("--").arg(original_exe);
    bwrap.args(args);

    Ok(bwrap)
}

fn expand_home(path: &str) -> Result<PathBuf> {
    if let Some(stripped) = path.strip_prefix("~/") {
        let home = home::home_dir()
            .ok_or_else(|| anyhow!("Could not find home directory for expansion: {}", path))?;
        Ok(home.join(stripped))
    } else {
        Ok(PathBuf::from(path))
    }
}
