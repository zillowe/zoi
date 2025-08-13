use crate::pkg;
use colored::*;

pub fn run(branch: &str, status: &str, number: &str, full: bool, force: bool) {
    println!("{}", "--- Upgrading Zoi ---".yellow());

    match pkg::upgrade::run(branch, status, number, full, force) {
        Ok(()) => {
            println!(
                "\n{}",
                "Zoi upgraded successfully! Please restart your shell for changes to take effect."
                    .green()
            );
        }
        Err(e) if e.to_string() == "already_on_latest" => {}
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            std::process::exit(1);
        }
    }
}
