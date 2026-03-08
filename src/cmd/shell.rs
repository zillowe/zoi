use crate::cli::{Cli, SetupScope};
use crate::pkg::{install, local, plugin, types};
use crate::utils;
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use colored::*;
use std::fs;
use std::io::{Error, ErrorKind, Write};
use std::path::PathBuf;
use std::process::Command;

fn get_completion_path(shell: Shell, scope: SetupScope) -> Result<PathBuf, Error> {
    if scope == SetupScope::System {
        if !utils::is_admin() {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "System-wide installation requires root privileges. Please run with sudo or as an administrator.",
            ));
        }
        Ok(match shell {
            Shell::Bash => PathBuf::from("/usr/share/bash-completion/completions/zoi"),
            Shell::Elvish => PathBuf::from("/usr/share/elvish/lib/zoi.elv"),
            Shell::Fish => PathBuf::from("/usr/share/fish/vendor_completions.d/zoi.fish"),
            Shell::Zsh => PathBuf::from("/usr/share/zsh/site-functions/_zoi"),
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "System-wide completion installation not supported for this shell.",
                ));
            }
        })
    } else {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "Home directory not found"))?;
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
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "User-level completion installation not supported for this shell.",
                ));
            }
        })
    }
}

fn install_completions(
    shell: Shell,
    scope: SetupScope,
    cmd: &mut clap::Command,
) -> Result<(), Error> {
    if cfg!(windows) && scope == SetupScope::System {
        return Err(Error::new(
            ErrorKind::Unsupported,
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
            path.parent().expect("Path should have a parent directory")
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
    _describe -t packages 'synced packages' packages
}

_zoi_do_list_all() {
    zoi list -a --completion 2>/dev/null
}
"#;
            script.push_str(helper);
            script = script.replace("':SOURCES: '", "':package:(_zoi_all_packages)'");
            script = script.replace("':PACKAGES: '", "':package:(_zoi_all_packages)'");
            script = script.replace("':PACKAGE: '", "':package:(_zoi_all_packages)'");
            script = script.replace("':package_name: '", "':package:(_zoi_all_packages)'");
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

pub fn enter_ephemeral_shell(
    package_sources: &[String],
    run_cmd: Option<String>,
    _plugin_manager: &plugin::PluginManager,
) -> Result<()> {
    println!("{} Resolving ephemeral environment...", "::".bold().blue());

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

    if !install_plan.is_empty() {
        println!(
            "{} Preparing {} ephemeral dependencies...",
            "::".bold().blue(),
            install_plan.len()
        );
        let m = indicatif::MultiProgress::new();
        for stage in stages {
            use rayon::prelude::*;
            stage.into_par_iter().try_for_each(|pkg_id| -> Result<()> {
                let node = graph
                    .nodes
                    .get(&pkg_id)
                    .expect("Package node missing from graph");
                let action = install_plan
                    .get(&pkg_id)
                    .expect("Install action missing for package");

                install::installer::install_node(node, action, Some(&m), None, true, false)?;
                Ok(())
            })?;
        }
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
                    let file_name = path.file_name().expect("Path should have a file name");
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

    if let Some(cmd_str) = run_cmd {
        println!("{} Running: {}", "::".bold().blue(), cmd_str.cyan());
        let mut child = if cfg!(windows) {
            Command::new("pwsh")
                .arg("-Command")
                .arg(&cmd_str)
                .env("PATH", new_path)
                .spawn()?
        } else {
            Command::new("bash")
                .arg("-c")
                .arg(&cmd_str)
                .env("PATH", new_path)
                .spawn()?
        };
        let status = child.wait()?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
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

        let mut child = Command::new(&shell_bin)
            .env("PATH", new_path)
            .env("ZOI_SHELL", "ephemeral")
            .spawn()?;

        let _ = child.wait()?;
        println!("{} Exited ephemeral shell.", "::".bold().blue());
    }

    Ok(())
}
