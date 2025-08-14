use crate::cli::Cli;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use colored::*;
use std::fs;
use std::io::{Error, ErrorKind};

fn install_bash_completions(cmd: &mut clap::Command) -> Result<(), Error> {
    println!("Installing bash completions...");
    let home = dirs::home_dir()
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "Home directory not found"))?;
    let completions_dir = home.join(".local/share/bash-completion/completions");
    fs::create_dir_all(&completions_dir)?;
    let path = completions_dir.join("zoi");
    let mut file = fs::File::create(&path)?;
    generate(Shell::Bash, cmd, "zoi", &mut file);
    println!("Bash completions installed in: {:?}", path);
    Ok(())
}

fn install_zsh_completions(cmd: &mut clap::Command) -> Result<(), Error> {
    println!("Installing zsh completions...");
    let home = dirs::home_dir()
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "Home directory not found"))?;
    let completions_dir = home.join(".zsh/completions");
    fs::create_dir_all(&completions_dir)?;
    let path = completions_dir.join("_zoi");
    let mut file = fs::File::create(&path)?;
    generate(Shell::Zsh, cmd, "zoi", &mut file);
    println!("Zsh completions installed in: {:?}", path);
    println!("Ensure the directory is in your $fpath. Add this to your .zshrc if it's not:");
    println!("  fpath=({:?} $fpath)", completions_dir);
    Ok(())
}

fn install_fish_completions(cmd: &mut clap::Command) -> Result<(), Error> {
    println!("Installing fish completions...");
    let home = dirs::home_dir()
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "Home directory not found"))?;
    let completions_dir = home.join(".config/fish/completions");
    fs::create_dir_all(&completions_dir)?;
    let path = completions_dir.join("zoi.fish");
    let mut file = fs::File::create(&path)?;
    generate(Shell::Fish, cmd, "zoi", &mut file);
    println!("Fish completions installed in: {:?}", path);
    Ok(())
}

fn install_elvish_completions(cmd: &mut clap::Command) -> Result<(), Error> {
    println!("Installing elvish completions...");
    let home = dirs::home_dir()
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "Home directory not found"))?;
    let completions_dir = home.join(".config/elvish/completions");
    fs::create_dir_all(&completions_dir)?;
    let path = completions_dir.join("zoi.elv");
    let mut file = fs::File::create(&path)?;
    generate(Shell::Elvish, cmd, "zoi", &mut file);
    println!("Elvish completions installed in: {:?}", path);
    Ok(())
}

#[cfg(windows)]
fn install_powershell_completions(cmd: &mut clap::Command) -> Result<(), Error> {
    println!("Installing PowerShell completions...");
    let home = dirs::home_dir()
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "Home directory not found"))?;
    let profile_dir = home.join("Documents/PowerShell");
    fs::create_dir_all(&profile_dir)?;
    let profile_path = profile_dir.join("Microsoft.PowerShell_profile.ps1");

    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&profile_path)?;
    use std::io::Write;
    writeln!(file, "")?;

    let mut script_buf = Vec::new();
    generate(Shell::PowerShell, cmd, "zoi", &mut script_buf);
    file.write_all(&script_buf)?;

    println!(
        "PowerShell completion script appended to your profile: {:?}",
        profile_path
    );
    println!("Please restart your shell or run '. $PROFILE' to activate it.");
    Ok(())
}

pub fn run(shell: Shell) {
    let mut cmd = Cli::command();

    let result = if cfg!(windows) {
        #[cfg(windows)]
        {
            if shell == Shell::PowerShell {
                install_powershell_completions(&mut cmd)
            } else {
                eprintln!("On Windows, only PowerShell completions are supported.");
                return;
            }
        }
        #[cfg(not(windows))]
        {
            Ok(())
        }
    } else {
        match shell {
            Shell::Bash => install_bash_completions(&mut cmd),
            Shell::Zsh => install_zsh_completions(&mut cmd),
            Shell::Fish => install_fish_completions(&mut cmd),
            Shell::Elvish => install_elvish_completions(&mut cmd),
            Shell::PowerShell => {
                eprintln!("PowerShell completions are only for Windows.");
                return;
            }
            _ => {
                eprintln!(
                    "{}: Automatic installation for '{}' is not yet supported.",
                    "Warning".yellow(),
                    shell
                );
                println!(
                    "Please generate them manually: zoi generate-completions {}",
                    shell
                );
                return;
            }
        }
    };

    if let Err(e) = result {
        eprintln!("{}: {}", "Error".red().bold(), e);
    }
}
