use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::io::{Read, Write};
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
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if n < 1024 {
            break;
        }
    }

    let request: Request = serde_json::from_slice(&buffer)?;
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
        Request::RollbackGeneration(id) => match gen_manager.activate_generation(id) {
            Ok(_) => Response::Success(format!("Activated generation {}", id)),
            Err(e) => Response::Error(e.to_string()),
        },
        Request::ApplySystemConfig(config) => {
            println!("Applying system configuration...");
            let sources = config.packages.clone();
            let install_options = zoi::SourceInstallOptions {
                scope_override: Some(zoi::Scope::System),
                yes: true,
                ..Default::default()
            };

            if let Err(e) = zoi::install_sources(&sources, &install_options) {
                Response::Error(format!("Failed to install system packages: {}", e))
            } else {
                match gen_manager.create_generation(config.packages) {
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

    let response_bytes = serde_json::to_vec(&response)?;
    stream.write_all(&response_bytes)?;
    Ok(should_exit)
}
