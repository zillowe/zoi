use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use colored::Colorize;

use crate::cli::{Cli, SetupScope};
use crate::pkg::{install, local, plugin, types};
use crate::utils;

/// Returns the path to the completion script for a given shell and scope.
fn get_completion_path(shell: Shell, scope: SetupScope) -> Result<PathBuf> {
    if scope == SetupScope::System {
        if !utils::is_admin() {
            return Err(anyhow!(
                "System-wide installation requires root privileges. Please \
                 run with sudo or as an administrator.",
            ));
        }
        Ok(match shell {
            Shell::Bash => {
                PathBuf::from("/usr/share/bash-completion/completions/zoi")
            }
            Shell::Elvish => PathBuf::from("/usr/share/elvish/lib/zoi.elv"),
            Shell::Fish => {
                PathBuf::from("/usr/share/fish/vendor_completions.d/zoi.fish")
            }
            Shell::Zsh => PathBuf::from("/usr/share/zsh/site-functions/_zoi"),
            _ => {
                return Err(anyhow!(
                    "System-wide completion installation not supported for \
                     this shell.",
                ));
            }
        })
    } else {
        let config_dir = crate::pkg::utils::get_user_config_dir()?;
        let data_dir = crate::pkg::utils::get_user_data_dir()?;
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow!("Home directory not found"))?;
        Ok(match shell {
            Shell::Bash => data_dir.join("bash-completion/completions/zoi"),
            Shell::Zsh => home.join(".zsh/completions/_zoi"),
            Shell::Fish => config_dir.join("fish/completions/zoi.fish"),
            Shell::Elvish => config_dir.join("elvish/completions/zoi.elv"),
            Shell::PowerShell => {
                if cfg!(windows) {
                    home.join(
                        "Documents/PowerShell/Microsoft.PowerShell_profile.ps1"
                    )
                } else {
                    home.join(
                        ".config/powershell/Microsoft.PowerShell_profile.ps1"
                    )
                }
            }
            _ => {
                return Err(anyhow!(
                    "User-level completion installation not supported for \
                     this shell.",
                ));
            }
        })
    }
}

/// Installs the completion script for a given shell and scope.
fn install_completions(
    shell: Shell,
    scope: SetupScope,
    cmd: &mut clap::Command
) -> Result<()> {
    if cfg!(windows) && scope == SetupScope::System {
        return Err(anyhow!(
            "System-wide shell setup is not supported on Windows.",
        ));
    }

    let path = get_completion_path(shell, scope)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if shell == Shell::PowerShell {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)?;
        writeln!(file)?;
        let mut script_buf = Vec::new();
        generate(shell, cmd, "zoi", &mut script_buf);
        let script = post_process_completions(
            shell,
            String::from_utf8_lossy(&script_buf).to_string()
        );
        file.write_all(script.as_bytes())?;
        println!(
            "PowerShell completion script appended to your profile: {}",
            path.display()
        );
        println!(
            "Please restart your shell or run '. $PROFILE' to activate it."
        );
    } else {
        let mut script_buf = Vec::new();
        generate(shell, cmd, "zoi", &mut script_buf);
        let script = post_process_completions(
            shell,
            String::from_utf8_lossy(&script_buf).to_string()
        );
        let mut file = fs::File::create(&path)?;
        file.write_all(script.as_bytes())?;
        println!("{shell} completions installed in: {}", path.display());
    }

    if shell == Shell::Zsh && scope == SetupScope::User {
        println!(
            "Ensure the directory is in your $fpath. Add this to your .zshrc \
             if it's not:"
        );
        println!(
            "  fpath=({} $fpath)",
            path.parent()
                .ok_or_else(|| anyhow!(
                    "Path should have a parent directory: {}",
                    path.display()
                ))?
                .display()
        );
    }

    Ok(())
}

