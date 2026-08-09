use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use zoi_core::{sysroot, types, utils};
use zoi_resolver::local;

/// Actions that can be performed on a background service.
#[derive(Debug, Clone, Copy, PartialEq, Eq,)]
pub enum ServiceAction {
    /// Start the service.
    Start,
    /// Stop the service.
    Stop,
    /// Restart the service.
    Restart,
    /// Check the status of the service.
    Status,
    /// Enable the service to start automatically.
    Enable,
    /// Disable the service from starting automatically.
    Disable,
}

/// Manages a service for a given package.
///
/// # Errors
///
/// Returns an error if the package is not installed, if it does not define
/// a background service, or if the service management command fails.
pub fn manage_service(
    package_name: &str,
    action: ServiceAction,
) -> Result<(),> {
    let installed_packages = local::get_installed_packages()?;
    let manifest = installed_packages
        .iter()
        .find(|p| p.name == package_name,)
        .ok_or_else(|| anyhow!("Package '{package_name}' is not installed."),)?;

    let service = manifest.service.as_ref().ok_or_else(|| {
        anyhow!(
            "Package '{package_name}' does not define a background service."
        )
    },)?;

    let service_name = format!("zoi-{}", manifest.name);

    match std::env::consts::OS {
        "linux" => {
            manage_linux_service(&service_name, service, action, manifest.scope,)
        }
        "macos" => {
            manage_macos_service(&service_name, service, action, manifest.scope,)
        }
        "windows" => manage_windows_service(
            &service_name,
            service,
            action,
            manifest.scope,
        ),
        _ => Err(anyhow!("Service management not supported on this OS."),),
    }
}

/// Lists all installed services and their current status.
///
/// # Errors
///
/// Returns an error if installed packages cannot be retrieved or if
/// querying service status fails.
pub fn list_services() -> Result<Vec<(String, String,),>,> {
    let installed_packages = local::get_installed_packages()?;
    let mut services = Vec::new();

    for pkg in installed_packages {
        if pkg.service.is_some() {
            let status = get_service_status(&pkg,)?;
            services.push((pkg.name.clone(), status,),);
        }
    }

    Ok(services,)
}

/// Cleans up service files for a given package.
///
/// # Errors
///
/// Returns an error if service files cannot be removed or if
/// the service manager cannot be reloaded.
pub fn cleanup_service(
    package_name: &str, scope: types::Scope,
) -> Result<(),> {
    let service_name = format!("zoi-{package_name}");
    let is_user = scope != types::Scope::System;

    match std::env::consts::OS {
        "linux" => {
            let unit_path = if is_user {
                let home = utils::get_user_home()
                    .ok_or_else(|| anyhow!("Could not find home directory"),)?;
                sysroot::apply_sysroot(
                    home.join(".config/systemd/user",)
                        .join(format!("{service_name}.service"),),
                )
            } else {
                sysroot::apply_sysroot(PathBuf::from(format!(
                    "/etc/systemd/system/{service_name}.service"
                ),),)
            };
            if unit_path.exists() {
                println!("Removing service unit file: {}", unit_path.display());
                fs::remove_file(&unit_path,).with_context(|| {
                    format!(
                        "Failed to remove unit file: {}",
                        unit_path.display()
                    )
                },)?;
                if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS",).is_err() {
                    let mut cmd = Command::new("systemctl",);
                    if is_user {
                        cmd.arg("--user",);
                    }
                    cmd.arg("daemon-reload",)
                        .status()
                        .context("Failed to run systemctl daemon-reload",)?;
                }
            }
        }
        "macos" => {
            let plist_path = if is_user {
                let home = utils::get_user_home()
                    .ok_or_else(|| anyhow!("Could not find home directory"),)?;
                sysroot::apply_sysroot(
                    home.join("Library/LaunchAgents",)
                        .join(format!("{service_name}.plist"),),
                )
            } else {
                sysroot::apply_sysroot(PathBuf::from(format!(
                    "/Library/LaunchDaemons/{service_name}.plist"
                ),),)
            };
            if plist_path.exists() {
                println!(
                    "Removing service plist file: {}",
                    plist_path.display()
                );
                fs::remove_file(&plist_path,).with_context(|| {
                    format!(
                        "Failed to remove plist file: {}",
                        plist_path.display()
                    )
                },)?;
            }
        }
        "windows"
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS",).is_err()
                && service_exists_windows(&service_name,)? =>
        {
            println!("Removing Windows service: {service_name}");
            Command::new("sc",)
                .arg("delete",)
                .arg(&service_name,)
                .status()
                .context("Failed to run sc delete",)?;
        }
        _ => {}
    }

    Ok((),)
}

