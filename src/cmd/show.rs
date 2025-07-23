use crate::pkg::{resolve, types::Package};
use colored::*;
use std::fs;

pub fn run(source: &str, raw: bool) {
    match resolve::resolve_source(source) {
        Ok(resolved_source) => {
            let content = fs::read_to_string(&resolved_source.path).unwrap();
            if raw {
                println!("{content}");
                return;
            }
            let pkg: Package = serde_yaml::from_str(&content).unwrap();
            print_beautiful(&pkg);
        }
        Err(e) => eprintln!("{}: {}", "Error".red(), e),
    }
}

fn print_beautiful(pkg: &crate::pkg::types::Package) {
    println!(
        "{} {} - {}",
        pkg.name.bold().green(),
        pkg.version.dimmed(),
        pkg.repo
    );
    println!("Website");
    println!("{}", pkg.website.cyan().underline());
    println!("Git Repo");
    println!("{}", pkg.git.cyan().underline());
    println!("\n{}\n", pkg.description);

    println!("{}: {}", "License".bold(), pkg.license);
    println!(
        "{}: {} <{}>",
        "Maintainer".bold(),
        pkg.maintainer.name,
        pkg.maintainer.email
    );

    println!("\n{}:", "Available installation methods".bold());
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    for method in &pkg.installation {
        if crate::utils::is_platform_compatible(&platform, &method.platforms) {
            println!("  - {}", method.install_type.yellow());
        }
    }
}
