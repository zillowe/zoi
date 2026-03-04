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
            println!("{} telemetry is currently {}.", "::".bold().blue(), status);

            if cfg.telemetry_enabled {
                let id = crate::pkg::telemetry::get_anonymous_id();
                println!("Anonymous Client ID: {}", id.cyan());
                println!(
                    "\nThank you for helping us improve Zoi! We collect minimal, anonymous data about:"
                );
                println!("- {} (OS, Arch, Distro, Shell)", "Environment".bold());
                println!("- {} (Action, Scope, Reason)", "Operations".bold());
                println!(
                    "- {} (Name, Version, Repo, License)",
                    "Package Metadata".bold()
                );
            } else {
                println!(
                    "\nTelemetry is anonymous and helps us prioritize features and platforms."
                );
                println!("Run 'zoi telemetry enable' to help the project.");
            }
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
