use crate::pkg::{install, types};
use colored::*;

pub fn run(sources: &[String], force: bool, interactive: bool, yes: bool) {
    let mode = if interactive {
        install::InstallMode::Interactive
    } else {
        install::InstallMode::PreferBinary
    };

    let mut failed_packages = Vec::new();

    for source in sources {
        println!("=> Installing package: {}", source.cyan().bold());
        if let Err(e) = install::run_installation(
            source,
            mode.clone(),
            force,
            types::InstallReason::Direct,
            yes,
        ) {
            eprintln!(
                "{}: Failed to install \"{}\": {}",
                "Error".red().bold(),
                source,
                e
            );
            failed_packages.push(source.to_string());
        }
    }

    if !failed_packages.is_empty() {
        eprintln!(
            "\n{}: The following packages failed to install:",
            "Error".red().bold()
        );
        for pkg in &failed_packages {
            eprintln!("  - {}", pkg);
        }
        std::process::exit(1);
    }
}
