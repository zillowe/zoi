use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::*;
use zoi_core::utils::is_zoios;
use zoi_system::config::load_system_lua;

#[cfg(unix)]
use zoi_system::client::send_request;
#[cfg(unix)]
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
    /// Enter a ZoiOS sysroot (chroot) with automatic device mounting
    Chroot {
        /// Path to the ZoiOS root directory
        target: String,
        /// Command to run inside the chroot (defaults to /bin/bash)
        #[arg(short, long)]
        run: Option<String>,
        /// Show additional details
        #[arg(long, short)]
        verbose: bool,
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
                        "Are you sure you want to proceed with the build? This will install ZoiOS to the target device.",
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
            DistroSubcommands::Chroot {
                target,
                run,
                verbose,
            } => {
                let target_path = std::path::Path::new(&target);
                if !target_path.exists() {
                    return Err(anyhow!("Target path '{}' does not exist.", target));
                }

                let os_release = target_path.join("etc/os-release");
                if !os_release.exists() {
                    return Err(anyhow!(
                        "Target path '{}' is not a valid ZoiOS root (missing /etc/os-release).",
                        target
                    ));
                }

                if verbose {
                    println!(
                        "{} Entering sysroot at {}...",
                        "::".bold().blue(),
                        target.cyan()
                    );
                }

                let mut envs = std::collections::HashMap::new();
                envs.insert(
                    "PATH".to_string(),
                    "/usr/bin:/bin:/usr/sbin:/sbin".to_string(),
                );
                envs.insert("SHELL".to_string(), "/bin/bash".to_string());
                envs.insert(
                    "TERM".to_string(),
                    std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string()),
                );

                let shell_bin = std::path::PathBuf::from("/bin/bash");

                #[cfg(target_os = "linux")]
                let mut cmd = if let Some(run_cmd) = run {
                    let args = vec!["-c".to_string(), run_cmd];
                    crate::sandbox::wrap_command_in_root(
                        target_path,
                        &shell_bin,
                        &args,
                        &envs,
                        &[],
                    )?
                } else {
                    crate::sandbox::wrap_command_in_root(target_path, &shell_bin, &[], &envs, &[])?
                };

                #[cfg(not(target_os = "linux"))]
                return Err(anyhow!(
                    "Distro chroot is only supported on Linux via Bubblewrap."
                ));

                let status = cmd.status()?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
        },
        SystemSubcommands::Apply { file } => {
            #[cfg(unix)]
            {
                println!("Reading system configuration from {}...", file.cyan());
                let config = load_system_lua(&file)?;
                let response = send_request(Request::ApplySystemConfig(Box::new(config)))?;
                handle_response(response)?;
            }
            #[cfg(not(unix))]
            return Err(anyhow!(
                "OS management daemon commands are only supported on Unix."
            ));
        }
        SystemSubcommands::List => {
            #[cfg(unix)]
            {
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
            #[cfg(not(unix))]
            return Err(anyhow!(
                "OS management daemon commands are only supported on Unix."
            ));
        }
        SystemSubcommands::Status => {
            #[cfg(unix)]
            {
                let response = send_request(Request::GetStatus)?;
                handle_response(response)?;
            }
            #[cfg(not(unix))]
            return Err(anyhow!(
                "OS management daemon commands are only supported on Unix."
            ));
        }
        SystemSubcommands::Rollback { id } => {
            #[cfg(unix)]
            {
                println!("Rolling back to generation {}...", id.to_string().yellow());
                let response = send_request(Request::RollbackGeneration(id))?;
                handle_response(response)?;
            }
            #[cfg(not(unix))]
            let _ = id;
            #[cfg(not(unix))]
            return Err(anyhow!(
                "OS management daemon commands are only supported on Unix."
            ));
        }
    }

    Ok(())
}

fn print_build_summary(target: &str, config: &zoi_system::config::SystemConfig, dry_run: bool) {
    use comfy_table::modifiers::UTF8_ROUND_CORNERS;
    use comfy_table::presets::UTF8_FULL_CONDENSED;
    use comfy_table::{Cell, Color, Table};

    println!("\n{}", " ZoiOS Build Plan ".bold().on_blue().white());
    if dry_run {
        println!(
            "{}",
            " [DRY-RUN MODE - NO CHANGES WILL BE MADE] "
                .on_yellow()
                .black()
                .bold()
        );
    }
    println!("{} {}\n", "Target Root:".bold(), target.cyan());

    // Filesystems
    let mut fs_table = Table::new();
    fs_table
        .load_preset(UTF8_FULL_CONDENSED)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Action").fg(Color::Yellow),
            Cell::new("Device").fg(Color::Yellow),
            Cell::new("FS Type").fg(Color::Yellow),
            Cell::new("Mount Point").fg(Color::Yellow),
            Cell::new("Options").fg(Color::Yellow),
        ]);

    for fs in &config.filesystems {
        fs_table.add_row(vec![
            Cell::new("Configure (fstab)").fg(Color::Blue),
            Cell::new(&fs.device),
            Cell::new(&fs.fs_type),
            Cell::new(&fs.mount).fg(Color::Cyan),
            Cell::new(fs.options.as_deref().unwrap_or("defaults")),
        ]);
    }
    println!("{}", " 1. Filesystem & Partitioning ".bold().underline());
    println!("{}\n", fs_table);

    // System Info
    let mut sys_table = Table::new();
    sys_table
        .load_preset(UTF8_FULL_CONDENSED)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Property").fg(Color::Yellow),
            Cell::new("Value").fg(Color::Yellow),
        ]);

    sys_table.add_row(vec![
        Cell::new("Hostname"),
        Cell::new(config.system.hostname.as_deref().unwrap_or("zoios")).fg(Color::Cyan),
    ]);
    sys_table.add_row(vec![
        Cell::new("Timezone"),
        Cell::new(config.system.timezone.as_deref().unwrap_or("UTC")),
    ]);
    sys_table.add_row(vec![
        Cell::new("Locale"),
        Cell::new(config.system.locale.as_deref().unwrap_or("en_US.UTF-8")),
    ]);

    println!("{}", " 2. System Configuration ".bold().underline());
    println!("{}\n", sys_table);

    // Packages
    println!("{}", " 3. Packages ".bold().underline());
    println!(
        "{} base packages will be installed from the registry.\n",
        config.packages.len().to_string().green().bold()
    );

    let mut pkg_list = String::new();
    for (i, pkg) in config.packages.iter().enumerate() {
        pkg_list.push_str(&format!("{}", pkg.cyan()));
        if i < config.packages.len() - 1 {
            pkg_list.push_str(", ");
        }
        if (i + 1) % 5 == 0 {
            pkg_list.push('\n');
        }
    }
    println!("{}\n", pkg_list);
}

#[cfg(unix)]
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
