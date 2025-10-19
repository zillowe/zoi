use crate::pkg;
use colored::*;

pub fn run(
    branch: &str,
    status: &str,
    number: &str,
    force: bool,
    tag: Option<String>,
    custom_branch: Option<String>,
) {
    println!("{}", "--- Upgrading Zoi ---".yellow());

    match pkg::upgrade::run(branch, status, number, force, tag, custom_branch) {
        Ok(()) => {
            println!(
                "\n{}",
                "Zoi upgraded successfully! Please restart your shell for changes to take effect."
                    .green()
            );
            println!(
                "\n{}: https://github.com/Zillowe/Zoi/blob/main/CHANGELOG.md",
                "Changelog".cyan().bold()
            );
            println!(
                "\n{}: To update shell completions, run 'zoi shell <your-shell>'.",
                "Hint".cyan().bold()
            );
        }
        Err(e) if e.to_string() == "already_on_latest" => {}
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            std::process::exit(1);
        }
    }
}
