use crate::pkg;
use crate::utils;
use colored::*;

pub fn run(verbose: bool, fallback: bool, no_pm: bool, no_shell_setup: bool) {
    println!("{}", "--- Syncing Package Database ---".yellow().bold());

    if let Err(e) = pkg::sync::run(verbose, fallback, no_pm) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }

    println!("{}", "Sync complete.".green());

    if no_shell_setup {
        return;
    }

    println!(
        "\n{}",
        "--- Setting up shell completions ---".yellow().bold()
    );
    if let Some(shell) = utils::get_current_shell() {
        println!("Detected shell: {}", shell.to_string().cyan());
        crate::cmd::shell::run(shell);
    } else {
        println!(
            "{}",
            "Could not detect shell. Skipping auto-completion setup.".yellow()
        );
    }
}

pub fn set_registry(url_or_keyword: &str) {
    let url = match url_or_keyword {
        "default" | "gitlab" => "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoidberg.git",
        "github" => "https://github.com/Zillowe/Zoidberg.git",
        "codeberg" => "https://codeberg.org/Zillowe/Zoidberg.git",
        _ => url_or_keyword,
    };

    if let Err(e) = pkg::config::set_registry(url) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
    println!("Registry set to: {}", url.cyan());
    println!("The new registry will be used the next time you run 'zoi sync'");
}

pub fn show_registry() {
    match pkg::config::read_config() {
        Ok(config) => {
            let registry = config.registry.unwrap_or_else(|| {
                "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoidberg.git".to_string()
            });
            println!("Current registry: {}", registry.cyan());
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            std::process::exit(1);
        }
    }
}
