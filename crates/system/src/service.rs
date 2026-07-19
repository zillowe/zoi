use crate::config::ServiceConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use zoi_install::service::{ServiceAction, manage_service};

pub fn apply_services(services: &HashMap<String, ServiceConfig>) -> Result<()> {
    for (name, cfg) in services {
        let action_str = if cfg.enable { "enable" } else { "disable" };
        let action = if cfg.enable {
            ServiceAction::Enable
        } else {
            ServiceAction::Disable
        };

        println!(
            "{} service {}...",
            if cfg.enable { "Enabling" } else { "Disabling" },
            name
        );

        // Try using Zoi's native service manager first
        if manage_service(name, action).is_ok() {
            continue;
        }

        // Fallback to standard systemctl for system services
        let mut cmd = Command::new("systemctl");
        cmd.arg(action_str).arg("--now").arg(name);

        match cmd.status() {
            Ok(status) if !status.success() => {
                eprintln!(
                    "Warning: Failed to {} service {}: systemctl exited with {}",
                    action_str, name, status
                );
            }
            Err(e) => eprintln!("Warning: Failed to {} service {}: {}", action_str, name, e),
            _ => {}
        }
    }
    Ok(())
}
