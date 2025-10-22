use crate::cli::SetupScope;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct InstallCommand {
    /// Path to the package archive file (e.g. path/to/name-os-arch.pkg.tar.zst)
    #[arg(required = true)]
    pub package_file: PathBuf,
    /// The scope to install the package to (user or system-wide)
    #[arg(long, value_enum, default_value_t = SetupScope::User)]
    pub scope: SetupScope,
    /// Automatically answer yes to all prompts
    #[arg(long)]
    pub yes: bool,
}

pub fn run(args: InstallCommand) {
    let scope = match args.scope {
        SetupScope::User => crate::pkg::types::Scope::User,
        SetupScope::System => crate::pkg::types::Scope::System,
    };
    if let Err(e) =
        crate::pkg::package::install::run(&args.package_file, Some(scope), "local", None, args.yes)
    {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
