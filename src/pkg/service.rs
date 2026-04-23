use crate::pkg::sysroot::apply_sysroot;
use crate::pkg::{local, types};
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Status,
}

pub fn manage_service(package_name: &str, action: ServiceAction) -> Result<()> {
    let installed_packages = local::get_installed_packages()?;
    let manifest = installed_packages
        .iter()
        .find(|p| p.name == package_name)
        .ok_or_else(|| anyhow!("Package '{}' is not installed.", package_name))?;

    let service = manifest.service.as_ref().ok_or_else(|| {
        anyhow!(
            "Package '{}' does not define a background service.",
            package_name
        )
    })?;

    let service_name = format!("zoi-{}", manifest.name);

    match std::env::consts::OS {
        "linux" => manage_linux_service(&service_name, service, action, manifest.scope),
        "macos" => manage_macos_service(&service_name, service, action, manifest.scope),
        "windows" => manage_windows_service(&service_name, service, action, manifest.scope),
        _ => Err(anyhow!("Service management not supported on this OS.")),
    }
}

pub fn list_services() -> Result<Vec<(String, String)>> {
    let installed_packages = local::get_installed_packages()?;
    let mut services = Vec::new();

    for pkg in installed_packages {
        if pkg.service.is_some() {
            let status = get_service_status(&pkg)?;
            services.push((pkg.name.clone(), status));
        }
    }

    Ok(services)
}

pub fn cleanup_service(package_name: &str, scope: types::Scope) -> Result<()> {
    let service_name = format!("zoi-{}", package_name);
    let is_user = scope != types::Scope::System;

    match std::env::consts::OS {
        "linux" => {
            let unit_path = if is_user {
                let home =
                    home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
                apply_sysroot(
                    home.join(".config/systemd/user")
                        .join(format!("{}.service", service_name)),
                )
            } else {
                apply_sysroot(PathBuf::from(format!(
                    "/etc/systemd/system/{}.service",
                    service_name
                )))
            };
            if unit_path.exists() {
                println!("Removing service unit file: {}", unit_path.display());
                fs::remove_file(&unit_path).with_context(|| {
                    format!("Failed to remove unit file: {}", unit_path.display())
                })?;
                if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_err() {
                    let mut cmd = Command::new("systemctl");
                    if is_user {
                        cmd.arg("--user");
                    }
                    cmd.arg("daemon-reload")
                        .status()
                        .context("Failed to run systemctl daemon-reload")?;
                }
            }
        }
        "macos" => {
            let plist_path = if is_user {
                let home =
                    home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
                apply_sysroot(
                    home.join("Library/LaunchAgents")
                        .join(format!("{}.plist", service_name)),
                )
            } else {
                apply_sysroot(PathBuf::from(format!(
                    "/Library/LaunchDaemons/{}.plist",
                    service_name
                )))
            };
            if plist_path.exists() {
                println!("Removing service plist file: {}", plist_path.display());
                fs::remove_file(&plist_path).with_context(|| {
                    format!("Failed to remove plist file: {}", plist_path.display())
                })?;
            }
        }
        "windows"
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_err()
                && service_exists_windows(&service_name)? =>
        {
            println!("Removing Windows service: {}", service_name);
            Command::new("sc")
                .arg("delete")
                .arg(&service_name)
                .status()
                .context("Failed to run sc delete")?;
        }
        _ => {}
    }

    Ok(())
}