/// Post-processes a completion script for a given shell.
fn post_process_completions(shell: Shell, mut script: String) -> String {
    match shell {
        Shell::Zsh => {
            let helper = r#"
_zoi_packages() {
    local -a entries
    local line
    while IFS= read -r line; do
        [[ -z "$line" ]] && continue
        entries+=("$line")
    done < <(zoi complete zsh $CURRENT "${words[@]}" 2>/dev/null)
    _describe -t packages 'packages' entries
}

_zoi_all_packages() {
    _zoi_packages
}

_zoi_installed_packages() {
    _zoi_packages
}
"#;
            let mut parts = script.splitn(2, '\n');
            let header = parts.next().unwrap_or("");
            let body = parts.next().unwrap_or("");

            script = format!("{header}\n{helper}\n{body}");

            script = script
                .replace("':ALL_SOURCES: '", "':package:(_zoi_packages)'");
            script = script
                .replace("':ALL_PACKAGES: '", "':package:(_zoi_packages)'");
            script = script
                .replace("':INST_PACKAGES: '", "':package:(_zoi_packages)'");

            let desc_marker =
                " -- Package identifier (e.g. @repo/name, path, or URL):";
            let mut search_start = 0;
            while let Some(pos) = script[search_start..].find(desc_marker) {
                let abs_pos = search_start + pos;
                let after_colon = abs_pos + desc_marker.len();
                if let Some(quote_pos) = script[after_colon..].find('\'') {
                    let action_end = after_colon + quote_pos;
                    script.replace_range(
                        after_colon..action_end,
                        ":_zoi_packages"
                    );
                }
                search_start = after_colon;
            }
        }
        Shell::Bash => {
            let helpers = r#"
_zoi_all_packages_comp() {
    local cur=${COMP_WORDS[COMP_CWORD]}
    local pkgs=$(zoi list -a --names 2>/dev/null)
    COMPREPLY=( $(compgen -W "${pkgs}" -- "$cur") )
}

_zoi_installed_packages_comp() {
    local cur=${COMP_WORDS[COMP_CWORD]}
    local pkgs=$(zoi list --names 2>/dev/null)
    COMPREPLY=( $(compgen -W "${pkgs}" -- "$cur") )
}

_zoi_wrapper() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local prev="${COMP_WORDS[COMP_CWORD-1]}"
    local cmd="${COMP_WORDS[1]}"

    if [[ "$prev" == -* ]]; then
        _zoi
        return 0
    fi

    case $cmd in
        install|i|in|add|show|exec|x|create|clone|use|tree|man|shell)
            _zoi_all_packages_comp
            return 0
            ;;
        uninstall|un|rm|remove|mark|m|update|up|why|files|pin|unpin|downgrade|dg|rollback)
            _zoi_installed_packages_comp
            return 0
            ;;
    esac

    _zoi
}
complete -F _zoi_wrapper zoi
"#;
            script = format!("{script}\n{helpers}");
        }
        _ => {}
    }
    script
}

/// Runs the shell setup command.
///
/// # Errors
///
/// Returns an error if:
/// - Root privileges are required for system-wide setup but elevation fails.
/// - The shell completion installation fails.
/// - The system path setup fails.
pub fn run(shell: Shell, scope: SetupScope) -> Result<()> {
    if scope == SetupScope::System && !utils::is_admin() {
        let exe = std::env::current_exe()?;
        let args: Vec<String> = std::env::args().collect();
        let escalator = crate::pkg::utils::get_privilege_escalator()
            .ok_or_else(|| {
                anyhow!(
                    "Root privileges required for system-wide setup, but \
                     neither 'sudo' nor 'doas' was found."
                )
            })?;

        let status = Command::new(escalator)
            .arg(&exe)
            .args(args.get(1..).unwrap_or(&[]))
            .status()
            .map_err(|e| anyhow!("Failed to elevate privileges: {e}"))?;
        std::process::exit(status.code().unwrap_or(1));
    }

    println!(
        "{} Setting up shell: {}...",
        "::".bold().blue(),
        shell.to_string().cyan()
    );

    let mut cmd = Cli::command();
    install_completions(shell, scope, &mut cmd)?;

    install_package_completions(shell, scope)?;

    println!();

    let scope_to_pass = match scope {
        SetupScope::User => types::Scope::User,
        SetupScope::System => types::Scope::System
    };
    utils::setup_path(scope_to_pass)?;
    Ok(())
}

