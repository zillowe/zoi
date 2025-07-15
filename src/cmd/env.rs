use crate::project::{config, environment};
use colored::*;

pub fn run(env_alias: Option<String>) {
    match config::load() {
        Ok(config) => {
            if let Err(e) = environment::setup(env_alias.as_deref(), &config) {
                eprintln!("\n{}: {}", "Error".red().bold(), e);
            } else {
                println!("\n{}", "Environment setup complete.".green());
            }
        }
        Err(e) => eprintln!("{}: {}", "Error".red().bold(), e),
    }
}
