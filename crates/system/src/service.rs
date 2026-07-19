use crate::config::ServiceConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;

pub fn apply_services(services: &HashMap<String, ServiceConfig>) -> Result<()> {
    for (name, cfg) in services {
        let action = if cfg.enable { "enable" } else { "disable" };
        println!(
            "{} service {}...",
            if cfg.enable { "Enabling" } else { "Disabling" },
            name
        );

        let mut cmd = Command::new("systemctl");
        cmd.arg(action).arg("--now").arg(name);

        if let Err(e) = cmd.status() {
            eprintln!("Warning: Failed to {} service {}: {}", action, name, e);
        }
    }
    Ok(())
}