/// Returns the directory for package completions for a given scope and shell.
fn get_completions_dir(scope: SetupScope, shell: &str) -> Result<PathBuf> {
    match scope {
        SetupScope::User => crate::pkg::utils::get_user_completions_dir(shell),
        SetupScope::System => {
            if cfg!(target_os = "windows") {
                Ok(PathBuf::from(format!(
                    "C:\\ProgramData\\zoi\\pkgs\\shell\\{shell}"
                )))
            } else {
                let base = match shell {
                    "bash" => "/usr/share/bash-completion/completions",
                    "zsh" => "/usr/share/zsh/site-functions",
                    "fish" => "/usr/share/fish/vendor_completions.d",
                    "elvish" => "/usr/share/elvish/lib",
                    _ => "/usr/local/share/zoi/completions"
                };
                Ok(PathBuf::from(base))
            }
        }
    }
}

/// Installs the package-specific completion directories for a given shell and
/// scope.
fn install_package_completions(shell: Shell, scope: SetupScope) -> Result<()> {
    let shell_name = match shell {
        Shell::Bash => "bash",
        Shell::Zsh => "zsh",
        Shell::Fish => "fish",
        Shell::Elvish => "elvish",
        _ => return Ok(())
    };

    let completions_dir = get_completions_dir(scope, shell_name)?;
    fs::create_dir_all(&completions_dir)?;

    match shell {
        Shell::Zsh => {
            let fpath_entry = completions_dir.to_string_lossy().to_string();
            println!(
                "{} Add this to your .zshrc to load package completions:",
                "::".bold().blue()
            );
            println!("  fpath=({fpath_entry:?} $fpath)");
            println!("  autoload -Uz compinit && compinit");
        }
        Shell::Bash => {
            let bash_completion_dir = match scope {
                SetupScope::User => {
                    let home = dirs::home_dir()
                        .ok_or_else(|| anyhow!("Home directory not found"))?;
                    home.join(".local/share/bash-completion/completions")
                }
                SetupScope::System => {
                    PathBuf::from("/usr/share/bash-completion/completions")
                }
            };
            if bash_completion_dir.exists() {
                println!(
                    "{} Package completions directory: {}",
                    "::".bold().blue(),
                    completions_dir.display()
                );
                println!(
                    "  Completions from installed packages will be available \
                     automatically."
                );
            }
        }
        Shell::Fish => {
            println!(
                "{} Package completions directory: {}",
                "::".bold().blue(),
                completions_dir.display()
            );
            println!(
                "  Completions from installed packages will be available \
                 automatically."
            );
        }
        _ => {}
    }

    Ok(())
}

/// Prints the shell hook script for a given shell.
///
/// # Errors
///
/// Returns an error if the shell hook is not supported for the given shell.
pub fn print_hook(shell: Shell) -> Result<()> {
    match shell {
        Shell::Bash => {
            println!(
                r#"
_zoi_hook() {{
  local previous_exit_status=$?;
  eval "$(zoi env --export-shell bash)";
  return $previous_exit_status;
}};
if [[ ";${{PROMPT_COMMAND[*]:-}};" != *";_zoi_hook;"* ]]; then
  if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND=(_zoi_hook "${{PROMPT_COMMAND[@]}}")
  else
    PROMPT_COMMAND="_zoi_hook${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}"
  fi
fi
"#
            );
        }
        Shell::Zsh => {
            println!(
                r#"
_zoi_hook() {{
  eval "$(zoi env --export-shell zsh)";
}};
typeset -ag precmd_functions;
if [[ -z "${{precmd_functions[(r)_zoi_hook]}}" ]]; then
  precmd_functions+=(_zoi_hook);
fi
"#
            );
        }
        Shell::Fish => {
            println!(
                r"
function _zoi_hook --on-variable PWD
  zoi env --export-shell fish | source
end
"
            );
        }
        _ => return Err(anyhow!("Shell hook not supported for {shell:?}"))
    }
    Ok(())
}