/// Gets the current status of a service.
fn get_service_status(manifest: &types::InstallManifest,) -> Result<String,> {
    let service_name = format!("zoi-{}", manifest.name);
    match std::env::consts::OS {
        "linux" => {
            let mut cmd = Command::new("systemctl",);
            if manifest.scope != types::Scope::System {
                cmd.arg("--user",);
            }
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS",).is_ok() {
                return Ok("inactive".to_string(),);
            }
            let output = cmd
                .arg("is-active",)
                .arg(&service_name,)
                .output()
                .context("Failed to run systemctl is-active",)?;
            Ok(String::from_utf8_lossy(&output.stdout,).trim().to_string(),)
        }
        "macos" => {
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS",).is_ok() {
                return Ok("inactive".to_string(),);
            }
            let output = Command::new("launchctl",)
                .arg("list",)
                .output()
                .context("Failed to run launchctl list",)?;
            let list = String::from_utf8_lossy(&output.stdout,);
            if list.contains(&service_name,) {
                Ok("active".to_string(),)
            } else {
                Ok("inactive".to_string(),)
            }
        }
        "windows" => {
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS",).is_ok() {
                return Ok("inactive".to_string(),);
            }
            let output = Command::new("sc",)
                .arg("query",)
                .arg(&service_name,)
                .output()
                .context("Failed to run sc query",)?;
            let out = String::from_utf8_lossy(&output.stdout,);
            if out.contains("RUNNING",) {
                Ok("active".to_string(),)
            } else {
                Ok("inactive".to_string(),)
            }
        }
        _ => Ok("unknown".to_string(),),
    }
}

/// Manages a Linux systemd service.
fn manage_linux_service(
    name: &str,
    service: &types::Service,
    action: ServiceAction,
    scope: types::Scope,
) -> Result<(),> {
    let is_user = scope != types::Scope::System;

    ensure_linux_unit_file(name, service, is_user,)?;

    if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS",).is_ok() {
        return Ok((),);
    }

    let mut cmd = Command::new("systemctl",);
    if is_user {
        cmd.arg("--user",);
    }

    match action {
        ServiceAction::Start => {
            cmd.arg("start",).arg(name,);
        }
        ServiceAction::Stop => {
            cmd.arg("stop",).arg(name,);
        }
        ServiceAction::Restart => {
            cmd.arg("restart",).arg(name,);
        }
        ServiceAction::Status => {
            cmd.arg("status",).arg(name,);
        }
        ServiceAction::Enable => {
            cmd.arg("enable",).arg("--now",).arg(name,);
        }
        ServiceAction::Disable => {
            cmd.arg("disable",).arg("--now",).arg(name,);
        }
    }

    let status = cmd.status().with_context(|| {
        format!("Failed to run systemctl for action {name:?}")
    },)?;
    if !status.success() {
        return Err(anyhow!("Failed to perform service action on '{name}'."),);
    }

    Ok((),)
}

