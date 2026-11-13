//! Secure sandboxing for Zoi using Bubblewrap.
//!
//! This crate provides utilities to wrap commands in isolated environments
//! on Linux, ensuring that packages can run with restricted access to the
//! host system.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Result, anyhow};
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
///
/// # Errors
///
/// Returns an error if:
/// - Bubblewrap (`bwrap`) is not installed on the system.
/// - The `SandboxConfig` contains invalid home directory expansions.
/// - Required system resources cannot be accessed or created.
pub fn wrap_command(
    original_exe: &Path,
    args: &[String],
    config: &SandboxConfig,
    pkg_store_path: &Path
) -> Result<Command> {
    if !config.enabled {
        let mut cmd = Command::new(original_exe);
        cmd.args(args);
        return Ok(cmd);
    }

    if !zoi_core::utils::command_exists("bwrap") {
        return Err(anyhow!(
            "Bubblewrap ('bwrap') is required for sandboxing but was not \
             found on your system. Please install it."
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
        // Write paths come from package metadata. Only paths inside the
        // user's home (but not the home directory itself), Zoi's user data
        // directory or the package's own store directory are honoured, and
        // nothing is created on the host before that check passes.
        let path = match resolve_sandbox_write_path(path_str, pkg_store_path) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Warning: ignoring sandbox write path: {e}");
                continue;
            }
        };
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
        if !path.exists() {
            continue;
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
    bwrap.arg("--setenv").arg("TERM").arg(
        std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string())
    );

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
            "USER"
        ];
        for var in &passthrough_vars {
            if let Ok(val) = std::env::var(var) {
                bwrap.arg("--setenv").arg(var).arg(val);
            }
        }
    }

    for var in &config.env {
        if is_sensitive_env_var(var) {
            eprintln!(
                "Warning: not forwarding sensitive environment variable \
                 '{var}' into the sandbox."
            );
            continue;
        }
        if let Ok(val) = std::env::var(var) {
            bwrap.arg("--setenv").arg(var).arg(val);
        }
    }

    bwrap.arg("--die-with-parent");

    bwrap.arg("--").arg(original_exe);
    bwrap.args(args);

    Ok(bwrap)
}

/// Selects how much of the host a sysroot sandbox may touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootProfile {
    /// Root hooks, `exec` and `shell` inside a sysroot. Host devices are
    /// bound (`--dev-bind /dev`) and `/sys` is writable, because tools such as
    /// bootloader installers need real hardware access.
    System,
    /// Package builds (`--pure`). Only a minimal, synthetic `/dev` is provided
    /// (`--dev /dev`) and `/sys` is mounted read-only. Networking is kept so
    /// builds can fetch sources.
    Build
}

/// Wraps a command in a sysroot (chroot-like) using Bubblewrap on Linux.
///
/// This binds the host's sysroot directory to '/' in the sandbox,
/// and provides access to essential system devices and kernels (/dev, /proc,
/// /sys). It uses [`RootProfile::System`]; package builds should use
/// [`wrap_command_in_root_with_profile`] with [`RootProfile::Build`].
///
/// # Errors
///
/// Returns an error if:
/// - Bubblewrap (`bwrap`) is not found.
/// - The sysroot path cannot be canonicalized.
/// - Essential directories cannot be created in the sysroot.
pub fn wrap_command_in_root<S: std::hash::BuildHasher>(
    sysroot: &Path,
    exe_inside_root: &Path,
    args: &[String],
    env: &std::collections::HashMap<String, String, S>,
    extra_binds: &[(PathBuf, PathBuf)],
    fakeroot: bool
) -> Result<Command> {
    wrap_command_in_root_with_profile(
        sysroot,
        exe_inside_root,
        args,
        env,
        extra_binds,
        fakeroot,
        RootProfile::System
    )
}