/// Enters an ephemeral shell with the given packages installed.
///
/// # Errors
///
/// Returns an error if:
/// - Dependency resolution fails.
/// - Package installation fails.
/// - Temporary directory creation fails.
/// - Symlinking binary files fails.
/// - Executing the shell command fails.
///
/// # Panics
///
/// Panics if a mutex lock is poisoned.
pub fn enter_ephemeral_shell(
    package_sources: &[String],
    run_cmd: Option<String>,
    verbose: bool,
    _plugin_manager: Option<&plugin::PluginManager>
) -> Result<()> {
    if verbose {
        println!("{} Resolving ephemeral environment...", "::".bold().blue());
    }

    let installed_before: HashSet<String> = local::get_installed_packages()?
        .into_iter()
        .map(|m| local::installed_manifest_source(&m))
        .collect();

    let (graph, _non_zoi_deps) = install::resolver::resolve_dependency_graph(
        package_sources,
        None,
        false,
        true,
        true,
        None,
        !verbose,
        None
    )?;

    let install_plan =
        install::plan::create_install_plan(&graph.nodes, None, false)?;
    let stages = graph.toposort()?;

    let mut session_installed = Vec::new();

    if !install_plan.is_empty() {
        if verbose {
            println!(
                "{} Preparing {} ephemeral dependencies...",
                "::".bold().blue(),
                install_plan.len()
            );
        }
        let m = indicatif::MultiProgress::new();
        if !verbose {
            m.set_draw_target(indicatif::ProgressDrawTarget::hidden());
        }
        let session_installed_mutex = Mutex::new(Vec::new());

        for stage in stages {
            use rayon::prelude::*;
            stage.into_par_iter().try_for_each(|pkg_id| -> Result<()> {
                let node = graph.nodes.get(&pkg_id).ok_or_else(|| {
                    anyhow!("Package node missing from graph for '{pkg_id}'")
                })?;
                let action = install_plan.get(&pkg_id).ok_or_else(|| {
                    anyhow!("Install action missing for package '{pkg_id}'")
                })?;

                let manifest = install::installer::install_node(
                    node,
                    action,
                    Some(&m),
                    None,
                    true,
                    false,
                    false,
                    verbose,
                    false
                )?;

                let mut session_lock = session_installed_mutex
                    .lock()
                    .expect("failed to lock session_installed_mutex");
                session_lock.push(manifest);
                Ok(())
            })?;
        }
        session_installed = session_installed_mutex
            .into_inner()
            .expect("failed to get session_installed_mutex inner value");
    }

    let temp_dir = tempfile::Builder::new().prefix("zoi-shell-").tempdir()?;
    let temp_bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&temp_bin_dir)?;

    for node in graph.nodes.values() {
        let handle = &node.registry_handle;
        let pkg = &node.pkg;

        let package_dir =
            local::get_package_dir(pkg.scope, handle, &pkg.repo, &pkg.name)?;
        let version_dir = package_dir.join(&node.version);
        let bin_dir = version_dir.join("bin");

        if bin_dir.exists() {
            for entry in fs::read_dir(bin_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() || path.is_symlink() {
                    let file_name = path.file_name().ok_or_else(|| {
                        anyhow!("Path has no file name: {}", path.display())
                    })?;
                    let dest = temp_bin_dir.join(file_name);
                    utils::symlink_file(&path, &dest)?;
                }
            }
        }
    }

    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut new_path = temp_bin_dir.to_string_lossy().to_string();
    if let Ok(old_path) = std::env::var("PATH") {
        new_path = format!("{new_path}{sep}{old_path}");
    }

    let package_list = package_sources.join(",");

    let shell_bin = std::env::var("SHELL").unwrap_or_else(|_| {
        if cfg!(windows) {
            "pwsh".to_string()
        } else {
            "bash".to_string()
        }
    });

    let mut envs = HashMap::new();
    envs.insert("PATH".to_string(), new_path);
    envs.insert("ZOI_SHELL".to_string(), "ephemeral".to_string());
    envs.insert("IN_ZOI_SHELL".to_string(), "ephemeral".to_string());
    envs.insert("ZOI_SHELL_PACKAGES".to_string(), package_list);

    #[cfg(target_os = "linux")]
    let mut shell_command = {
        use std::path::Path;
        let sysroot = zoi_core::sysroot::get_sysroot();
        if let Some(root) = sysroot {
            if verbose {
                println!(
                    "{} Entering shell within sysroot: {}",
                    "::".bold().yellow(),
                    root.display()
                );
            }

            let extra_binds = vec![(
                temp_dir.path().to_path_buf(),
                temp_dir.path().to_path_buf()
            )];

            if let Some(cmd_str) = run_cmd {
                let args = vec!["-c".to_string(), cmd_str];
                crate::sandbox::wrap_command_in_root(
                    &root,
                    Path::new(&shell_bin),
                    &args,
                    &envs,
                    &extra_binds,
                    false
                )?
            } else {
                crate::sandbox::wrap_command_in_root(
                    &root,
                    Path::new(&shell_bin),
                    &[],
                    &envs,
                    &extra_binds,
                    false
                )?
            }
        } else if let Some(cmd_str) = run_cmd {
            if verbose {
                println!("{} Running: {}", "::".bold().blue(), cmd_str.cyan());
            }
            let mut c = if cfg!(windows) {
                Command::new("pwsh")
            } else {
                Command::new("bash")
            };
            if cfg!(windows) {
                c.arg("-Command");
            } else {
                c.arg("-c");
            }
            c.arg(&cmd_str);
            c.envs(&envs);
            c
        } else {
            if verbose {
                println!(
                    "{} Entering ephemeral shell (type 'exit' to leave)...",
                    "::".bold().green()
                );
            }
            let mut c = Command::new(&shell_bin);
            c.envs(&envs);
            c
        }
    };

    #[cfg(not(target_os = "linux"))]
    let mut shell_command = {
        if let Some(cmd_str) = run_cmd {
            if verbose {
                println!("{} Running: {}", "::".bold().blue(), cmd_str.cyan());
            }
            let mut c = if cfg!(windows) {
                Command::new("pwsh")
            } else {
                Command::new("bash")
            };
            if !cfg!(windows) {
                c.arg("-c");
            } else {
                c.arg("-Command");
            }
            c.arg(&cmd_str);
            c.envs(&envs);
            c
        } else {
            if verbose {
                println!(
                    "{} Entering ephemeral shell (type 'exit' to leave)...",
                    "::".bold().green()
                );
            }
            let mut c = Command::new(&shell_bin);
            c.envs(&envs);
            c
        }
    };

    let status = shell_command.status()?;

    if !session_installed.is_empty() {
        if verbose {
            println!(
                "{} Cleaning up ephemeral packages...",
                "::".bold().blue()
            );
        }
        for manifest in session_installed {
            let ident = local::installed_manifest_source(&manifest);
            if installed_before.contains(&ident) {
                continue;
            }
            let version_dir = match get_version_dir_from_manifest(&manifest) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!(
                        "Warning: failed to resolve path for {ident}: {e}"
                    );
                    continue;
                }
            };
            if version_dir.exists()
                && let Err(e) = fs::remove_dir_all(&version_dir)
            {
                eprintln!(
                    "Warning: failed to cleanup ephemeral package {ident}: {e}"
                );
            }
            let package_dir = version_dir
                .parent()
                .expect("version directory should have a parent");
            if let Ok(mut entries) = fs::read_dir(package_dir) {
                let has_other_entries = entries.any(|e| {
                    e.as_ref().is_ok_and(|e| {
                        e.file_name() != "latest"
                            && e.file_name() != "dependents"
                    })
                });
                if !has_other_entries {
                    let _ = fs::remove_dir_all(package_dir);
                }
            }
        }
    }

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Returns the version directory for a given install manifest.
fn get_version_dir_from_manifest(
    manifest: &zoi_core::types::InstallManifest
) -> Result<PathBuf> {
    local::get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version
    )
}
