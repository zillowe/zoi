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
            let mut pkg: Package = serde_yaml::from_str(&content).unwrap();
            if let Some(repo_name) = resolved_source.repo_name {
                pkg.repo = repo_name;
            }
            pkg.version =
                Some(resolve::get_default_version(&pkg).unwrap_or_else(|_| "N/A".to_string()));
            print_beautiful(&pkg);
        }
        Err(e) => eprintln!("{}: {}", "Error".red(), e),
    }
}

fn print_beautiful(pkg: &crate::pkg::types::Package) {
    println!(
        "{} {} - {}",
        pkg.name.bold().green(),
        pkg.version.as_deref().unwrap_or("").dimmed(),
        pkg.repo
    );
    if let Some(website) = &pkg.website {
        println!("Website: {}", website.cyan().underline());
    }
    if !pkg.git.is_empty() {
        println!("Git Repo: {}", pkg.git.cyan().underline());
    }
    println!("\n{}\n", pkg.description);

    println!("{}: {}", "License".bold(), pkg.license);

    let mut maintainer_line = format!(
        "{}: {} <{}>",
        "Maintainer".bold(),
        pkg.maintainer.name,
        pkg.maintainer.email
    );
    if let Some(website) = &pkg.maintainer.website {
        maintainer_line.push_str(&format!(" - {}", website.cyan().underline()));
    }
    if pkg.maintainer.key.is_some() {
        maintainer_line.push_str(&format!(" {}", "(Has Key)".dimmed()));
    }
    println!("{}", maintainer_line);

    if let Some(author) = &pkg.author {
        let mut author_line = format!("{}: {}", "Author".bold(), author.name);
        if let Some(email) = &author.email {
            author_line.push_str(&format!(" <{}>", email));
        }
        if let Some(website) = &author.website {
            author_line.push_str(&format!(" - {}", website.cyan().underline()));
        }
        if author.key.is_some() {
            author_line.push_str(&format!(" {}", "(Has Key)".dimmed()));
        }
        println!("{}", author_line);
    }

    let type_display = match pkg.package_type {
        crate::pkg::types::PackageType::Package => "Package",
        crate::pkg::types::PackageType::Collection => "Collection",
        crate::pkg::types::PackageType::Service => "Service",
        crate::pkg::types::PackageType::Config => "Config",
    };
    println!("{}: {}", "Type".bold(), type_display);

    let scope_display = match pkg.scope {
        crate::pkg::types::Scope::User => "User",
        crate::pkg::types::Scope::System => "System",
    };
    println!("{}: {}", "Scope".bold(), scope_display);

    if !pkg.tags.is_empty() {
        println!("{}: {}", "Tags".bold(), pkg.tags.join(", "));
    }

    if let Some(bins) = &pkg.bins {
        if !bins.is_empty() {
            println!("{}: {}", "Provides".bold(), bins.join(", ").blue());
        }
    }

    if let Some(conflicts) = &pkg.conflicts {
        if !conflicts.is_empty() {
            println!("{}: {}", "Conflicts".bold(), conflicts.join(", ").red());
        }
    }

    if pkg.package_type == crate::pkg::types::PackageType::Package {
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

            let platforms_str = if method.platforms.is_empty()
                || method.platforms.iter().any(|p| p == "any")
            {
                "any".italic().to_string()
            } else {
                let mut platform_map: std::collections::HashMap<String, Vec<String>> =
                    std::collections::HashMap::new();
                for p in &method.platforms {
                    let parts: Vec<&str> = p.split('-').collect();
                    if parts.len() == 2 {
                        let os = parts[0];
                        let arch = parts[1];
                        platform_map
                            .entry(os.to_string())
                            .or_default()
                            .push(arch.to_string());
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

    if let Some(deps) = &pkg.dependencies {
        println!("\n{}:", "Dependencies".bold());

        if let Some(build) = &deps.build {
            println!("  {}:", "Build".bold());
            for dep in build.get_required_simple() {
                println!("    - {}", dep);
            }
            for group in build.get_required_options() {
                println!(
                    "    - {}: {} (choose {})",
                    group.name.bold(),
                    group.desc,
                    if group.all { "any" } else { "one" }
                );
                for dep in &group.depends {
                    let parts: Vec<&str> = dep.rsplitn(2, ':').collect();
                    if parts.len() == 2
                        && !parts[0].contains(['=', '>', '<', '~', '^'])
                        && !parts[1].is_empty()
                    {
                        println!("      - {}: {}", parts[1], parts[0].italic());
                    } else {
                        println!("      - {}", dep);
                    }
                }
            }
            for dep in build.get_optional() {
                let parts: Vec<&str> = dep.rsplitn(2, ':').collect();
                if parts.len() == 2
                    && !parts[0].contains(['=', '>', '<', '~', '^'])
                    && !parts[1].is_empty()
                {
                    println!("    - {} (optional): {}", parts[1], parts[0].italic());
                } else {
                    println!("    - {} (optional)", dep);
                }
            }
        }

        if let Some(runtime) = &deps.runtime {
            println!("  {}:", "Runtime".bold());
            for dep in runtime.get_required_simple() {
                println!("    - {}", dep);
            }
            for group in runtime.get_required_options() {
                println!(
                    "    - {}: {} (choose {})",
                    group.name.bold(),
                    group.desc,
                    if group.all { "any" } else { "one" }
                );
                for dep in &group.depends {
                    let parts: Vec<&str> = dep.rsplitn(2, ':').collect();
                    if parts.len() == 2
                        && !parts[0].contains(['=', '>', '<', '~', '^'])
                        && !parts[1].is_empty()
                    {
                        println!("      - {}: {}", parts[1], parts[0].italic());
                    } else {
                        println!("      - {}", dep);
                    }
                }
            }
            for dep in runtime.get_optional() {
                let parts: Vec<&str> = dep.rsplitn(2, ':').collect();
                if parts.len() == 2
                    && !parts[0].contains(['=', '>', '<', '~', '^'])
                    && !parts[1].is_empty()
                {
                    println!("    - {} (optional): {}", parts[1], parts[0].italic());
                } else {
                    println!("    - {} (optional)", dep);
                }
            }
        }
    }
}
