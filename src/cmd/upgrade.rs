use crate::pkg;
use colored::*;

pub fn run() {
    println!("{}", "--- Upgrading Zoi ---".yellow());

    if let Err(e) = pkg::upgrade::run() {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }

    println!(
        "\n{}",
        "Zoi upgraded successfully! Please restart your shell for changes to take effect.".green()
    );
}
