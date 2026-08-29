//! Logic for the `system` command.
//!
//! This module provides commands for managing ZoiOS systems, including
//! declarative configuration, system generations, secrets management, and
//! distribution building.

use std::fmt::Write as _;
use std::io::Read;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::Colorize;
use zoi_core::utils::is_zoios;
#[cfg(unix)]
use zoi_system::client::send_request;
use zoi_system::config::load_system_lua;
#[cfg(unix)]
use zoi_system::protocol::{Request, Response};

/// The root system management command.
#[derive(Parser, Debug)]
pub struct SystemCommand {
    /// The specific system subcommand to execute.
    #[command(subcommand)]
    pub command: SystemSubcommands
}

/// Available system subcommands.
#[derive(Subcommand, Debug)]
pub enum SystemSubcommands {
    /// Apply a declarative system configuration from system.lua
    Apply {
        /// Path to the system configuration file
        #[arg(default_value = "/etc/zoi/system.lua")]
        file: String
    },
    /// List all system generations
    List,
    /// Show current system status and active generation
    Status,
    /// Rollback to a previous system generation
    Rollback {
        /// Generation ID to roll back to
        id: u32
    },
    /// Pin a system generation to prevent it from being pruned
    Pin {
        /// Generation ID to pin
        id: u32
    },
    /// Unpin a system generation
    Unpin {
        /// Generation ID to unpin
        id: u32
    },
    /// Manage secrets (hashes and encrypted strings)
    Secret {
        /// Secret subcommands.
        #[command(subcommand)]
        command: SecretSubcommands
    },
    /// Commands for building and managing `ZoiOS` distributions
    Distro {
        /// Distro subcommands.
        #[command(subcommand)]
        command: DistroSubcommands
    }
}

/// Commands for building and managing `ZoiOS` distributions.
#[derive(Subcommand, Debug)]
pub enum DistroSubcommands {
    /// Build a new `ZoiOS` distribution image or install to a disk
    Build {
        /// The target device or image path (e.g. /dev/sdb)
        #[arg(short, long)]
        target: String,
        /// Path to the system configuration to use for the build
        #[arg(short, long)]
        config: String,
        /// Show the build plan without executing destructive commands
        #[arg(long)]
        dry_run: bool
    },
    /// Enter a `ZoiOS` sysroot (chroot) with automatic device mounting
    Chroot {
        /// Path to the `ZoiOS` root directory
        target: String,
        /// Command to run inside the chroot (defaults to /bin/bash)
        #[arg(short, long)]
        run: Option<String>,
        /// Show additional details
        #[arg(long, short)]
        verbose: bool
    }
}

/// Commands for managing secrets like password hashes and encrypted strings.
#[derive(Subcommand, Debug)]
pub enum SecretSubcommands {
    /// Generate a one-way hash of a password for use in system.lua
    Hash {
        /// The password to hash
        password: String
    },
    /// Encrypt a sensitive string (like an API key) so only Zoi can decrypt it
    Encrypt {
        /// The plaintext string to encrypt
        value: String
    },
    /// Decrypt a ZOISEC string (only works on the same machine where it was
    /// encrypted)
    Decrypt {
        /// The encrypted ZOISEC string
        secret: String
    },
    /// Export the `ZoiSEC` master key as a base64 string
    ExportKey,
    /// Import a `ZoiSEC` master key from a base64 string
    ImportKey {
        /// The base64-encoded master key
        key: String
    }
}

