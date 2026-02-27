use crate::pkg::service::{self, ServiceAction};
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use comfy_table::{Table, presets::UTF8_FULL};

#[derive(Parser, Debug)]
#[command(long_about = "Manages background services for installed packages.")]
pub struct ServiceCommand {
    #[command(subcommand)]
    pub command: ServiceCommands,
}

#[derive(Subcommand, Debug)]
pub enum ServiceCommands {
    /// Start a service
    Start {
        /// The name of the package whose service to start
        package: String,
    },
    /// Stop a service
    Stop {
        /// The name of the package whose service to stop
        package: String,
    },
    /// Restart a service
    Restart {
        /// The name of the package whose service to restart
        package: String,
    },
    /// Show the status of a service
    Status {
        /// The name of the package whose service status to show
        package: String,
    },
    /// List all packages that define a service and their current status
    #[command(alias = "ls")]
    List,
}

pub fn run(args: ServiceCommand) -> Result<()> {
    match args.command {
        ServiceCommands::Start { package } => {
            println!("Starting service for package '{}'...", package.cyan());
            service::manage_service(&package, ServiceAction::Start)?;
            println!("{}", "Service started successfully.".green());
        }
        ServiceCommands::Stop { package } => {
            println!("Stopping service for package '{}'...", package.cyan());
            service::manage_service(&package, ServiceAction::Stop)?;
            println!("{}", "Service stopped successfully.".green());
        }
        ServiceCommands::Restart { package } => {
            println!("Restarting service for package '{}'...", package.cyan());
            service::manage_service(&package, ServiceAction::Restart)?;
            println!("{}", "Service restarted successfully.".green());
        }
        ServiceCommands::Status { package } => {
            service::manage_service(&package, ServiceAction::Status)?;
        }
        ServiceCommands::List => {
            let services = service::list_services()?;
            if services.is_empty() {
                println!("No installed packages define background services.");
                return Ok(());
            }

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_header(vec!["Package", "Status"]);

            for (pkg, status) in services {
                let status_cell = if status == "active" || status == "running" {
                    status.green()
                } else {
                    status.yellow()
                };
                table.add_row(vec![pkg.cyan(), status_cell.to_string().into()]);
            }

            println!("{}", table);
        }
    }
    Ok(())
}
