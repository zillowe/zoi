use crate::cli::SetupScope;
use crate::pkg::types::Scope;
use crate::utils;
use colored::*;

pub fn run(scope: SetupScope) {
    let scope_to_pass = match scope {
        SetupScope::User => Scope::User,
        SetupScope::System => Scope::System,
    };
    if let Err(e) = utils::setup_path(scope_to_pass) {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}
