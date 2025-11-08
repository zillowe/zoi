use crate::cli::{Cli, SetupScope};
use crate::pkg::types::Scope;
use crate::utils;
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use colored::*;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

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
            Shell::PowerShell => home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1"),
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
        use std::io::Write;
        writeln!(file)?;
        let mut script_buf = Vec::new();
        generate(shell, cmd, "zoi", &mut script_buf);
        file.write_all(&script_buf)?;
        println!(
            "PowerShell completion script appended to your profile: {:?}",
            path
        );
        println!("Please restart your shell or run '. $PROFILE' to activate it.");
    } else {
        let mut file = fs::File::create(&path)?;
        generate(shell, cmd, "zoi", &mut file);
        println!("{} completions installed in: {:?}", shell, path);
    }

    if shell == Shell::Zsh && scope == SetupScope::User {
        println!("Ensure the directory is in your $fpath. Add this to your .zshrc if it's not:");
        println!("  fpath=({:?} $fpath)", path.parent().unwrap());
    }

    Ok(())
}

pub fn run(shell: Shell, scope: SetupScope) -> Result<()> {
    println!("--- Setting up shell: {} ---", shell.to_string().cyan());

    let mut cmd = Cli::command();
    install_completions(shell, scope, &mut cmd)?;

    println!();

    let scope_to_pass = match scope {
        SetupScope::User => Scope::User,
        SetupScope::System => Scope::System,
    };
    utils::setup_path(scope_to_pass)?;
    Ok(())
}
