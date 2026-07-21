use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use zoi_system::client::send_request;
use zoi_system::generation::GenerationManager;
use zoi_system::protocol::{Request, Response};

const SOCKET_PATH: &str = "/run/zoid.sock";
const PID_PATH: &str = "/run/zoid.pid";

/// zoid - The ZoiOS privileged system daemon.
#[derive(Parser)]
#[command(name = "zoid", author, about, version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the zoid daemon
    Start {
        /// Do not background the process
        #[arg(short, long)]
        foreground: bool,
    },
    /// Stop the zoid daemon
    Stop,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { foreground } => {
            if foreground {
                start_daemon()?;
            } else {
                daemonize()?;
            }
        }
        Commands::Stop => {
            println!("Stopping zoid daemon...");
            match send_request(Request::Shutdown) {
                Ok(Response::Ok) => {
                    println!("Daemon stopped successfully.");
                    if Path::new(PID_PATH).exists() {
                        let _ = fs::remove_file(PID_PATH);
                    }
                }
                Ok(Response::Error(e)) => eprintln!("Error stopping daemon: {}", e),
                Err(e) => {
                    eprintln!(
                        "Failed to connect to daemon: {}. Checking for PID file...",
                        e
                    );
                    if let Ok(pid_str) = fs::read_to_string(PID_PATH)
                        && let Ok(pid) = pid_str.trim().parse::<i32>()
                    {
                        use nix::sys::signal::{self, Signal};
                        use nix::unistd::Pid;
                        if let Err(e) = signal::kill(Pid::from_raw(pid), Signal::SIGTERM) {
                            eprintln!("Failed to kill process {}: {}", pid, e);
                        } else {
                            println!("Killed process {}.", pid);
                            let _ = fs::remove_file(PID_PATH);
                        }
                    }
                }
                _ => eprintln!("Unexpected response from daemon"),
            }
        }
    }

    Ok(())
}

fn daemonize() -> Result<()> {
    use nix::unistd::{ForkResult, fork};

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            println!("zoid started in background (PID: {})", child);
            std::process::exit(0);
        }
        Ok(ForkResult::Child) => {
            // Close standard streams
            let dev_null = fs::File::open("/dev/null")?;
            let fd = dev_null.as_raw_fd();
            unsafe {
                nix::libc::dup2(fd, 0);
                nix::libc::dup2(fd, 1);
                nix::libc::dup2(fd, 2);
            }

            start_daemon()?;
        }
        Err(e) => return Err(anyhow::anyhow!("Fork failed: {}", e)),
    }
    Ok(())
}