/// Run the system management command.
///
/// # Errors
///
/// Returns an error if:
/// - The command is not run on `ZoiOS` (except for secrets and distro
///   commands).
/// - Package validation fails during a distro build.
/// - The user aborts a build.
/// - Any OS management daemon request fails.
/// # Errors
///
/// Returns an error if the system operation fails.
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
                println!(
                    "Password hash generated successfully. Use this in your \
                     system.lua:"
                );
                println!("\n  {}", hash.green());
            }
            SecretSubcommands::Encrypt { value } => {
                let encrypted = zoi_system::secret::encrypt_secret(&value)?;
                println!(
                    "Value encrypted successfully. Use this in your \
                     system.lua or home.lua:"
                );
                println!("\n  {}", encrypted.yellow());
                println!(
                    "\n{}",
                    "Note: This can only be decrypted by Zoi on this specific \
                     machine."
                        .dimmed()
                );
            }
            SecretSubcommands::Decrypt { secret } => {
                let decrypted = zoi_system::secret::decrypt_secret(&secret)?;
                if decrypted == secret {
                    return Err(anyhow!(
                        "Input is not a valid Zoi secret string."
                    ));
                }
                println!("Secret decrypted successfully:");
                println!("\n  {}", decrypted.green());
            }
            SecretSubcommands::ExportKey => {
                let key = zoi_system::secret::export_master_key()?;
                println!("ZoiSEC Master Key (base64):");
                println!("\n  {}", key.yellow());
                println!(
                    "\n{}",
                    "Keep this key safe! Anyone with this key can decrypt \
                     your ZoiSEC secrets."
                        .red()
                        .bold()
                );
            }
            SecretSubcommands::ImportKey { key } => {
                zoi_system::secret::import_master_key(&key)?;
                println!(
                    "{} ZoiSEC master key imported successfully.",
                    "Success:".green()
                );
            }
        },
        SystemSubcommands::Distro { command } => match command {
            DistroSubcommands::Build {
                target,
                config,
                dry_run
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
                    if let Err(e) = zoi_resolver::resolve::resolve_source(
                        pkg_id, None, true, true
                    ) {
                        return Err(anyhow!(
                            "Package validation failed for '{pkg_id}': {e}"
                        ));
                    }
                }

                print_build_summary(&target, &config, dry_run);

                if !dry_run
                    && !zoi_core::utils::ask_for_confirmation(
                        "Are you sure you want to proceed with the build? \
                         This will install ZoiOS to the target device.",
                        yes
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
                    dry_run
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
                    let project_config = zoi_project::config::ProjectConfig {
                        name: "system".to_string(),
                        registries: std::collections::HashMap::new(),
                        packages: Vec::new(),
                        pkgs: config.packages.clone(),
                        pkgs_v2: config.packages_v2.clone(),
                        config:
                            zoi_project::config::ProjectLocalConfig::default(),
                        commands: Vec::new(),
                        environments: Vec::new(),
                        shell: Some(zoi_project::config::ShellSpec::default())
                    };

                    crate::cmd::install::run(
                        &config.packages,
                        None,
                        false, // force
                        false, // all_optional
                        yes,
                        Some(crate::cli::InstallScope::System),
                        false,
                        false,
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
                        Some(project_config)
                    )?;
                }

                // Finalize Generation
                zoi_system::distro::finalize_first_generation(
                    target_path,
                    config.packages.clone(),
                    dry_run
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
                verbose
            } => {
                let target_path = std::path::Path::new(&target);
                if !target_path.exists() {
                    return Err(anyhow!(
                        "Target path '{target}' does not exist."
                    ));
                }

                let os_release = target_path.join("etc/os-release");
                if !os_release.exists() {
                    return Err(anyhow!(
                        "Target path '{target}' is not a valid ZoiOS root \
                         (missing /etc/os-release)."
                    ));
                }

                // --- BOOTSTRAP AUDIT ---

                // Filesystem check
                let fs_type = std::process::Command::new("stat")
                    .arg("-f")
                    .arg("-c")
                    .arg("%T")
                    .arg(&target)
                    .output();
                if let Ok(out) = fs_type {
                    let t =
                        String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if t == "msdos" || t == "vfat" {
                        eprintln!(
                            "\n{} CRITICAL: Target filesystem is '{}'. ZoiOS \
                             requires a Linux filesystem (ext4, btrfs, xfs) \
                             to support hard links and permissions. Your \
                             bootstrap will NOT work on FAT32.",
                            "Error:".red().bold(),
                            t.yellow()
                        );
                    } else if verbose {
                        println!(
                            "{} Filesystem type: {}",
                            "::".bold().blue(),
                            t.green()
                        );
                    }
                }

                // Merged-Usr Symlink Audit
                let mut broken_layout = false;
                for sym in &["bin", "sbin", "lib", "lib64"] {
                    let p = target_path.join(sym);
                    let meta = std::fs::symlink_metadata(&p);
                    if let Ok(m) = meta {
                        if !m.file_type().is_symlink() {
                            eprintln!(
                                "{} WARNING: '/{}' is a real directory, but \
                                 ZoiOS expects a merged-usr symlink to \
                                 'usr/{}'.",
                                "::".bold().yellow(),
                                sym,
                                sym
                            );
                            broken_layout = true;
                        } else if let Ok(target) = std::fs::read_link(&p) {
                            if target.is_absolute() {
                                eprintln!(
                                    "{} WARNING: '/{}' is an absolute symlink \
                                     to '{}'. This WILL break inside the \
                                     chroot. It should be relative (e.g. \
                                     'usr/{}').",
                                    "::".bold().yellow(),
                                    sym,
                                    target.display(),
                                    sym
                                );
                                broken_layout = true;
                            } else {
                                let abs_target = target_path.join(target);
                                if !abs_target.exists() {
                                    eprintln!(
                                        "{} WARNING: Symlink '/{}' points to \
                                         non-existent path '{}'.",
                                        "::".bold().yellow(),
                                        sym,
                                        abs_target.display()
                                    );
                                    broken_layout = true;
                                }
                            }
                        }
                    } else if *sym != "sbin" {
                        eprintln!(
                            "{} WARNING: '/{}' is missing! Your binaries will \
                             likely fail to find their loader or shell.",
                            "::".bold().yellow(),
                            sym
                        );
                        broken_layout = true;
                    }
                }

                // Dynamic Loader Validation (The common cause of 139)
                let mut loader_found = false;
                let loaders = [
                    "usr/lib/ld-linux-x86-64.so.2",
                    "lib64/ld-linux-x86-64.so.2",
                    "lib/ld-linux-x86-64.so.2"
                ];
                for l in &loaders {
                    let lp = target_path.join(l);
                    if lp.exists() {
                        loader_found = true;
                        if let Ok(mut file) = std::fs::File::open(&lp) {
                            let mut magic = [0u8; 4];
                            if file.read_exact(&mut magic).is_ok() {
                                if magic != [0x7f, b'E', b'L', b'F'] {
                                    eprintln!(
                                        "{} CRITICAL: Dynamic loader '{}' is \
                                         NOT an ELF file! Your glibc \
                                         installation is corrupted.",
                                        "Error:".red().bold(),
                                        l
                                    );
                                }
                            } else {
                                eprintln!(
                                    "{} CRITICAL: Dynamic loader '{}' is 0 \
                                     bytes or unreadable.",
                                    "Error:".red().bold(),
                                    l
                                );
                            }
                        }
                        break;
                    }
                }
                if !loader_found {
                    eprintln!(
                        "{} Dynamic loader not found. Binaries WILL Segfault \
                         (139).",
                        "::".bold().yellow()
                    );
                }

                if broken_layout || !loader_found {
                    println!(
                        "{} Hint: Your ZoiOS bootstrap appears incomplete or \
                         corrupted. Please verify your base system packages.",
                        "::".bold().blue()
                    );
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
                    "/usr/bin:/bin:/usr/sbin:/sbin".to_string()
                );
                envs.insert("SHELL".to_string(), "/usr/bin/bash".to_string());
                envs.insert(
                    "TERM".to_string(),
                    std::env::var("TERM")
                        .unwrap_or_else(|_| "xterm-256color".to_string())
                );

                // Resolve shell path in guest (Prefer /usr/bin/bash)
                let mut shell_bin = std::path::PathBuf::from("/usr/bin/bash");
                if !target_path.join("usr/bin/bash").exists()
                    && target_path.join("bin/bash").exists()
                {
                    shell_bin = std::path::PathBuf::from("/bin/bash");
                }

                if verbose {
                    let cmd_display = if let Some(r) = &run {
                        format!("{} -c '{}'", shell_bin.display(), r)
                    } else {
                        shell_bin.display().to_string()
                    };
                    println!(
                        "{} Running inside chroot: {}",
                        "::".bold().blue(),
                        cmd_display.green()
                    );
                }

                #[cfg(target_os = "linux")]
                {
                    let mut cmd = if let Some(run_cmd) = run {
                        let args = vec!["-c".to_string(), run_cmd];
                        crate::sandbox::wrap_command_in_root(
                            target_path,
                            &shell_bin,
                            &args,
                            &envs,
                            &[],
                            false
                        )?
                    } else {
                        crate::sandbox::wrap_command_in_root(
                            target_path,
                            &shell_bin,
                            &[],
                            &envs,
                            &[],
                            false
                        )?
                    };

                    if verbose {
                        println!(
                            "{} Full command: {:?}",
                            "::".bold().blue(),
                            cmd
                        );
                    }

                    let status = cmd.status()?;
                    if !status.success() {
                        let code = status.code().unwrap_or(1);
                        eprintln!(
                            "\n{} Chroot execution failed with exit code: {}",
                            "Error:".red().bold(),
                            code.to_string().yellow()
                        );
                        if code == 139 {
                            println!(
                                "{} Hint: Segfaults (139) often indicate an \
                                 instruction set mismatch (e.g. x86-64-v3 \
                                 binaries on older CPUs).",
                                "::".bold().blue()
                            );
                        }
                        std::process::exit(code);
                    }
                }

                #[cfg(not(target_os = "linux"))]
                return Err(anyhow!(
                    "Distro chroot is only supported on Linux via Bubblewrap."
                ));
            }
        },
        SystemSubcommands::Apply { file } => {
            #[cfg(unix)]
            {
                println!(
                    "Reading system configuration from {}...",
                    file.cyan()
                );
                let config = load_system_lua(&file)?;
                let response =
                    send_request(Request::ApplySystemConfig(Box::new(config)))?;
                handle_response(response)?;
            }
            #[cfg(not(unix))]
            {
                let _ = file;
                return Err(anyhow!(
                    "OS management daemon commands are only supported on Unix."
                ));
            }
        }
        SystemSubcommands::List => {
            #[cfg(unix)]
            {
                let response = send_request(Request::ListGenerations)?;
                match response {
                    Response::Generations(gens) => {
                        println!(
                            "{:<5} {:<25} {:<50}",
                            "ID", "Created At", "Packages"
                        );
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
                    _ => handle_response(response)?
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
                println!(
                    "Rolling back to generation {}...",
                    id.to_string().yellow()
                );
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
        SystemSubcommands::Pin { id } => {
            #[cfg(unix)]
            {
                let response = send_request(Request::PinGeneration(id, true))?;
                handle_response(response)?;
            }
            #[cfg(not(unix))]
            let _ = id;
            #[cfg(not(unix))]
            return Err(anyhow!(
                "OS management daemon commands are only supported on Unix."
            ));
        }
        SystemSubcommands::Unpin { id } => {
            #[cfg(unix)]
            {
                let response = send_request(Request::PinGeneration(id, false))?;
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

/// Prints a summary of the system build plan.
fn print_build_summary(
    target: &str,
    config: &zoi_system::config::SystemConfig,
    dry_run: bool
) {
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
        .load_style(UTF8_FULL_CONDENSED.with_rounded_corners())
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
    println!("{fs_table}\n");

    // System Info
    let mut sys_table = Table::new();
    sys_table
        .load_style(UTF8_FULL_CONDENSED.with_rounded_corners())
        .set_header(vec![
            Cell::new("Property").fg(Color::Yellow),
            Cell::new("Value").fg(Color::Yellow),
        ]);

    sys_table.add_row(vec![
        Cell::new("Hostname"),
        Cell::new(config.system.hostname.as_deref().unwrap_or("zoios"))
            .fg(Color::Cyan),
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
    println!("{sys_table}\n");

    // Packages
    println!("{}", " 3. Packages ".bold().underline());
    println!(
        "{} base packages will be installed from the registry.\n",
        config.packages.len().to_string().green().bold()
    );

    let mut pkg_list = String::new();
    for (i, pkg) in config.packages.iter().enumerate() {
        let _ = write!(pkg_list, "{}", pkg.cyan());
        if i < config.packages.len() - 1 {
            pkg_list.push_str(", ");
        }
        if (i + 1) % 5 == 0 {
            pkg_list.push('\n');
        }
    }
    println!("{pkg_list}\n");
}

/// Handles the response from the system daemon.
#[cfg(unix)]
fn handle_response(response: Response) -> Result<()> {
    match response {
        Response::Ok => println!("{}", "Operation successful.".green()),
        Response::Success(msg) => println!("{} {}", "Success:".green(), msg),
        Response::Status(msg) => println!("Daemon status: {}", msg.cyan()),
        Response::Error(err) => return Err(anyhow!("Daemon error: {err}")),
        Response::Generations(_) => {
            return Err(anyhow!("Unexpected response from daemon"));
        }
    }
    Ok(())
}
