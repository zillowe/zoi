use crate::pkg;
use crate::utils;
use colored::*;

pub fn run(verbose: bool, fallback: bool, no_pm: bool, no_shell_setup: bool) {
    println!("{}", "--- Syncing Package Databases ---".yellow().bold());

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
    let url_storage;
    let url = match url_or_keyword {
        "default" => {
            url_storage = pkg::config::get_default_registry();
            &url_storage
        }
        "gitlab" => "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoidberg.git",
        "github" => "https://github.com/Zillowe/Zoidberg.git",
        "codeberg" => "https://codeberg.org/Zillowe/Zoidberg.git",
        _ => url_or_keyword,
    };

    if let Err(e) = pkg::config::set_default_registry(url) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
    println!("Default registry set to: {}", url.cyan());
    println!("The new registry will be used the next time you run 'zoi sync'");
}

pub fn add_registry(url: &str) {
    if let Err(e) = pkg::config::add_added_registry(url) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
    println!("Registry '{}' added.", url.cyan());
    println!("It will be synced on the next 'zoi sync' run.");
}

pub fn remove_registry(handle: &str) {
    if let Err(e) = pkg::config::remove_added_registry(handle) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
    println!("Registry with handle '{}' removed.", handle.cyan());
}

pub fn list_registries() {
    match crate::pkg::config::read_config() {
        Ok(config) => {
            let db_root = match crate::pkg::resolve::get_db_root() {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("\n{}: {}", "Error".red().bold(), e);
                    std::process::exit(1);
                }
            };

            println!("{}", "--- Configured Registries ---".bold());

            if let Some(default) = config.default_registry {
                let handle = &default.handle;
                let mut desc = "".to_string();
                if !handle.is_empty() {
                    let repo_path = db_root.join(handle);
                    if let Ok(repo_config) = crate::pkg::config::read_repo_config(&repo_path) {
                        desc = format!(" - {}", repo_config.description);
                    }
                }
                let handle_str = if handle.is_empty() {
                    "<not synced>".italic().to_string()
                } else {
                    handle.cyan().to_string()
                };
                println!("[Set] {}: {}{}", handle_str, default.url, desc);
            } else {
                println!("[Set]: <not set>");
            }

            if !config.added_registries.is_empty() {
                println!();
                for reg in config.added_registries {
                    let handle = &reg.handle;
                    let mut desc = "".to_string();
                    if !handle.is_empty() {
                        let repo_path = db_root.join(handle);
                        if let Ok(repo_config) = crate::pkg::config::read_repo_config(&repo_path) {
                            desc = format!(" - {}", repo_config.description);
                        }
                    }
                    let handle_str = if handle.is_empty() {
                        "<not synced>".italic().to_string()
                    } else {
                        handle.cyan().to_string()
                    };
                    println!("[Add] {}: {}{}", handle_str, reg.url, desc);
                }
            }
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            std::process::exit(1);
        }
    }
}
