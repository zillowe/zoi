use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Command;

use anyhow::{Result, bail};
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

/// Enables (`true`) or disables (`false`) units in the system under `root`.
///
/// On the live system (`/`) this is [`apply_services`], which also starts
/// or stops them. In another root the units are only enabled or disabled
/// (they start when that system boots): with the host's `systemctl --root`
/// when it has one, else with the target's `systemctl` run inside it.
/// Returns the units that could not be changed.
///
/// # Errors
///
/// Returns an error if a unit name is malformed.
pub fn apply_services_in_root(
    root: &Path,
    services: &BTreeMap<String, bool>,
    dry_run: bool
) -> Result<Vec<String>> {
    for name in services.keys() {
        let valid = !name.is_empty()
            && !name.starts_with('-')
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ":_.@\\-".contains(c));
        if !valid {
            bail!("Invalid service name '{name}' in system.lua");
        }
    }
    if dry_run {
        for (name, enable) in services {
            println!(
                "  [DRY-RUN] Would {} {name}",
                if *enable { "enable" } else { "disable" }
            );
        }
        return Ok(Vec::new());
    }
    if root == Path::new("/") {
        let live: HashMap<String, ServiceConfig> = services
            .iter()
            .map(|(name, enable)| {
                (name.clone(), ServiceConfig { enable: *enable })
            })
            .collect();
        apply_services(&live)?;
        return Ok(Vec::new());
    }

    let host_systemctl = Command::new("systemctl")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success());
    let mut failed = Vec::new();
    for (name, enable) in services {
        let action = if *enable { "enable" } else { "disable" };
        println!(
            "{} service {name}...",
            if *enable { "Enabling" } else { "Disabling" }
        );
        let ok = if host_systemctl {
            Command::new("systemctl")
                .arg(format!("--root={}", root.display()))
                .args([action, name.as_str()])
                .status()
                .is_ok_and(|s| s.success())
        } else {
            let mut cmd = Command::new("systemctl");
            cmd.args([action, name.as_str()]);
            crate::kernel::run_in_root(root, &mut cmd).unwrap_or(false)
        };
        if !ok {
            failed.push(name.clone());
        }
    }
    Ok(failed)
}
