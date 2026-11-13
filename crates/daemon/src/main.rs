//! zoid, the `ZoiOS` privileged system daemon.
//!
//! `zoid` owns the operations the CLI cannot perform as an unprivileged user:
//! reconciling the machine against `system.lua`, rolling system generations
//! back, and exposing the `PackageKit` D-Bus interface that desktop software
//! expects to find.
//!
//! It is a thin layer. Every operation it performs is implemented in
//! `zoi-system` or `zoi`, so the daemon and `zoi system` converge on the same
//! result rather than maintaining two code paths. What the daemon adds is
//! authentication (running as root, behind a 0600 socket) and lifetime (a
//! single process that desktop tools can talk to).

use std::fs;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use anyhow::Result;
use clap::{Parser, Subcommand};
use zoi_system::client::send_request;
use zoi_system::generation::GenerationManager;
use zoi_system::protocol::{Request, Response};

mod packagekit;

// Paths live in `zoi-system` so the daemon, the CLI client and the systemd unit
// cannot drift apart. Getting these out of sync is a silent failure: the client
// would connect to a socket nobody is listening on.
use zoi_system::protocol::{PID_PATH, RUNTIME_DIR, SOCKET_PATH};

/// zoid - The `ZoiOS` privileged system daemon.
#[derive(Parser)]
#[command(name = "zoid", author, about, version)]
struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    command: Commands
}

/// The two things an administrator can ask the daemon binary to do.
#[derive(Subcommand)]
enum Commands {
    /// Start the zoid daemon
    Start {
        /// Do not background the process
        #[arg(short, long)]
        foreground: bool,
        /// Do not expose the `PackageKit` D-Bus interface
        #[arg(long)]
        no_packagekit: bool
    },
    /// Stop the zoid daemon
    Stop
}

/// Parses the command line and dispatches to the requested action.
///
/// `zoid stop` is handled here rather than by the running daemon, so it works
/// even when the socket is unusable: it falls back to signalling the recorded
/// pid directly.
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            foreground,
            no_packagekit
        } => {
            if foreground {
                start_daemon(no_packagekit)?;
            } else {
                daemonize(no_packagekit)?;
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
                Ok(Response::Error(e)) => {
                    return Err(anyhow::anyhow!("Error stopping daemon: {e}"));
                }
                Err(e) => {
                    use nix::sys::signal::{self, Signal};
                    use nix::unistd::Pid;

                    eprintln!(
                        "Failed to connect to daemon: {e}. Checking for PID \
                         file..."
                    );
                    let pid = fs::read_to_string(PID_PATH)
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok())
                        .filter(|pid| *pid > 0 && pid_is_zoid(*pid))
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "zoid does not appear to be running (no \
                                 reachable socket and no live zoid PID in \
                                 {PID_PATH})"
                            )
                        })?;

                    signal::kill(Pid::from_raw(pid), Signal::SIGTERM).map_err(
                        |e| {
                            anyhow::anyhow!("Failed to kill process {pid}: {e}")
                        }
                    )?;
                    println!("Killed process {pid}.");
                    let _ = fs::remove_file(PID_PATH);
                }
                Ok(_) => {
                    return Err(anyhow::anyhow!(
                        "Unexpected response from daemon"
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Forks into the background and serves requests from the child.
///
/// This is the path for systems that predate `zoid.service`. Under systemd the
/// unit passes `--foreground` and `Type=exec` handles supervision properly,
/// including the pid file systemd maintains itself.
fn daemonize(no_packagekit: bool) -> Result<()> {
    use nix::unistd::{ForkResult, fork};

    // The child's stderr goes to /dev/null, so anything that can be checked
    // up front is checked here, where it can still fail the command.
    check_can_start()?;

    // SAFETY: `fork` is safe here because this is the only thread in the
    // process. `main` has not spawned anything, so the child inherits a
    // consistent copy of the address space and no other thread can observe a
    // half-updated data structure.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            println!("zoid started in background (PID: {child})");
            std::process::exit(0);
        }
        Ok(ForkResult::Child) => {
            // Close standard streams
            let dev_null = fs::File::open("/dev/null")?;
            let fd = dev_null.as_raw_fd();

            // SAFETY: `fd` comes from `dev_null`, which stays alive for the
            // duration of the block, and fds 0-2 are always valid targets. The
            // return values are ignored on purpose: the worst case is that one
            // stream keeps pointing at the inherited terminal.
            unsafe {
                nix::libc::dup2(fd, 0);
                nix::libc::dup2(fd, 1);
                nix::libc::dup2(fd, 2);
            }

            start_daemon(no_packagekit)?;
        }
        Err(e) => return Err(anyhow::anyhow!("Fork failed: {e}"))
    }
    Ok(())
}

/// Serves the daemon socket until a [`Request::Shutdown`] arrives.
///
/// The socket is created mode 0600 inside the runtime directory systemd owns,
/// so reaching this point already means the caller is root. That is the whole
/// authorization model: the daemon does no per-request checking, because an
/// unprivileged process cannot open the socket at all.
fn start_daemon(no_packagekit: bool) -> Result<()> {
    check_can_start()?;
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

    println!("zoid listening on {SOCKET_PATH}");

    // PackageKit is what makes desktop software work, so it is on by default.
    // A failure here costs GNOME Software and friends but must not cost the
    // whole ZoiOS layer, so it is a warning rather than a fatal error.
    if no_packagekit {
        println!("PackageKit interface disabled by --no-packagekit");
    } else if let Err(e) = packagekit::start_packagekit() {
        eprintln!(
            "Warning: failed to start the PackageKit interface: {e}. Desktop \
             package tools will not work; the zoi CLI is unaffected."
        );
    }

    let gen_manager = GenerationManager::new()?;

    // SIGHUP is accepted and ignored deliberately. The unit offers
    // `systemctl reload zoid`, and having that be an error would be worse than
    // having it be a no-op: configuration is re-read from disk on every
    // request, so there is genuinely nothing to reload.
    install_sighup_handler();

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
                Err(e) => eprintln!("Error handling client: {e}")
            },
            Err(e) => {
                eprintln!("Error accepting connection: {e}");
            }
        }
    }

    Ok(())
}

