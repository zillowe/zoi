use crate::pkg;
use colored::*;

pub fn run(branch: &str, status: &str, number: &str) {
    println!("{}", "--- Upgrading Zoi ---".yellow());

    if let Err(e) = pkg::upgrade::run(branch, status, number) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }



    println!(
        "\n{}",
        "Zoi upgraded successfully! Please restart your shell for changes to take effect.".green()
    );
}