/// Ensures a Linux systemd unit file exists.
fn ensure_linux_unit_file(
    name: &str,
    service: &types::Service,
    is_user: bool,
) -> Result<(),> {
    let unit_path = if is_user {
        let home = utils::get_user_home()
            .ok_or_else(|| anyhow!("Could not find home directory"),)?;
        let path = sysroot::apply_sysroot(home.join(".config/systemd/user",),);
        fs::create_dir_all(&path,).with_context(|| {
            format!("Failed to create directory: {}", path.display())
        },)?;
        path.join(format!("{name}.service"),)
    } else {
        sysroot::apply_sysroot(PathBuf::from(format!(
            "/etc/systemd/system/{name}.service"
        ),),)
    };

    if unit_path.exists() {
        return Ok((),);
    }

    let mut content = String::from(
        "[Unit]
Description=Zoi managed service: ",
    );
    content.push_str(name,);
    content.push_str(
        "

[Service]
ExecStart=",
    );
    content.push_str(&service.run,);

    if let Some(dir,) = &service.working_dir {
        content.push_str(
            "
WorkingDirectory=",
        );
        content.push_str(dir,);
    }

    if let Some(envs,) = &service.env {
        for (k, v,) in envs {
            let _ = write!(content, "\nEnvironment=\"{k}={v}\"");
        }
    }

    if let Some(log,) = &service.log_path {
        content.push_str("\nStandardOutput=append:",);
        content.push_str(log,);
    }
    if let Some(err_log,) = &service.error_log_path {
        content.push_str("\nStandardError=append:",);
        content.push_str(err_log,);
    }

    content.push_str("\n\n[Install]\nWantedBy=",);
    content.push_str(if is_user {
        "default.target"
    } else {
        "multi-user.target"
    },);
    content.push('\n',);

    fs::write(&unit_path, content,).with_context(|| {
        format!("Failed to write unit file: {}", unit_path.display())
    },)?;

    if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS",).is_err() {
        let mut cmd = Command::new("systemctl",);
        if is_user {
            cmd.arg("--user",);
        }
        cmd.arg("daemon-reload",)
            .status()
            .context("Failed to run systemctl daemon-reload",)?;
    }

    Ok((),)
}

/// Manages a macOS launchd service.
fn manage_macos_service(
    name: &str,
    service: &types::Service,
    action: ServiceAction,
    scope: types::Scope,
) -> Result<(),> {
    let is_user = scope != types::Scope::System;
    let plist_path = ensure_macos_plist(name, service, is_user,)?;

    if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS",).is_ok() {
        return Ok((),);
    }

    match action {
        ServiceAction::Start | ServiceAction::Enable => {
            Command::new("launchctl",)
                .arg("bootstrap",)
                .arg(if is_user { "gui" } else { "system" },)
                .arg(plist_path,)
                .status()
                .context("Failed to run launchctl bootstrap",)?;
        }
        ServiceAction::Stop | ServiceAction::Disable => {
            Command::new("launchctl",)
                .arg("bootout",)
                .arg(if is_user { "gui" } else { "system" },)
                .arg(plist_path,)
                .status()
                .context("Failed to run launchctl bootout",)?;
        }
        ServiceAction::Restart => {
            manage_macos_service(name, service, ServiceAction::Stop, scope,)?;
            manage_macos_service(name, service, ServiceAction::Start, scope,)?;
        }
        ServiceAction::Status => {
            Command::new("launchctl",)
                .arg("list",)
                .arg(name,)
                .status()
                .context("Failed to run launchctl list",)?;
        }
    }

    Ok((),)
}

