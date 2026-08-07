//! Secure sandboxing for Zoi using Bubblewrap.
//!
//! This crate provides utilities to wrap commands in isolated environments
//! on Linux, ensuring that packages can run with restricted access to the
//! host system.

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
    if let Some(home) = zoi_core::utils::get_user_home() {
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

/// Wraps a command in a sysroot (chroot-like) using Bubblewrap on Linux.
///
/// This binds the host's sysroot directory to '/' in the sandbox,
/// and provides access to essential system devices and kernels (/dev, /proc, /sys).
pub fn wrap_command_in_root(
    sysroot: &Path,
    exe_inside_root: &Path,
    args: &[String],
    env: &std::collections::HashMap<String, String>,
    extra_binds: &[(PathBuf, PathBuf)],
    fakeroot: bool,
) -> Result<Command> {
    if !zoi_core::utils::command_exists("bwrap") {
        return Err(anyhow!(
            "Bubblewrap ('bwrap') is required for sysroot execution but was not found. Please install it."
        ));
    }

    let actual_sysroot = if sysroot.exists() {
        std::fs::canonicalize(sysroot)?
    } else {
        sysroot.to_path_buf()
    };

    // Ensure essential mount point directories exist in the sysroot
    // bwrap will fail if the mount destination does not exist.
    for dir in &["dev", "proc", "sys", "run", "tmp", "var/run", "root"] {
        let path = actual_sysroot.join(dir);
        if !path.exists() {
            let _ = std::fs::create_dir_all(&path);
        }
    }

    let mut bwrap = Command::new("bwrap");

    // Standard isolation flags
    bwrap.arg("--unshare-all");
    bwrap.arg("--new-session");
    bwrap.arg("--share-net");

    if fakeroot {
        bwrap.arg("--uid").arg("0");
        bwrap.arg("--gid").arg("0");
    }

    // Bind the sysroot to /
    bwrap.arg("--bind").arg(&actual_sysroot).arg("/");

    // Provide merged-usr layout inside the sandbox.
    // If the guest has real directories at /bin, /lib, etc., we bind mount the usr/ counterparts over them.
    // If they are missing or broken absolute symlinks, we force create relative symlinks.
    for (src_rel, guest_dest) in &[
        ("usr/bin", "/bin"),
        ("usr/sbin", "/sbin"),
        ("usr/lib", "/lib"),
        ("usr/lib", "/lib64"),
    ] {
        let dest_rel = guest_dest.strip_prefix("/").unwrap_or(guest_dest);
        let guest_p = actual_sysroot.join(dest_rel);

        let mut force_overlay = false;
        if guest_p.exists() || std::fs::symlink_metadata(&guest_p).is_ok() {
            if let Ok(meta) = std::fs::symlink_metadata(&guest_p) {
                if !meta.file_type().is_symlink() && meta.is_dir() {
                    force_overlay = true;
                } else if let Ok(target) = std::fs::read_link(&guest_p)
                    && target.is_absolute()
                {
                    force_overlay = true;
                }
            }
        } else {
            // Missing entirely, create a relative symlink in the sandbox
            bwrap.arg("--symlink").arg(*src_rel).arg(*guest_dest);
            continue;
        }

        if force_overlay {
            // Overlay the usr/ equivalent using a bind mount inside the sandbox
            bwrap
                .arg("--bind")
                .arg(actual_sysroot.join(src_rel))
                .arg(*guest_dest);
        }
    }

    // Essential system mounts for hardware interaction (grub, etc)
    // Use --dev-bind to give full access to host devices
    bwrap.arg("--dev-bind").arg("/dev").arg("/dev");
    bwrap.arg("--proc").arg("/proc");
    bwrap.arg("--bind").arg("/sys").arg("/sys");
    bwrap.arg("--tmpfs").arg("/run");
    bwrap.arg("--tmpfs").arg("/tmp");
    bwrap.arg("--tmpfs").arg("/var/run");

    // Ensure we start at the root of the guest
    bwrap.arg("--chdir").arg("/");

    // Additional binds (e.g. for ephemeral package symlinks)
    for (host_path, guest_path) in extra_binds {
        bwrap.arg("--bind").arg(host_path).arg(guest_path);
    }

    // Set environment variables
    bwrap.arg("--clearenv");
    for (k, v) in env {
        bwrap.arg("--setenv").arg(k).arg(v);
    }

    // Set user info to look like root inside
    bwrap.arg("--setenv").arg("USER").arg("root");
    bwrap.arg("--setenv").arg("HOME").arg("/root");

    bwrap.arg("--die-with-parent");

    bwrap.arg("--").arg(exe_inside_root);
    bwrap.args(args);

    Ok(bwrap)
}

/// Expands the `~/` prefix in a path string to the user's home directory.
///
/// If the path doesn't start with `~/`, it is returned as-is.
fn expand_home(path: &str) -> Result<PathBuf> {
    if let Some(stripped) = path.strip_prefix("~/") {
        let home = zoi_core::utils::get_user_home()
            .ok_or_else(|| anyhow!("Could not find home directory for expansion: {}", path))?;
        Ok(home.join(stripped))
    } else {
        Ok(PathBuf::from(path))
    }
}
