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

            println!(
                "{}",
                "Notice: Enabling telemetry shares anonymous usage data to help improve Zoi."
                    .dimmed()
            );
            println!(
                "{}",
                "No personal data or IP addresses are ever collected.".dimmed()
            );
            println!(
                "{} {}",
                "Full Privacy Policy:".dimmed(),
                "https://zillowe.qzz.io/legal/privacy".cyan()
            );

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