/// Checks that a daemon can start: the runtime directory exists and no other
/// zoid is running.
///
/// Run by the parent before forking as well as by the daemon itself, so that
/// `zoid start` fails with a non-zero exit status (and a visible message)
/// instead of reporting success while the detached child dies silently.
fn check_can_start() -> Result<()> {
    // Under systemd `RuntimeDirectory=` creates this; without it (the
    // non-systemd `zoid start` path) nothing does, and every write after
    // this would fail.
    ensure_runtime_dir()?;

    // A recorded PID only counts as a running daemon if that process is
    // actually zoid: PIDs are reused, and a stale file pointing at an
    // unrelated process must not block startup.
    if let Ok(old_pid) = fs::read_to_string(PID_PATH)
        && let Ok(pid) = old_pid.trim().parse::<i32>()
        && pid > 0
        && pid != i32::try_from(std::process::id()).unwrap_or(-1)
        && pid_is_zoid(pid)
    {
        return Err(anyhow::anyhow!("zoid is already running (PID: {pid})"));
    }
    Ok(())
}

/// Creates the runtime directory holding the socket and PID file.
///
/// Mode 0755 matches what systemd's `RuntimeDirectory=` produces: the socket
/// inside is 0600, so the directory itself does not need to hide anything.
fn ensure_runtime_dir() -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    let dir = Path::new(RUNTIME_DIR);
    if !dir.is_dir() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o755)
            .create(dir)
            .map_err(|e| {
                anyhow::anyhow!("Failed to create {RUNTIME_DIR}: {e}")
            })?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Returns whether `pid` is a live process running a `zoid` binary.
///
/// Checked through `/proc/<pid>/exe` rather than the mere existence of
/// `/proc/<pid>`, so a PID that has been recycled for another program is
/// treated as stale. A deleted-and-replaced binary shows up as
/// `zoid (deleted)`, which still counts.
fn pid_is_zoid(pid: i32) -> bool {
    fs::read_link(format!("/proc/{pid}/exe")).is_ok_and(|exe| {
        exe.file_name()
            .is_some_and(|name| name.to_string_lossy().contains("zoid"))
    })
}

/// Ignores `SIGHUP` so the process does not die from a `systemctl reload`.
///
/// The default action for `SIGHUP` is to terminate. The systemd unit declares
/// `ExecReload=/bin/kill -HUP $MAINPID`, so without this a reload would
/// silently kill the daemon and systemd would restart it. Since the daemon
/// re-reads configuration per request, ignoring the signal is the correct
/// behaviour rather than a workaround.
fn install_sighup_handler() {
    use std::sync::OnceLock;

    use nix::sys::signal::{SigHandler, Signal, signal};

    // `signal()` is process-wide, so it is installed at most once even if the
    // function is somehow reached twice.
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        // SAFETY: `SIG_IGN` is a valid handler and carries no state, so there
        // is nothing for the signal machinery to access and no `SigHandler`
        // lifetime to uphold.
        let result = unsafe { signal(Signal::SIGHUP, SigHandler::SigIgn) };
        if let Err(e) = result {
            eprintln!("Warning: failed to ignore SIGHUP: {e}");
        }
    });
}

