use crate::pkg::install;
use colored::*;

pub fn run(source: &str, force: bool, interactive: bool, yes: bool) {
    let mode = if interactive {
        install::InstallMode::Interactive
    } else {
        install::InstallMode::PreferBinary
    };

    if let Err(e) = install::run_installation(
        source,
        mode,
        force,
        crate::pkg::types::InstallReason::Direct,
        yes,
    ) {
        eprintln!("{}: {}", "Installation failed".red().bold(), e);
    }
}
