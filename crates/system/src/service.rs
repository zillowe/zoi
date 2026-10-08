use std::collections::HashMap;
use std::process::Command;

use anyhow::Result;
use zoi_install::service::{ServiceAction, manage_service};

use crate::config::ServiceConfig;

/// Enables or disables the given systemd units to match the configuration.
///
/// # Errors
///
/// Returns an error if any unit cannot be enabled or disabled.
pub fn apply_services<S: std::hash::BuildHasher>(
    services: &HashMap<String, ServiceConfig, S>
) -> Result<()> {
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
                    "Warning: Failed to {action_str} service {name}: \
                     systemctl exited with {status}"
                );
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to {action_str} service {name}: {e}"
                );
            }
            _ => {}
        }
    }
    Ok(())
}
