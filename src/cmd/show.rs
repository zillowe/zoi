use crate::pkg::{resolve, types::Package};
use colored::*;
use std::fs;

pub fn run(source: &str, raw: bool) {
    let source = source.trim();
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
    for method in &pkg.installation {
        let type_str = &method.install_type;
        let display_type = match type_str.as_str() {
            "com_binary" => "Compressed Binary".to_string(),
            _ => {
                let mut chars = type_str.chars();
                match chars.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        };

        let platforms_str = if method.platforms.is_empty() || method.platforms.iter().any(|p| p == "any") {
            "any".italic().to_string()
        } else {
            let mut platform_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
            for p in &method.platforms {
                let parts: Vec<&str> = p.split('-').collect();
                if parts.len() == 2 {
                    let os = parts[0];
                    let arch = parts[1];
                    platform_map.entry(os.to_string()).or_default().push(arch.to_string());
                } else {
                    platform_map.entry(p.to_string()).or_default();
                }
            }

            let mut display_parts = Vec::new();
            let mut sorted_os: Vec<_> = platform_map.keys().collect();
            sorted_os.sort();

            for os in sorted_os {
                let archs = platform_map.get(os).unwrap();
                let mut capitalized_os = os.chars();
                let os_display = match capitalized_os.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + capitalized_os.as_str(),
                };

                if archs.is_empty() {
                    display_parts.push(os_display);
                } else {
                    display_parts.push(format!("{} ({})", os_display, archs.join(", ")));
                }
            }
            display_parts.join(", ")
        };

        println!("  - {}: {}", display_type.yellow(), platforms_str);
    }
}
