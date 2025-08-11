use crate::project::{config, runner};
use colored::*;

pub fn run(cmd_alias: Option<String>, args: Vec<String>) {
    match config::load() {
        Ok(config) => {
            if let Err(e) = runner::run(cmd_alias.as_deref(), &args, &config) {
                eprintln!("\n{}: {}", "Error".red().bold(), e);
            }
        }
        Err(e) => eprintln!("{}: {}", "Error".red().bold(), e),
    }
}