/// Ensures a macOS launchd plist file exists.
fn ensure_macos_plist(
    name: &str,
    service: &types::Service,
    is_user: bool,
) -> Result<PathBuf,> {
    let plist_path = if is_user {
        let home = utils::get_user_home()
            .ok_or_else(|| anyhow!("Could not find home directory"),)?;
        let path = sysroot::apply_sysroot(home.join("Library/LaunchAgents",),);
        fs::create_dir_all(&path,).with_context(|| {
            format!("Failed to create directory: {}", path.display())
        },)?;
        path.join(format!("{name}.plist"),)
    } else {
        sysroot::apply_sysroot(PathBuf::from(format!(
            "/Library/LaunchDaemons/{name}.plist"
        ),),)
    };

    if plist_path.exists() {
        return Ok(plist_path,);
    }

    let mut content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{name}</string>
    <key>ProgramArguments</key>
    <array>
"#
    );

    for part in service.run.split_whitespace() {
        let _ = writeln!(content, "        <string>{part}</string>");
    }

    content.push_str("    </array>\n",);

    if let Some(dir,) = &service.working_dir {
        let _ = write!(
            content,
            "    <key>WorkingDirectory</key>\n    <string>{dir}</string>\n"
        );
    }

    if let Some(envs,) = &service.env {
        content.push_str("    <key>EnvironmentVariables</key>\n    <dict>\n",);
        for (k, v,) in envs {
            let _ = write!(
                content,
                "        <key>{k}</key>\n        <string>{v}</string>\n"
            );
        }
        content.push_str("    </dict>\n",);
    }

    if let Some(log,) = &service.log_path {
        let _ = write!(
            content,
            "    <key>StandardOutPath</key>\n    <string>{log}</string>\n"
        );
    }
    if let Some(err_log,) = &service.error_log_path {
        let _ = write!(
            content,
            "    <key>StandardErrorPath</key>\n    \
             <string>{err_log}</string>\n"
        );
    }

    if service.run_at_load {
        content.push_str(
            "    <key>RunAtLoad</key>
    <true/>
",
        );
    }

    content.push_str(
        "</dict>
</plist>
",
    );

    fs::write(&plist_path, content,).with_context(|| {
        format!("Failed to write plist file: {}", plist_path.display())
    },)?;
    Ok(plist_path,)
}

/// Manages a Windows service.
fn manage_windows_service(
    name: &str,
    service: &types::Service,
    action: ServiceAction,
    _scope: types::Scope,
) -> Result<(),> {
    if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS",).is_ok() {
        return Ok((),);
    }

    match action {
        ServiceAction::Start => {
            if !service_exists_windows(name,)? {
                create_windows_service(name, service,)?;
            }
            Command::new("sc",)
                .arg("start",)
                .arg(name,)
                .status()
                .context("Failed to run sc start",)?;
        }
        ServiceAction::Stop => {
            Command::new("sc",)
                .arg("stop",)
                .arg(name,)
                .status()
                .context("Failed to run sc stop",)?;
        }
        ServiceAction::Restart => {
            Command::new("sc",)
                .arg("stop",)
                .arg(name,)
                .status()
                .context("Failed to run sc stop (restart)",)?;
            Command::new("sc",)
                .arg("start",)
                .arg(name,)
                .status()
                .context("Failed to run sc start (restart)",)?;
        }
        ServiceAction::Status => {
            Command::new("sc",)
                .arg("query",)
                .arg(name,)
                .status()
                .context("Failed to run sc query",)?;
        }
        ServiceAction::Enable => {
            if !service_exists_windows(name,)? {
                create_windows_service(name, service,)?;
            }
            Command::new("sc",)
                .arg("config",)
                .arg(name,)
                .arg("start=auto",)
                .status()?;
            Command::new("sc",).arg("start",).arg(name,).status()?;
        }
        ServiceAction::Disable => {
            Command::new("sc",).arg("stop",).arg(name,).status()?;
            Command::new("sc",)
                .arg("config",)
                .arg(name,)
                .arg("start=disabled",)
                .status()?;
        }
    }
    Ok((),)
}

/// Checks if a Windows service exists.
fn service_exists_windows(name: &str,) -> Result<bool,> {
    let output = Command::new("sc",)
        .arg("query",)
        .arg(name,)
        .output()
        .context("Failed to run sc query (exists check)",)?;
    Ok(output.status.success(),)
}

/// Creates a Windows service.
fn create_windows_service(
    name: &str, service: &types::Service,
) -> Result<(),> {
    let mut cmd = Command::new("sc",);
    cmd.arg("create",)
        .arg(name,)
        .arg(format!("binPath={}", service.run),);

    if service.run_at_load {
        cmd.arg("start=auto",);
    }

    let status = cmd.status().context("Failed to run sc create",)?;
    if !status.success() {
        return Err(anyhow!("Failed to create Windows service '{name}'."),);
    }
    Ok((),)
}
