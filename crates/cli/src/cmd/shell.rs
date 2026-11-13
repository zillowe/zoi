use std::collections::HashMap;
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

/// Removes plan entries for packages that are already installed.
///
/// Ephemeral environments (`zoi shell -p`, `zoi exec`) only install what is
/// missing and never modify a persistent installation. Installing over an
/// installed package would rebuild and replace its version directory without
/// updating its manifest, file index or database record (ephemeral installs
/// are not recorded), leaving the new files paired with stale records.
///
/// A node counts as installed when an installed manifest matches it on
/// registry, repo, name, sub-package, version and scope (split packages share
/// a version directory, and another scope's copy does not provide the files)
/// and its version directory exists. The installed copy is used as-is even
/// if the registry now has a newer revision of that version; a warning points
/// to `zoi update` in that case.
pub fn drop_already_installed<P, N>(
    install_plan: &mut HashMap<String, install::plan::InstallAction, P>,
    nodes: &HashMap<String, install::resolver::InstallNode, N>,
    installed: &[types::InstallManifest]
) where
    P: std::hash::BuildHasher,
    N: std::hash::BuildHasher
{
    install_plan.retain(|pkg_id, _| {
        let Some(node) = nodes.get(pkg_id) else {
            return true;
        };
        let ident = local::package_source_string(
            &node.registry_handle,
            &node.pkg.repo,
            &node.pkg.name,
            node.sub_package.as_deref(),
            &node.version
        );
        let Some(manifest) = installed.iter().find(|manifest| {
            manifest.scope == node.pkg.scope
                && local::installed_manifest_source(manifest) == ident
        }) else {
            return true;
        };
        let present = local::get_package_version_dir(
            node.pkg.scope,
            &node.registry_handle,
            &node.pkg.repo,
            &node.pkg.name,
            &node.version
        )
        .is_ok_and(|dir| dir.exists());
        if !present {
            return true;
        }
        if manifest.revision != node.revision {
            eprintln!(
                "{}: using installed {} revision {} (the registry has \
                 revision {}); run `zoi update` to upgrade it.",
                "Warning".yellow(),
                ident,
                manifest.revision,
                node.revision
            );
        }
        false
    });
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

    // Ephemeral packages are written into the shared store, so the store
    // lock is held while they are resolved and installed. It is released
    // before the user's program runs: holding it for the lifetime of an
    // interactive session would block every other Zoi command.
    let install_lock = crate::pkg::lock::acquire_lock()?;

    let installed_manifests = local::get_installed_packages()?;

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

    let mut install_plan =
        install::plan::create_install_plan(&graph.nodes, None, false)?;
    drop_already_installed(
        &mut install_plan,
        &graph.nodes,
        &installed_manifests
    );
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
                // Packages that are already installed have no plan entry.
                let Some(action) = install_plan.get(&pkg_id) else {
                    return Ok(());
                };

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

    // `SHELL` is environment-controlled, so it goes through the validated
    // resolver instead of being executed directly.
    let shell_bin = zoi_core::utils::resolve_shell_binary()?;

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

    drop(install_lock);
    let status = shell_command.status()?;

    // Cleanup removes store directories, so it needs the lock back. If
    // another Zoi process holds it now, skipping cleanup is safer than
    // deleting files underneath it.
    let cleanup_lock = if session_installed.is_empty() {
        None
    } else {
        match crate::pkg::lock::acquire_lock() {
            Ok(guard) => Some(guard),
            Err(e) => {
                eprintln!(
                    "Warning: skipping cleanup of ephemeral packages: {e}"
                );
                session_installed.clear();
                None
            }
        }
    };

    cleanup_ephemeral_installs(
        session_installed,
        &installed_manifests,
        verbose
    );

    drop(cleanup_lock);

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Removes the packages an ephemeral session installed.
///
/// `installed_before` is the snapshot of installed manifests taken before the
/// session. A session manifest that matches one of them exactly (same package,
/// sub-package, version and scope) belongs to a persistent installation and is
/// left alone. When another persistently installed sub-package shares the
/// version directory, only the session's own store files are removed, since
/// deleting the directory would also remove that sub-package. Otherwise the
/// version directory (and an emptied package directory) is removed.
pub fn cleanup_ephemeral_installs(
    session_installed: Vec<types::InstallManifest>,
    installed_before: &[types::InstallManifest],
    verbose: bool
) {
    if session_installed.is_empty() {
        return;
    }
    if verbose {
        println!("{} Cleaning up ephemeral packages...", "::".bold().blue());
    }
    for manifest in session_installed {
        let ident = local::installed_manifest_source(&manifest);
        if installed_before.iter().any(|before| {
            before.scope == manifest.scope
                && local::installed_manifest_source(before) == ident
        }) {
            continue;
        }
        let version_dir = match get_version_dir_from_manifest(&manifest) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Warning: failed to resolve path for {ident}: {e}");
                continue;
            }
        };

        let shares_version_dir = installed_before.iter().any(|before| {
            get_version_dir_from_manifest(before)
                .is_ok_and(|dir| dir == version_dir)
        });
        if shares_version_dir {
            for file in &manifest.installed_files {
                let Some(rel) = file.strip_prefix("${pkgstore}/") else {
                    continue;
                };
                let Some(path) = store_entry_path(&version_dir, rel) else {
                    continue;
                };
                // `symlink_metadata` and `remove_file` act on a final symlink
                // itself, so links are removed even when their target is
                // outside the store or already gone.
                if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_dir())
                    && let Err(e) = fs::remove_file(&path)
                {
                    eprintln!(
                        "Warning: failed to remove ephemeral file {}: {e}",
                        path.display()
                    );
                }
            }
            continue;
        }

        if version_dir.exists()
            && let Err(e) = fs::remove_dir_all(&version_dir)
        {
            eprintln!(
                "Warning: failed to cleanup ephemeral package {ident}: {e}"
            );
        }
        if let Some(package_dir) = version_dir.parent()
            && let Ok(mut entries) = fs::read_dir(package_dir)
        {
            let has_other_entries = entries.any(|e| {
                e.as_ref().is_ok_and(|e| {
                    e.file_name() != "latest" && e.file_name() != "dependents"
                })
            });
            if !has_other_entries {
                let _ = fs::remove_dir_all(package_dir);
            }
        }
    }
}

/// Resolves a `${pkgstore}`-relative entry inside `version_dir` without
/// following its final component.
///
/// Only the parent directories are checked against `version_dir` (with
/// symlinks resolved); the last component is appended as-is, so a symlink
/// there can be inspected and unlinked even when its target lies outside the
/// directory or no longer exists. Returns `None` for absolute paths, `..`
/// components, or parents that resolve outside `version_dir`.
fn store_entry_path(
    version_dir: &std::path::Path,
    rel: &str
) -> Option<PathBuf> {
    let rel = std::path::Path::new(rel);
    let name = rel.file_name()?;
    let parent = rel.parent().unwrap_or_else(|| std::path::Path::new(""));
    let parent_dir = zoi_core::utils::safe_join(version_dir, parent).ok()?;
    Some(parent_dir.join(name))
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
