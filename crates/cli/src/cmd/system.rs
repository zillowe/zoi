use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::*;
use zoi_core::utils::is_zoios;
use zoi_system::client::send_request;
use zoi_system::config::load_system_lua;
use zoi_system::protocol::{Request, Response};

#[derive(Parser, Debug)]
pub struct SystemCommand {
    #[command(subcommand)]
    pub command: SystemSubcommands,
}

#[derive(Subcommand, Debug)]
pub enum SystemSubcommands {
    /// Apply a declarative system configuration from system.lua
    Apply {
        /// Path to the system configuration file
        #[arg(default_value = "/etc/zoi/system.lua")]
        file: String,
    },
    /// List all system generations
    List,
    /// Show current system status and active generation
    Status,
    /// Rollback to a previous system generation
    Rollback {
        /// Generation ID to roll back to
        id: u32,
    },
    /// Manage secrets (hashes and encrypted strings)
    Secret {
        #[command(subcommand)]
        command: SecretSubcommands,
    },
    /// Commands for building and managing ZoiOS distributions
    Distro {
        #[command(subcommand)]
        command: DistroSubcommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum DistroSubcommands {
    /// Build a new ZoiOS distribution image or install to a disk
    Build {
        /// The target device or image path (e.g. /dev/sdb)
        #[arg(short, long)]
        target: String,
        /// Path to the system configuration to use for the build
        #[arg(short, long)]
        config: String,
        /// Show the build plan without executing destructive commands
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SecretSubcommands {
    /// Generate a one-way hash of a password for use in system.lua
    Hash {
        /// The password to hash
        password: String,
    },
    /// Encrypt a sensitive string (like an API key) so only Zoi can decrypt it
    Encrypt {
        /// The plaintext string to encrypt
        value: String,
    },
    /// Decrypt a ZOISEC string (only works on the same machine where it was encrypted)
    Decrypt {
        /// The encrypted ZOISEC string
        secret: String,
    },
}

pub fn run(args: SystemCommand, yes: bool) -> Result<()> {
    // secret commands and distro build are allowed on generic systems
    let is_secret = matches!(args.command, SystemSubcommands::Secret { .. });
    let is_distro = matches!(args.command, SystemSubcommands::Distro { .. });

    if !is_secret && !is_distro && !is_zoios() {
        return Err(anyhow!(
            "OS management features are only available on ZoiOS systems."
        ));
    }

    match args.command {
        SystemSubcommands::Secret { command } => match command {
            SecretSubcommands::Hash { password } => {
                let hash = zoi_system::secret::hash_password(&password)?;
                println!("Password hash generated successfully. Use this in your system.lua:");
                println!("\n  {}", hash.green());
            }
            SecretSubcommands::Encrypt { value } => {
                let encrypted = zoi_system::secret::encrypt_secret(&value)?;
                println!("Value encrypted successfully. Use this in your system.lua or home.lua:");
                println!("\n  {}", encrypted.yellow());
                println!(
                    "\n{}",
                    "Note: This can only be decrypted by Zoi on this specific machine.".dimmed()
                );
            }
            SecretSubcommands::Decrypt { secret } => {
                let decrypted = zoi_system::secret::decrypt_secret(&secret)?;
                if decrypted == secret {
                    return Err(anyhow!("Input is not a valid Zoi secret string."));
                }
                println!("Secret decrypted successfully:");
                println!("\n  {}", decrypted.green());
            }
        },
        SystemSubcommands::Distro { command } => match command {
            DistroSubcommands::Build {
                target,
                config,
                dry_run,
            } => {
                let target_path = std::path::Path::new(&target);
                let config = load_system_lua(&config)?;

                // Pre-flight: Validate packages exist in registry
                println!(
                    "{} Validating {} packages...",
                    "::".bold().blue(),
                    config.packages.len().to_string().cyan()
                );
                for pkg_id in &config.packages {
                    if let Err(e) = zoi_resolver::resolve::resolve_source(pkg_id, None, true, true)
                    {
                        return Err(anyhow!("Package validation failed for '{}': {}", pkg_id, e));
                    }
                }

                print_build_summary(&target, &config, dry_run);

                if !dry_run
                    && !zoi_core::utils::ask_for_confirmation(
                        "Are you sure you want to proceed with the build? This will format the target device!",
                        yes,
                    )
                {
                    return Err(anyhow!("Build aborted by user."));
                }

                println!(
                    "{} Orchestrating ZoiOS build on {}...",
                    "::".bold().blue(),
                    target.cyan()
                );

                //Format & Subvolumes
                zoi_system::distro::prepare_target_filesystems(&config, dry_run)?;

                // Marker
                zoi_system::distro::initialize_zoios_marker(
                    target_path,
                    config.system.hostname.as_deref(),
                    dry_run,
                )?;

                // Install packages into target sysroot
                if dry_run {
                    println!(
                        "  {} Would install base packages: {}",
                        "[DRY-RUN]".dimmed(),
                        config.packages.join(", ")
                    );
                } else {
                    println!(
                        "{} Installing base packages to {}...",
                        "::".bold().blue(),
                        target.cyan()
                    );

                    // Use CLI's install engine
                    crate::cmd::install::run(
                        &config.packages,
                        None,
                        false,
                        false,
                        true,
                        Some(crate::cli::InstallScope::System),
                        false,
                        false,
                        false,
                        None,
                        false,
                        None,
                        false,
                        false,
                        false,
                        false,
                        3,
                        false,
                        false,
                    )?;
                }

                // Finalize Generation
                zoi_system::distro::finalize_first_generation(
                    target_path,
                    config.packages.clone(),
                    dry_run,
                )?;

                let success_msg = if dry_run {
                    "Dry-run complete."
                } else {
                    "ZoiOS build complete."
                };
                println!(
                    "{} {} on {}.",
                    "Success:".green(),
                    success_msg,
                    target.cyan()
                );
            }
        },
        SystemSubcommands::Apply { file } => {
            println!("Reading system configuration from {}...", file.cyan());
            let config = load_system_lua(&file)?;
            let response = send_request(Request::ApplySystemConfig(Box::new(config)))?;
            handle_response(response)?;
        }
        SystemSubcommands::List => {
            let response = send_request(Request::ListGenerations)?;
            match response {
                Response::Generations(gens) => {
                    println!("{:<5} {:<25} {:<50}", "ID", "Created At", "Packages");
                    println!("{:-<80}", "");
                    for generation in gens {
                        println!(
                            "{:<5} {:<25} {:<50}",
                            generation.id,
                            generation.created_at.to_rfc3339(),
                            generation.packages.join(", ")
                        );
                    }
                }
                _ => handle_response(response)?,
            }
        }
        SystemSubcommands::Status => {
            let response = send_request(Request::GetStatus)?;
            handle_response(response)?;
        }
        SystemSubcommands::Rollback { id } => {
            println!("Rolling back to generation {}...", id.to_string().yellow());
            let response = send_request(Request::RollbackGeneration(id))?;
            handle_response(response)?;
        }
    }

    Ok(())
}

fn print_build_summary(target: &str, config: &zoi_system::config::SystemConfig, dry_run: bool) {
    use comfy_table::Table;
    use comfy_table::presets::UTF8_FULL;

    println!("\n{}", "ZoiOS Build Plan Summary".bold().underline());
    if dry_run {
        println!(
            "{}",
            "[DRY-RUN MODE - NO CHANGES WILL BE MADE]".yellow().bold()
        );
    }
    println!("Target Device/Root: {}\n", target.cyan());

    // Filesystems
    let mut fs_table = Table::new();
    fs_table.load_preset(UTF8_FULL);
    fs_table.set_header(vec![
        "Action",
        "Device",
        "FS Type",
        "Mount Point",
        "Options",
    ]);
    for fs in &config.filesystems {
        fs_table.add_row(vec![
            "Format".red().to_string(),
            fs.device.clone(),
            fs.fs_type.clone(),
            fs.mount.clone(),
            fs.options.as_deref().unwrap_or("defaults").to_string(),
        ]);
    }
    println!("{}", "Filesystem & Partitioning:".bold());
    println!("{}\n", fs_table);

    // System Info
    let mut sys_table = Table::new();
    sys_table.load_preset(UTF8_FULL);
    sys_table.set_header(vec!["Property", "Value"]);
    sys_table.add_row(vec![
        "Hostname",
        config.system.hostname.as_deref().unwrap_or("zoios"),
    ]);
    sys_table.add_row(vec![
        "Timezone",
        config.system.timezone.as_deref().unwrap_or("UTC"),
    ]);
    sys_table.add_row(vec![
        "Locale",
        config.system.locale.as_deref().unwrap_or("en_US.UTF-8"),
    ]);
    println!("{}", "System Configuration:".bold());
    println!("{}\n", sys_table);

    // Packages
    println!(
        "{} {} base packages will be installed.",
        "Packages:".bold(),
        config.packages.len().to_string().cyan()
    );
    println!("  {}\n", config.packages.join(", "));
}

fn handle_response(response: Response) -> Result<()> {
    match response {
        Response::Ok => println!("{}", "Operation successful.".green()),
        Response::Success(msg) => println!("{} {}", "Success:".green(), msg),
        Response::Status(msg) => println!("Daemon status: {}", msg.cyan()),
        Response::Error(err) => return Err(anyhow!("Daemon error: {}", err)),
        _ => return Err(anyhow!("Unexpected response from daemon")),
    }
    Ok(())
}
