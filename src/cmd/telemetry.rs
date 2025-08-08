use colored::*;

pub enum TelemetryCommand {
    Status,
    Enable,
    Disable,
}

pub fn run(cmd: TelemetryCommand) {
    match cmd {
        TelemetryCommand::Status => match crate::pkg::config::read_config() {
            Ok(cfg) => {
                let status = if cfg.telemetry_enabled {
                    "Enabled".green()
                } else {
                    "Disabled".yellow()
                };
                println!("Telemetry: {}", status);
            }
            Err(e) => eprintln!("{} failed to read config: {}", "Error".red(), e),
        },
        TelemetryCommand::Enable => match crate::pkg::config::read_config() {
            Ok(mut cfg) => {
                cfg.telemetry_enabled = true;
                if let Err(e) = crate::pkg::config::write_config(&cfg) {
                    eprintln!("{} failed to enable telemetry: {}", "Error".red(), e);
                } else {
                    println!("{} telemetry enabled", "Success:".green());
                }
            }
            Err(e) => eprintln!("{} failed to read config: {}", "Error".red(), e),
        },
        TelemetryCommand::Disable => match crate::pkg::config::read_config() {
            Ok(mut cfg) => {
                cfg.telemetry_enabled = false;
                if let Err(e) = crate::pkg::config::write_config(&cfg) {
                    eprintln!("{} failed to disable telemetry: {}", "Error".red(), e);
                } else {
                    println!("{} telemetry disabled", "Success:".green());
                }
            }
            Err(e) => eprintln!("{} failed to read config: {}", "Error".red(), e),
        },
    }
}
