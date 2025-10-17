use crate::pkg;
use clap::Parser;
use colored::*;

#[derive(Parser)]
pub struct CreateCommand {
    /// The source of the package (name, @repo/name, path to .pkg.lua, or URL)
    pub source: String,
    /// The application name and directory to create (defaults to package name)
    pub app_name: Option<String>,
}

pub fn run(args: CreateCommand, yes: bool) {
    if let Err(e) = pkg::create::run(&args.source, args.app_name, yes) {
        eprintln!("{}: {}", "Error".red().bold(), e);
    }
}