fn start_daemon() -> Result<()> {
    // PID management
    if Path::new(PID_PATH).exists()
        && let Ok(old_pid) = fs::read_to_string(PID_PATH)
        && Path::new(&format!("/proc/{}", old_pid.trim())).exists()
    {
        return Err(anyhow::anyhow!(
            "zoid is already running (PID: {})",
            old_pid.trim()
        ));
    }
    fs::write(PID_PATH, std::process::id().to_string())?;

    if Path::new(SOCKET_PATH).exists() {
        fs::remove_file(SOCKET_PATH)?;
    }

    let listener = UnixListener::bind(SOCKET_PATH)?;

    // Restrict socket permissions (root only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o600))?;
    }

    println!("zoid listening on {}", SOCKET_PATH);

    let gen_manager = GenerationManager::new()?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => match handle_client(stream, &gen_manager) {
                Ok(true) => {
                    println!("Shutdown requested. Exiting...");
                    if Path::new(PID_PATH).exists() {
                        let _ = fs::remove_file(PID_PATH);
                    }
                    break;
                }
                Ok(false) => {}
                Err(e) => eprintln!("Error handling client: {}", e),
            },
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(mut stream: UnixStream, gen_manager: &GenerationManager) -> Result<bool> {
    let request: Request = zoi_system::protocol::receive_message(&mut stream)?;
    let mut should_exit = false;
    let response = match request {
        Request::Shutdown => {
            should_exit = true;
            Response::Ok
        }
        Request::GetStatus => {
            let current = gen_manager.get_current_generation_id().unwrap_or(None);
            let status_msg = match current {
                Some(id) => format!("zoid is active. Current generation: {}", id),
                None => "zoid is active. No active generation found.".to_string(),
            };
            Response::Status(status_msg)
        }
        Request::ListGenerations => match gen_manager.list_generations() {
            Ok(gens) => Response::Generations(gens),
            Err(e) => Response::Error(e.to_string()),
        },
        Request::RollbackGeneration(target_id) => {
            let mut gens = gen_manager.list_generations()?;
            gens.sort_by_key(|g| g.id);

            let current_id = gen_manager.get_current_generation_id()?.unwrap_or(0);

            if target_id >= current_id {
                Response::Error(format!(
                    "Target generation {} is not older than current generation {}",
                    target_id, current_id
                ))
            } else {
                let gens_to_rollback: Vec<_> = gens
                    .into_iter()
                    .filter(|g| g.id > target_id && g.id <= current_id)
                    .rev()
                    .collect();

                let mut rolled_back_ids = Vec::new();
                let mut error = None;

                for g in gens_to_rollback {
                    if let Some(tid) = g.transaction_id {
                        println!(
                            "Rolling back transaction {} for generation {}...",
                            tid, g.id
                        );
                        if let Err(e) = zoi_transaction::rollback(&tid) {
                            error = Some(format!(
                                "Failed to roll back transaction {} for generation {}: {}",
                                tid, g.id, e
                            ));
                            break;
                        }
                    } else {
                        println!(
                            "Warning: Generation {} has no transaction ID. Performing legacy activation.",
                            g.id
                        );
                    }
                    rolled_back_ids.push(g.id);
                }

                if let Some(err) = error {
                    Response::Error(err)
                } else {
                    if let Err(e) = gen_manager.activate_generation(target_id) {
                        Response::Error(format!(
                            "Transactions rolled back, but failed to activate generation {}: {}",
                            target_id, e
                        ))
                    } else {
                        Response::Success(format!(
                            "Successfully rolled back to generation {}. (Rolled back generations: {:?})",
                            target_id, rolled_back_ids
                        ))
                    }
                }
            }
        }
        Request::ApplySystemConfig(config) => {
            println!("Applying system configuration...");

            // --- Phase 1: Declarative Uninstallation ---
            // We identify packages that are currently installed in the system scope
            // but are no longer present in the new system.lua configuration.
            if let Ok(installed) = zoi_resolver::local::get_installed_packages() {
                let current_system_packages: Vec<_> = installed
                    .into_iter()
                    .filter(|m| m.scope == zoi::Scope::System)
                    .collect();

                let new_package_specs = &config.packages;

                for manifest in current_system_packages {
                    let mut is_still_requested = false;
                    for spec in new_package_specs {
                        if let Ok(request) = zoi_resolver::resolve::parse_source_string(spec)
                            && request.name == manifest.name
                            && request.sub_package == manifest.sub_package
                            && request.repo.as_ref().is_none_or(|r| r == &manifest.repo)
                            && request
                                .handle
                                .as_ref()
                                .is_none_or(|h| h == &manifest.registry_handle)
                        {
                            is_still_requested = true;
                            break;
                        }
                    }

                    if !is_still_requested {
                        let source = zoi_resolver::local::installed_manifest_source(&manifest);
                        println!("Removing package no longer in configuration: {}...", source);
                        if let Err(e) = zoi_uninstall::run(
                            &source,
                            Some(zoi::Scope::System),
                            true,
                            false,
                            false,
                        ) {
                            eprintln!(
                                "Warning: failed to uninstall orphaned system package {}: {}",
                                source, e
                            );
                        }
                    }
                }
            }

            // --- Phase 2: Installation ---
            let sources = config.packages.clone();
            let install_options = zoi::SourceInstallOptions {
                scope_override: Some(zoi::Scope::System),
                yes: true,
                ..Default::default()
            };

            if let Err(e) = zoi::install_sources(&sources, &install_options) {
                Response::Error(format!("Failed to install system packages: {}", e))
            } else {
                let transaction_id = zoi_transaction::get_last_transaction_id().ok().flatten();
                match gen_manager
                    .create_generation_with_transaction(config.packages, transaction_id)
                {
                    Ok(id) => {
                        // Update Bootloader
                        let mut boot_msg = String::new();
                        if let Ok(gens) = gen_manager.list_generations()
                            && let Some(new_gen) = gens.iter().find(|g| g.id == id)
                            && let Ok(bootloader) = zoi_system::boot::detect_bootloader()
                            && let Ok((kernel, initrd)) = gen_manager.find_boot_assets(new_gen)
                        {
                            let cmdline = config.system.kernel_params.as_deref().unwrap_or("");
                            if let Err(e) =
                                bootloader.install_entry(new_gen, &kernel, &initrd, cmdline)
                            {
                                boot_msg = format!(" (Bootloader update failed: {})", e);
                            } else {
                                boot_msg = format!(
                                    " (Bootloader entry installed via {})",
                                    bootloader.name()
                                );
                            }
                        }

                        if let Err(e) = gen_manager.activate_generation(id) {
                            Response::Error(format!(
                                "Failed to activate new generation {}: {}",
                                id, e
                            ))
                        } else {
                            // Apply Services
                            if let Err(e) = zoi_system::service::apply_services(&config.services) {
                                eprintln!("Warning: Failed to apply some services: {}", e);
                            }

                            // Update fstab
                            let fstab_content =
                                zoi_system::mount::generate_fstab(&config.filesystems);
                            if let Err(e) = std::fs::write("/etc/fstab", fstab_content) {
                                eprintln!("Warning: Failed to update /etc/fstab: {}", e);
                            }

                            // Prune old generations
                            if let Ok(zoi_cfg) = zoi_core::config::read_config()
                                && let Err(e) =
                                    gen_manager.prune_generations(zoi_cfg.system_generations_limit)
                            {
                                eprintln!("Warning: Failed to prune old generations: {}", e);
                            }

                            Response::Success(format!(
                                "Applied system configuration. New generation: {}{}",
                                id, boot_msg
                            ))
                        }
                    }
                    Err(e) => Response::Error(e.to_string()),
                }
            }
        }
    };

    zoi_system::protocol::send_message(&mut stream, &response)?;
    Ok(should_exit)
}
