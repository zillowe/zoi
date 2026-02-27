use crate::pkg::{local, types};
use anyhow::{Result, anyhow};
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
                home.join(".config/systemd/user")
                    .join(format!("{}.service", service_name))
            } else {
                PathBuf::from(format!("/etc/systemd/system/{}.service", service_name))
            };
            if unit_path.exists() {
                println!("Removing service unit file: {}", unit_path.display());
                fs::remove_file(unit_path)?;
                let mut cmd = Command::new("systemctl");
                if is_user {
                    cmd.arg("--user");
                }
                cmd.arg("daemon-reload").status()?;
            }
        }
        "macos" => {
            let plist_path = if is_user {
                let home =
                    home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
                home.join("Library/LaunchAgents")
                    .join(format!("{}.plist", service_name))
            } else {
                PathBuf::from(format!("/Library/LaunchDaemons/{}.plist", service_name))
            };
            if plist_path.exists() {
                println!("Removing service plist file: {}", plist_path.display());
                fs::remove_file(plist_path)?;
            }
        }
        "windows" => {
            if service_exists_windows(&service_name)? {
                println!("Removing Windows service: {}", service_name);
                Command::new("sc")
                    .arg("delete")
                    .arg(&service_name)
                    .status()?;
            }
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
            let output = cmd.arg("is-active").arg(&service_name).output()?;
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        "macos" => {
            let output = Command::new("launchctl").arg("list").output()?;
            let list = String::from_utf8_lossy(&output.stdout);
            if list.contains(&service_name) {
                Ok("active".to_string())
            } else {
                Ok("inactive".to_string())
            }
        }
        "windows" => {
            let output = Command::new("sc")
                .arg("query")
                .arg(&service_name)
                .output()?;
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

    let status = cmd.status()?;
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
        let path = home.join(".config/systemd/user");
        fs::create_dir_all(&path)?;
        path.join(format!("{}.service", name))
    } else {
        PathBuf::from(format!("/etc/systemd/system/{}.service", name))
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

    fs::write(&unit_path, content)?;

    let mut cmd = Command::new("systemctl");
    if is_user {
        cmd.arg("--user");
    }
    cmd.arg("daemon-reload").status()?;

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

    match action {
        ServiceAction::Start => {
            Command::new("launchctl")
                .arg("bootstrap")
                .arg(if is_user { "gui" } else { "system" })
                .arg(plist_path)
                .status()?;
        }
        ServiceAction::Stop => {
            Command::new("launchctl")
                .arg("bootout")
                .arg(if is_user { "gui" } else { "system" })
                .arg(plist_path)
                .status()?;
        }
        ServiceAction::Restart => {
            manage_macos_service(name, service, ServiceAction::Stop, scope)?;
            manage_macos_service(name, service, ServiceAction::Start, scope)?;
        }
        ServiceAction::Status => {
            Command::new("launchctl").arg("list").arg(name).status()?;
        }
    }

    Ok(())
}

fn ensure_macos_plist(name: &str, service: &types::Service, is_user: bool) -> Result<PathBuf> {
    let plist_path = if is_user {
        let home = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        let path = home.join("Library/LaunchAgents");
        fs::create_dir_all(&path)?;
        path.join(format!("{}.plist", name))
    } else {
        PathBuf::from(format!("/Library/LaunchDaemons/{}.plist", name))
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

    fs::write(&plist_path, content)?;
    Ok(plist_path)
}

fn manage_windows_service(
    name: &str,
    service: &types::Service,
    action: ServiceAction,
    _scope: types::Scope,
) -> Result<()> {
    match action {
        ServiceAction::Start => {
            if !service_exists_windows(name)? {
                create_windows_service(name, service)?;
            }
            Command::new("sc").arg("start").arg(name).status()?;
        }
        ServiceAction::Stop => {
            Command::new("sc").arg("stop").arg(name).status()?;
        }
        ServiceAction::Restart => {
            Command::new("sc").arg("stop").arg(name).status()?;
            Command::new("sc").arg("start").arg(name).status()?;
        }
        ServiceAction::Status => {
            Command::new("sc").arg("query").arg(name).status()?;
        }
    }
    Ok(())
}

fn service_exists_windows(name: &str) -> Result<bool> {
    let output = Command::new("sc").arg("query").arg(name).output()?;
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

    let status = cmd.status()?;
    if !status.success() {
        return Err(anyhow!("Failed to create Windows service '{}'.", name));
    }
    Ok(())
}