/// Handles one request from the socket and writes back the response.
///
/// Returns whether the daemon should stop afterwards, so that the caller can
/// break out of its accept loop rather than threading a flag through it.
///
/// A failure to apply the configuration is reported *inside* the response, not
/// as an `Err`. The client asked a question and deserves an answer explaining
/// what went wrong; dropping the connection would leave `zoi system apply`
/// reporting nothing at all.
fn handle_client(
    mut stream: UnixStream,
    gen_manager: &GenerationManager
) -> Result<bool> {
    let request: Request = zoi_system::protocol::receive_message(&mut stream)?;
    let mut should_exit = false;
    let response = match request {
        Request::Shutdown => {
            should_exit = true;
            Response::Ok
        }
        Request::GetStatus => {
            let current =
                gen_manager.get_current_generation_id().unwrap_or(None);
            let status_msg = match current {
                Some(id) => {
                    format!("zoid is active. Current generation: {id}")
                }
                None => {
                    "zoid is active. No active generation found.".to_string()
                }
            };
            Response::Status(status_msg)
        }
        Request::ListGenerations => match gen_manager.list_generations() {
            Ok(gens) => Response::Generations(gens),
            Err(e) => Response::Error(e.to_string())
        },
        Request::RollbackGeneration(target_id) => {
            rollback_generation(target_id, gen_manager)
                .unwrap_or_else(|e| Response::Error(e.to_string()))
        }
        Request::PinGeneration(id, pinned) => {
            match gen_manager.pin_generation(id, pinned) {
                Ok(()) => {
                    let action = if pinned { "pinned" } else { "unpinned" };
                    Response::Success(format!(
                        "Generation {id} {action} successfully."
                    ))
                }
                Err(e) => Response::Error(e.to_string())
            }
        }
        Request::ApplySystemConfig(config) => {
            apply_system_config(*config, gen_manager)
        }
    };

    zoi_system::protocol::send_message(&mut stream, &response)?;
    Ok(should_exit)
}

/// Takes the Zoi store lock on behalf of a daemon request.
///
/// The daemon mutates the same store as the CLI, so it must hold the same lock
/// or a concurrent `zoi install` could interleave with it. The lock is
/// re-entrant per thread, so the library calls made while it is held (which
/// take it themselves) do not deadlock.
fn acquire_store_lock() -> Result<zoi_core::lock::LockGuard> {
    zoi_core::lock::acquire_lock().map_err(|e| {
        anyhow::anyhow!(
            "Another Zoi operation is in progress ({e}); try again once it \
             has finished."
        )
    })
}

/// Rolls the system back to an older generation.
///
/// Every failure is returned as an error so the caller can turn it into a
/// [`Response::Error`]: the client is always owed a reply.
fn rollback_generation(
    target_id: u32,
    gen_manager: &GenerationManager
) -> Result<Response> {
    let _lock = acquire_store_lock()?;

    let mut gens = gen_manager.list_generations()?;
    gens.sort_by_key(|g| g.id);

    let current_id = gen_manager.get_current_generation_id()?.unwrap_or(0);

    if target_id >= current_id {
        return Ok(Response::Error(format!(
            "Target generation {target_id} is not older than current \
             generation {current_id}"
        )));
    }

    let gens_to_rollback: Vec<_> = gens
        .into_iter()
        .filter(|g| g.id > target_id && g.id <= current_id)
        .rev()
        .collect();

    let mut rolled_back_ids = Vec::new();

    for g in gens_to_rollback {
        if let Some(tid) = g.transaction_id {
            println!(
                "Rolling back transaction {} for generation {}...",
                tid, g.id
            );
            if let Err(e) = zoi_transaction::rollback(&tid) {
                return Ok(Response::Error(format!(
                    "Failed to roll back transaction {} for generation {}: {}",
                    tid, g.id, e
                )));
            }
        } else {
            println!(
                "Warning: Generation {} has no transaction ID. Performing \
                 legacy activation.",
                g.id
            );
        }
        rolled_back_ids.push(g.id);
    }

    if let Err(e) = gen_manager.activate_generation(target_id) {
        return Ok(Response::Error(format!(
            "Transactions rolled back, but failed to activate generation \
             {target_id}: {e}"
        )));
    }

    Ok(Response::Success(format!(
        "Successfully rolled back to generation {target_id}. (Rolled back \
         generations: {rolled_back_ids:?})"
    )))
}

