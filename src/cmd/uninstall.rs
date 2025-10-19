use crate::pkg::{self, types};
use colored::*;

pub fn run(
    package_names: &[String],
    scope: Option<crate::cli::InstallScope>,
    local: bool,
    global: bool,
) {
    let mut scope_override = scope.map(|s| match s {
        crate::cli::InstallScope::User => types::Scope::User,
        crate::cli::InstallScope::System => types::Scope::System,
        crate::cli::InstallScope::Project => types::Scope::Project,
    });

    if local {
        scope_override = Some(types::Scope::Project);
    } else if global {
        scope_override = Some(types::Scope::User);
    }

    for name in package_names {
        println!(
            "{}{}{}",
            "--- Uninstalling package '".yellow(),
            name.blue().bold(),
            "' ---".yellow()
        );

        if let Err(e) = pkg::uninstall::run(name, scope_override) {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
            continue;
        }

        println!("\n{}", "Uninstallation complete.".green());
    }
}