fn get_service_status(manifest: &types::InstallManifest) -> Result<String> {
    let service_name = format!("zoi-{}", manifest.name);
    match std::env::consts::OS {
        "linux" => {
            let mut cmd = Command::new("systemctl");
            if manifest.scope != types::Scope::System {
                cmd.arg("--user");
            }
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_ok() {
                return Ok("inactive".to_string());
            }
            let output = cmd
                .arg("is-active")
                .arg(&service_name)
                .output()
                .context("Failed to run systemctl is-active")?;
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        "macos" => {
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_ok() {
                return Ok("inactive".to_string());
            }
            let output = Command::new("launchctl")
                .arg("list")
                .output()
                .context("Failed to run launchctl list")?;
            let list = String::from_utf8_lossy(&output.stdout);
            if list.contains(&service_name) {
                Ok("active".to_string())
            } else {
                Ok("inactive".to_string())
            }
        }
        "windows" => {
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_ok() {
                return Ok("inactive".to_string());
            }
            let output = Command::new("sc")
                .arg("query")
                .arg(&service_name)
                .output()
                .context("Failed to run sc query")?;
            let out = String::from_utf8_lossy(&output.stdout);
            if out.contains("RUNNING") {
                Ok("active".to_string())
            } else {
                Ok("inactive".to_string())
            }
        }
        _ => Ok("unknown".to_string()),
    }
}

fn manage_linux_service(
    name: &str,
    service: &types::Service,
    action: ServiceAction,
    scope: types::Scope,
) -> Result<()> {
    let is_user = scope != types::Scope::System;

    ensure_linux_unit_file(name, service, is_user)?;

    if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_ok() {
        return Ok(());
    }

    let mut cmd = Command::new("systemctl");
    if is_user {
        cmd.arg("--user");
    }

    match action {
        ServiceAction::Start => {
            cmd.arg("start").arg(name);
        }
        ServiceAction::Stop => {
            cmd.arg("stop").arg(name);
        }
        ServiceAction::Restart => {
            cmd.arg("restart").arg(name);
        }
        ServiceAction::Status => {
            cmd.arg("status").arg(name);
        }
    }

    let status = cmd
        .status()
        .with_context(|| format!("Failed to run systemctl for action {:?}", name))?;
    if !status.success() {
        return Err(anyhow!(
            "Failed to {} service '{}'.",
            match action {
                ServiceAction::Start => "start",
                ServiceAction::Stop => "stop",
                ServiceAction::Restart => "restart",
                ServiceAction::Status => "get status of",
            },
            name
        ));
    }

    Ok(())
}

fn ensure_linux_unit_file(name: &str, service: &types::Service, is_user: bool) -> Result<()> {
    let unit_path = if is_user {
        let home = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        let path = apply_sysroot(home.join(".config/systemd/user"));
        fs::create_dir_all(&path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;
        path.join(format!("{}.service", name))
    } else {
        apply_sysroot(PathBuf::from(format!(
            "/etc/systemd/system/{}.service",
            name
        )))
    };

    if unit_path.exists() {
        return Ok(());
    }

    let mut content = String::from(
        "[Unit]
Description=Zoi managed service: ",
    );
    content.push_str(name);
    content.push_str(
        "

[Service]
ExecStart=",
    );
    content.push_str(&service.run);

    if let Some(dir) = &service.working_dir {
        content.push_str(
            "
WorkingDirectory=",
        );
        content.push_str(dir);
    }

    if let Some(envs) = &service.env {
        for (k, v) in envs {
            content.push_str(&format!("\nEnvironment=\"{}={}\"", k, v));
        }
    }

    if let Some(log) = &service.log_path {
        content.push_str("\nStandardOutput=append:");
        content.push_str(log);
    }
    if let Some(err_log) = &service.error_log_path {
        content.push_str("\nStandardError=append:");
        content.push_str(err_log);
    }

    content.push_str("\n\n[Install]\nWantedBy=");
    content.push_str(if is_user {
        "default.target"
    } else {
        "multi-user.target"
    });
    content.push('\n');

    fs::write(&unit_path, content)
        .with_context(|| format!("Failed to write unit file: {}", unit_path.display()))?;

    if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_err() {
        let mut cmd = Command::new("systemctl");
        if is_user {
            cmd.arg("--user");
        }
        cmd.arg("daemon-reload")
            .status()
            .context("Failed to run systemctl daemon-reload")?;
    }

    Ok(())
}

fn manage_macos_service(
    name: &str,
    service: &types::Service,
    action: ServiceAction,
    scope: types::Scope,
) -> Result<()> {
    let is_user = scope != types::Scope::System;
    let plist_path = ensure_macos_plist(name, service, is_user)?;

    if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_ok() {
        return Ok(());
    }

    match action {
        ServiceAction::Start => {
            Command::new("launchctl")
                .arg("bootstrap")
                .arg(if is_user { "gui" } else { "system" })
                .arg(plist_path)
                .status()
                .context("Failed to run launchctl bootstrap")?;
        }
        ServiceAction::Stop => {
            Command::new("launchctl")
                .arg("bootout")
                .arg(if is_user { "gui" } else { "system" })
                .arg(plist_path)
                .status()
                .context("Failed to run launchctl bootout")?;
        }
        ServiceAction::Restart => {
            manage_macos_service(name, service, ServiceAction::Stop, scope)?;
            manage_macos_service(name, service, ServiceAction::Start, scope)?;
        }
        ServiceAction::Status => {
            Command::new("launchctl")
                .arg("list")
                .arg(name)
                .status()
                .context("Failed to run launchctl list")?;
        }
    }

    Ok(())
}

fn ensure_macos_plist(name: &str, service: &types::Service, is_user: bool) -> Result<PathBuf> {
    let plist_path = if is_user {
        let home = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        let path = apply_sysroot(home.join("Library/LaunchAgents"));
        fs::create_dir_all(&path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;
        path.join(format!("{}.plist", name))
    } else {
        apply_sysroot(PathBuf::from(format!(
            "/Library/LaunchDaemons/{}.plist",
            name
        )))
    };

    if plist_path.exists() {
        return Ok(plist_path);
    }

    let mut content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
"#,
        name
    );

    for part in service.run.split_whitespace() {
        content.push_str(&format!(
            "        <string>{}</string>
",
            part
        ));
    }

    content.push_str(
        "    </array>
",
    );

    if let Some(dir) = &service.working_dir {
        content.push_str(&format!(
            "    <key>WorkingDirectory</key>
    <string>{}</string>
",
            dir
        ));
    }

    if let Some(envs) = &service.env {
        content.push_str(
            "    <key>EnvironmentVariables</key>
    <dict>
",
        );
        for (k, v) in envs {
            content.push_str(&format!(
                "        <key>{}</key>
        <string>{}</string>
",
                k, v
            ));
        }
        content.push_str(
            "    </dict>
",
        );
    }

    if let Some(log) = &service.log_path {
        content.push_str(&format!(
            "    <key>StandardOutPath</key>
    <string>{}</string>
",
            log
        ));
    }
    if let Some(err_log) = &service.error_log_path {
        content.push_str(&format!(
            "    <key>StandardErrorPath</key>
    <string>{}</string>
",
            err_log
        ));
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

    fs::write(&plist_path, content)
        .with_context(|| format!("Failed to write plist file: {}", plist_path.display()))?;
    Ok(plist_path)
}

fn manage_windows_service(
    name: &str,
    service: &types::Service,
    action: ServiceAction,
    _scope: types::Scope,
) -> Result<()> {
    if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_ok() {
        return Ok(());
    }

    match action {
        ServiceAction::Start => {
            if !service_exists_windows(name)? {
                create_windows_service(name, service)?;
            }
            Command::new("sc")
                .arg("start")
                .arg(name)
                .status()
                .context("Failed to run sc start")?;
        }
        ServiceAction::Stop => {
            Command::new("sc")
                .arg("stop")
                .arg(name)
                .status()
                .context("Failed to run sc stop")?;
        }
        ServiceAction::Restart => {
            Command::new("sc")
                .arg("stop")
                .arg(name)
                .status()
                .context("Failed to run sc stop (restart)")?;
            Command::new("sc")
                .arg("start")
                .arg(name)
                .status()
                .context("Failed to run sc start (restart)")?;
        }
        ServiceAction::Status => {
            Command::new("sc")
                .arg("query")
                .arg(name)
                .status()
                .context("Failed to run sc query")?;
        }
    }
    Ok(())
}

fn service_exists_windows(name: &str) -> Result<bool> {
    let output = Command::new("sc")
        .arg("query")
        .arg(name)
        .output()
        .context("Failed to run sc query (exists check)")?;
    Ok(output.status.success())
}

fn create_windows_service(name: &str, service: &types::Service) -> Result<()> {
    let mut cmd = Command::new("sc");
    cmd.arg("create")
        .arg(name)
        .arg(format!("binPath={}", service.run));

    if service.run_at_load {
        cmd.arg("start=auto");
    }

    let status = cmd.status().context("Failed to run sc create")?;
    if !status.success() {
        return Err(anyhow!("Failed to create Windows service '{}'.", name));
    }
    Ok(())
}
