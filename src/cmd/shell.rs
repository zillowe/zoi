use crate::cli::{Cli, SetupScope};
use crate::pkg::{install, local, plugin, types};
use crate::utils;
use anyhow::{Result, anyhow};
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use colored::*;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

fn get_completion_path(shell: Shell, scope: SetupScope) -> Result<PathBuf> {
    if scope == SetupScope::System {
        if !utils::is_admin() {
            return Err(anyhow!(
                "System-wide installation requires root privileges. Please run with sudo or as an administrator.",
            ));
        }
        Ok(match shell {
            Shell::Bash => PathBuf::from("/usr/share/bash-completion/completions/zoi"),
            Shell::Elvish => PathBuf::from("/usr/share/elvish/lib/zoi.elv"),
            Shell::Fish => PathBuf::from("/usr/share/fish/vendor_completions.d/zoi.fish"),
            Shell::Zsh => PathBuf::from("/usr/share/zsh/site-functions/_zoi"),
            _ => {
                return Err(anyhow!(
                    "System-wide completion installation not supported for this shell.",
                ));
            }
        })
    } else {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Home directory not found"))?;
        Ok(match shell {
            Shell::Bash => home.join(".local/share/bash-completion/completions/zoi"),
            Shell::Zsh => home.join(".zsh/completions/_zoi"),
            Shell::Fish => home.join(".config/fish/completions/zoi.fish"),
            Shell::Elvish => home.join(".config/elvish/completions/zoi.elv"),
            Shell::PowerShell => {
                if cfg!(windows) {
                    home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1")
                } else {
                    home.join(".config/powershell/Microsoft.PowerShell_profile.ps1")
                }
            }
            _ => {
                return Err(anyhow!(
                    "User-level completion installation not supported for this shell.",
                ));
            }
        })
    }
}

fn install_completions(shell: Shell, scope: SetupScope, cmd: &mut clap::Command) -> Result<()> {
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
        let script =
            post_process_completions(shell, String::from_utf8_lossy(&script_buf).to_string());
        file.write_all(script.as_bytes())?;
        println!(
            "PowerShell completion script appended to your profile: {:?}",
            path
        );
        println!("Please restart your shell or run '. $PROFILE' to activate it.");
    } else {
        let mut script_buf = Vec::new();
        generate(shell, cmd, "zoi", &mut script_buf);
        let script =
            post_process_completions(shell, String::from_utf8_lossy(&script_buf).to_string());
        let mut file = fs::File::create(&path)?;
        file.write_all(script.as_bytes())?;
        println!("{} completions installed in: {:?}", shell, path);
    }

    if shell == Shell::Zsh && scope == SetupScope::User {
        println!("Ensure the directory is in your $fpath. Add this to your .zshrc if it's not:");
        println!(
            "  fpath=({:?} $fpath)",
            path.parent()
                .ok_or_else(|| anyhow!("Path should have a parent directory: {:?}", path))?
        );
    }

    Ok(())
}

fn post_process_completions(shell: Shell, mut script: String) -> String {
    match shell {
        Shell::Zsh => {
            let helper = r#"
_zoi_all_packages() {
    local -a packages
    packages=(${(f)"$(_zoi_do_list_all)"})
    _describe -t packages 'available packages' packages
}

_zoi_installed_packages() {
    local -a packages
    packages=(${(f)"$(_zoi_do_list_installed)"})
    _describe -t packages 'installed packages' packages
}

_zoi_do_list_all() {
    zoi list -a --completion 2>/dev/null
}

_zoi_do_list_installed() {
    zoi list --completion 2>/dev/null
}
"#;
            script.push_str(helper);
            script = script.replace("':ALL_SOURCES: '", "':package:(_zoi_all_packages)'");
            script = script.replace("':ALL_PACKAGES: '", "':package:(_zoi_all_packages)'");
            script = script.replace("':INST_PACKAGES: '", "':package:(_zoi_installed_packages)'");
        }
        Shell::Bash => {
            let helper = r#"
_zoi_all_packages() {
    local cur=${COMP_WORDS[COMP_CWORD]}
    local pkgs=$(zoi list -a --names 2>/dev/null)
    COMPREPLY=( $(compgen -W "${pkgs}" -- "$cur") )
}
"#;
            script = format!("{}\n{}", helper, script);
        }
        _ => {}
    }
    script
}

pub fn run(shell: Shell, scope: SetupScope) -> Result<()> {
    println!(
        "{} Setting up shell: {}...",
        "::".bold().blue(),
        shell.to_string().cyan()
    );

    let mut cmd = Cli::command();
    install_completions(shell, scope, &mut cmd)?;

    println!();

    let scope_to_pass = match scope {
        SetupScope::User => types::Scope::User,
        SetupScope::System => types::Scope::System,
    };
    utils::setup_path(scope_to_pass)?;
    Ok(())
}

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
                r#"