/// Returns the explicitly installed system packages that `config` no longer
/// requests.
///
/// Packages installed as dependencies are never returned: they belong to
/// whatever requested them, and removing them here would break a package the
/// configuration still wants.
fn orphaned_system_packages(
    config: &zoi_system::config::SystemConfig
) -> Vec<String> {
    let Ok(installed) = zoi_resolver::local::get_installed_packages() else {
        return Vec::new();
    };

    let requests: Vec<_> = config
        .packages
        .iter()
        .filter_map(|spec| {
            zoi_resolver::resolve::parse_source_string(spec).ok()
        })
        .collect();

    installed
        .into_iter()
        .filter(|m| m.scope == zoi::Scope::System)
        .filter(|m| matches!(m.reason, zoi_core::types::InstallReason::Direct))
        .filter(|manifest| {
            !requests.iter().any(|request| {
                request.name == manifest.name
                    && request.sub_package == manifest.sub_package
                    && request.repo.as_ref().is_none_or(|r| r == &manifest.repo)
                    && request
                        .handle
                        .as_ref()
                        .is_none_or(|h| h == &manifest.registry_handle)
            })
        })
        .map(|manifest| {
            zoi_resolver::local::installed_manifest_source(&manifest)
        })
        .collect()
}

/// Reconciles the machine against a `system.lua` configuration.
///
/// The whole operation holds the store lock. The requested packages are
/// installed first and packages dropped from the configuration are removed
/// only once that succeeded: removing first would leave the system without
/// them (and possibly unbootable) if the install then failed.
fn apply_system_config(
    config: zoi_system::config::SystemConfig,
    gen_manager: &GenerationManager
) -> Response {
    println!("Applying system configuration...");

    let _lock = match acquire_store_lock() {
        Ok(lock) => lock,
        Err(e) => return Response::Error(e.to_string())
    };

    // --- Phase 1: Installation ---
    let install_options = zoi::SourceInstallOptions {
        scope_override: Some(zoi::Scope::System),
        yes: true,
        ..Default::default()
    };

    if let Err(e) = zoi::install_sources(&config.packages, &install_options) {
        return Response::Error(format!(
            "Failed to install system packages: {e}. No packages were removed."
        ));
    }
    let transaction_id =
        zoi_transaction::get_last_transaction_id().ok().flatten();

    // --- Phase 2: Declarative uninstallation ---
    // Explicitly installed system packages that the new configuration no
    // longer lists are removed, now that everything it does list is in place.
    for source in orphaned_system_packages(&config) {
        println!("Removing package no longer in configuration: {source}...");
        if let Err(e) = zoi_uninstall::run(
            &source,
            Some(zoi::Scope::System),
            true,
            false,
            false
        ) {
            eprintln!(
                "Warning: failed to uninstall orphaned system package \
                 {source}: {e}"
            );
        }
    }

    let id = match gen_manager
        .create_generation_with_transaction(config.packages, transaction_id)
    {
        Ok(id) => id,
        Err(e) => return Response::Error(e.to_string())
    };

    // --- Boot ---
    //
    // The kernel pipeline (depmod, initramfs, Secure Boot signing, bootloader
    // entries) runs here as well as from the transaction hooks, because `zoi
    // system apply` is the path a fresh install takes and it has to be able
    // to produce a bootable system on its own.
    let mut boot_msg = String::new();
    let cmdline = config.system.kernel_params.as_deref().unwrap_or("");

    match zoi_system::kernel::regenerate_all(cmdline) {
        Ok(report) => {
            if !report.is_empty() {
                report.print();
                boot_msg = format!(
                    " (kernel prepared via {})",
                    report
                        .bootloader
                        .as_deref()
                        .unwrap_or("no bootloader found")
                );
            }
        }
        Err(e) => {
            boot_msg = format!(" (Boot update failed: {e})");
        }
    }

    if let Err(e) = gen_manager.activate_generation(id) {
        return Response::Error(format!(
            "Failed to activate new generation {id}: {e}"
        ));
    }

    // Apply accounts. Groups are reconciled before users because a user's
    // supplementary memberships have to resolve to real GIDs.
    match zoi_system::account::apply_accounts(
        &config.groups,
        &config.users,
        false
    ) {
        Ok(report) => zoi_system::account::print_report(&report),
        Err(e) => eprintln!("Warning: Failed to apply users/groups: {e}")
    }

    // Apply Services
    if let Err(e) = zoi_system::service::apply_services(&config.services) {
        eprintln!("Warning: Failed to apply some services: {e}");
    }

    // Update fstab
    let fstab_content = zoi_system::mount::generate_fstab(&config.filesystems);
    if let Err(e) = std::fs::write("/etc/fstab", fstab_content) {
        eprintln!("Warning: Failed to update /etc/fstab: {e}");
    }

    // Prune old generations
    if let Ok(zoi_cfg) = zoi_core::config::read_config()
        && let Err(e) =
            gen_manager.prune_generations(zoi_cfg.system_generations_limit)
    {
        eprintln!("Warning: Failed to prune old generations: {e}");
    }

    Response::Success(format!(
        "Applied system configuration. New generation: {id}{boot_msg}"
    ))
}
