use anyhow::Result;
use colored::*;

pub enum TelemetryCommand {
    Status,
    Enable,
    Disable,
}

pub fn run(cmd: TelemetryCommand) -> Result<()> {
    match cmd {
        TelemetryCommand::Status => {
            let cfg = crate::pkg::config::read_config()?;
            let status = if cfg.telemetry_enabled {
                "Enabled".green()
            } else {
                "Disabled".yellow()
            };
            println!("Telemetry: {}", status);
        }
        TelemetryCommand::Enable => {
            let mut cfg = crate::pkg::config::read_user_config()?;
            cfg.telemetry_enabled = true;
            crate::pkg::config::write_user_config(&cfg)?;
            println!("{} telemetry enabled", "Success:".green());
        }
        TelemetryCommand::Disable => {
            let mut cfg = crate::pkg::config::read_user_config()?;
            cfg.telemetry_enabled = false;
            crate::pkg::config::write_user_config(&cfg)?;
            println!("{} telemetry disabled", "Success:".green());
        }
    }
    Ok(())
}