/// Wraps a command in a sysroot using Bubblewrap with an explicit
/// [`RootProfile`].
///
/// # Errors
///
/// Returns an error if:
/// - Bubblewrap (`bwrap`) is not found.
/// - The sysroot path cannot be canonicalized.
/// - Essential directories cannot be created in the sysroot.
pub fn wrap_command_in_root_with_profile<S: std::hash::BuildHasher>(
    sysroot: &Path,
    exe_inside_root: &Path,
    args: &[String],
    env: &std::collections::HashMap<String, String, S>,
    extra_binds: &[(PathBuf, PathBuf)],
    fakeroot: bool,
    profile: RootProfile
) -> Result<Command> {
    if !zoi_core::utils::command_exists("bwrap") {
        return Err(anyhow!(
            "Bubblewrap ('bwrap') is required for sysroot execution but was \
             not found. Please install it."
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
    // If the guest has real directories at /bin, /lib, etc., we bind mount the
    // usr/ counterparts over them. If they are missing or broken absolute
    // symlinks, we force create relative symlinks.
    for (src_rel, guest_dest) in &[
        ("usr/bin", "/bin"),
        ("usr/sbin", "/sbin"),
        ("usr/lib", "/lib"),
        ("usr/lib", "/lib64")
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

    match profile {
        RootProfile::System => {
            // Essential system mounts for hardware interaction (grub, etc)
            // Use --dev-bind to give full access to host devices
            bwrap.arg("--dev-bind").arg("/dev").arg("/dev");
            bwrap.arg("--proc").arg("/proc");
            bwrap.arg("--bind").arg("/sys").arg("/sys");
        }
        RootProfile::Build => {
            // Builds get a minimal synthetic /dev (null, zero, random, tty,
            // ...) and a read-only /sys: no host block devices, no writable
            // kernel knobs.
            bwrap.arg("--dev").arg("/dev");
            bwrap.arg("--proc").arg("/proc");
            bwrap.arg("--ro-bind").arg("/sys").arg("/sys");
        }
    }
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

/// Reports whether an environment variable name looks like it carries a secret
/// and must therefore never be forwarded into a sandbox, even when a package
/// asks for it.
///
/// Matches (case-insensitively) names containing `TOKEN`, `SECRET`,
/// `PASSWORD`, `PASSWD`, `PASSPHRASE`, `CREDENTIAL` or `PRIVATE`, names ending
/// in `_KEY`, `_PASS` or `_AUTH`, cloud credential prefixes (`AWS_`, `AZURE_`,
/// `GOOGLE_APPLICATION_CREDENTIALS`) and a few well-known agent variables
/// (`GPG_PASSWORD`, `SSH_AUTH_SOCK`, `GPG_AGENT_INFO`).
pub fn is_sensitive_env_var(name: &str) -> bool {
    const EXACT: &[&str] = &[
        "GPG_PASSWORD",
        "SSH_AUTH_SOCK",
        "GPG_AGENT_INFO",
        "KEY",
        "GOOGLE_APPLICATION_CREDENTIALS"
    ];
    const CONTAINS: &[&str] = &[
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "PASSPHRASE",
        "CREDENTIAL",
        "PRIVATE"
    ];
    const PREFIXES: &[&str] = &["AWS_", "AZURE_"];
    const SUFFIXES: &[&str] = &["_KEY", "_PASS", "_AUTH"];

    let upper = name.to_ascii_uppercase();
    EXACT.contains(&upper.as_str())
        || CONTAINS.iter().any(|needle| upper.contains(needle))
        || PREFIXES.iter().any(|prefix| upper.starts_with(prefix))
        || SUFFIXES.iter().any(|suffix| upper.ends_with(suffix))
}

/// Resolves a sandbox `write` path from package metadata and checks that it is
/// allowed.
///
/// The path (after `~/` expansion) must be absolute, must not contain `..`
/// components, and must lie strictly inside the user's home directory, inside
/// Zoi's user data directory, or inside `pkg_store_path`. The home directory
/// itself is rejected because binding it writable would defeat the sandbox.
/// Symlinks in the existing part of the path are resolved before the check.
///
/// # Errors
///
/// Returns an error describing why the path was rejected.
pub fn resolve_sandbox_write_path(
    path_str: &str,
    pkg_store_path: &Path
) -> Result<PathBuf> {
    let path = expand_home(path_str)?;
    if !path.is_absolute() {
        return Err(anyhow!("'{path_str}' is not an absolute path"));
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(anyhow!("'{path_str}' contains '..'"));
    }

    let home = zoi_core::utils::get_user_home().map(|h| canonical_or_self(&h));
    let store = canonical_or_self(pkg_store_path);
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = &home {
        roots.push(home.clone());
    }
    if let Ok(data_dir) = zoi_core::utils::get_user_data_dir() {
        roots.push(canonical_or_self(&data_dir));
    }

    let resolved = resolve_existing_prefix(&path);

    if home.as_deref() == Some(resolved.as_path()) {
        return Err(anyhow!(
            "'{path_str}' is the home directory itself; request specific \
             subdirectories instead"
        ));
    }

    // Strictly below the home or data directory, or the package's own store
    // directory (or anything below it).
    let allowed = resolved.starts_with(&store)
        || roots
            .iter()
            .any(|root| resolved.starts_with(root) && resolved != *root);
    if !allowed {
        return Err(anyhow!(
            "'{path_str}' is outside the user's home directory and the \
             package's data directories"
        ));
    }

    Ok(path)
}

/// Canonicalizes a path, falling back to the path itself when it does not
/// exist.
fn canonical_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Resolves symlinks in the longest existing prefix of `path` and re-appends
/// the not-yet-existing remainder.
fn resolve_existing_prefix(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut remainder = Vec::new();
    loop {
        if let Ok(canonical) = std::fs::canonicalize(&existing) {
            let mut result = canonical;
            for part in remainder.iter().rev() {
                result.push(part);
            }
            return result;
        }
        let Some(name) =
            existing.file_name().map(std::ffi::OsStr::to_os_string)
        else {
            return path.to_path_buf();
        };
        remainder.push(name);
        if !existing.pop() {
            return path.to_path_buf();
        }
    }
}

/// Expands the `~/` prefix in a path string to the user's home directory.
///
/// If the path doesn't start with `~/`, it is returned as-is.
fn expand_home(path: &str) -> Result<PathBuf> {
    if let Some(stripped) = path.strip_prefix("~/") {
        let home = zoi_core::utils::get_user_home().ok_or_else(|| {
            anyhow!("Could not find home directory for expansion: {path}")
        })?;
        Ok(home.join(stripped))
    } else {
        Ok(PathBuf::from(path))
    }
}
