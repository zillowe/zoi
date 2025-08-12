use crate::pkg;
use colored::*; 

pub fn run(verbose: bool) {
    println!("{}", "--- Syncing Package Database ---".yellow().bold());

    if let Err(e) = pkg::sync::run(verbose) {
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }

    println!("{}", "Sync complete.".green());
}

pub fn set_registry(url_or_keyword: &str) {
    let url = match url_or_keyword {
        "default" | "gitlab" => "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs.git",
        "github" => "https://github.com/Zillowe/Zoi-Pkgs.git",
        "codeberg" => "https://codeberg.org/Zillowe/Zoi-Pkgs.git",
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
                "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs.git".to_string()
            });
            println!("Current registry: {}", registry.cyan());
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            std::process::exit(1);
        }
    }
}