function _zoi_hook --on-variable PWD
  zoi env --export-shell fish | source
end
"#
            );
        }
        _ => return Err(anyhow!("Shell hook not supported for {:?}", shell)),
    }
    Ok(())
}

pub fn enter_ephemeral_shell(
    package_sources: &[String],
    run_cmd: Option<String>,
    _plugin_manager: Option<&plugin::PluginManager>,
) -> Result<()> {
    println!("{} Resolving ephemeral environment...", "::".bold().blue());

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
        true,
    )?;

    let install_plan = install::plan::create_install_plan(&graph.nodes, None, false)?;
    let stages = graph.toposort()?;

    let mut session_installed = Vec::new();

    if !install_plan.is_empty() {
        println!(
            "{} Preparing {} ephemeral dependencies...",
            "::".bold().blue(),
            install_plan.len()
        );
        let m = indicatif::MultiProgress::new();
        let session_installed_mutex = Mutex::new(Vec::new());

        for stage in stages {
            use rayon::prelude::*;
            stage.into_par_iter().try_for_each(|pkg_id| -> Result<()> {
                let node = graph
                    .nodes
                    .get(&pkg_id)
                    .ok_or_else(|| anyhow!("Package node missing from graph for '{}'", pkg_id))?;
                let action = install_plan
                    .get(&pkg_id)
                    .ok_or_else(|| anyhow!("Install action missing for package '{}'", pkg_id))?;

                let manifest =
                    install::installer::install_node(node, action, Some(&m), None, true, false)?;

                let mut session_lock = session_installed_mutex.lock().unwrap();
                session_lock.push(manifest);
                Ok(())
            })?;
        }
        session_installed = session_installed_mutex.into_inner().unwrap();
    }

    let temp_dir = tempfile::Builder::new().prefix("zoi-shell-").tempdir()?;
    let temp_bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&temp_bin_dir)?;

    for node in graph.nodes.values() {
        let handle = &node.registry_handle;
        let pkg = &node.pkg;

        let package_dir = local::get_package_dir(pkg.scope, handle, &pkg.repo, &pkg.name)?;
        let version_dir = package_dir.join(&node.version);
        let bin_dir = version_dir.join("bin");

        if bin_dir.exists() {
            for entry in fs::read_dir(bin_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() || path.is_symlink() {
                    let file_name = path
                        .file_name()
                        .ok_or_else(|| anyhow!("Path has no file name: {:?}", path))?;
                    let dest = temp_bin_dir.join(file_name);
                    utils::symlink_file(&path, &dest)?;
                }
            }
        }
    }

    let mut new_path = temp_bin_dir.to_string_lossy().to_string();
    if let Ok(old_path) = std::env::var("PATH") {
        new_path = format!(
            "{}{}{}",
            new_path,
            if cfg!(windows) { ";" } else { ":" },
            old_path
        );
    }

    let package_list = package_sources.join(",");

    let mut shell_command = if let Some(cmd_str) = run_cmd {
        println!("{} Running: {}", "::".bold().blue(), cmd_str.cyan());
        if cfg!(windows) {
            let mut c = Command::new("pwsh");
            c.arg("-Command").arg(&cmd_str);
            c
        } else {
            let mut c = Command::new("bash");
            c.arg("-c").arg(&cmd_str);
            c
        }
    } else {
        let shell_bin = std::env::var("SHELL").unwrap_or_else(|_| {
            if cfg!(windows) {
                "pwsh".to_string()
            } else {
                "bash".to_string()
            }
        });

        println!(
            "{} Entering ephemeral shell (type 'exit' to leave)...",
            "::".bold().green()
        );

        Command::new(&shell_bin)
    };

    shell_command
        .env("PATH", new_path)
        .env("ZOI_SHELL", "ephemeral")
        .env("IN_ZOI_SHELL", "ephemeral")
        .env("ZOI_SHELL_PACKAGES", package_list);

    let status = shell_command.status()?;

    if !session_installed.is_empty() {
        println!("{} Cleaning up ephemeral packages...", "::".bold().blue());
        for manifest in session_installed {
            let ident = local::installed_manifest_source(&manifest);
            if !installed_before.contains(&ident)
                && let Err(e) = crate::pkg::uninstall::run(&ident, Some(manifest.scope), true)
            {
                eprintln!(
                    "Warning: failed to cleanup ephemeral package {}: {}",
                    ident, e
                );
            }
        }
    }

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
