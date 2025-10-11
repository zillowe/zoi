use crate::pkg::{
    local, resolve,
    types::{InstallManifest, Package},
};
use crate::utils;
use colored::*;
use std::fs;

pub fn run(source: &str, raw: bool) {
    let source = source.trim();
    match resolve::resolve_source(source) {
        Ok(resolved_source) => {
            if raw {
                let content = fs::read_to_string(&resolved_source.path).unwrap();
                println!("{content}");
                return;
            }
            let mut pkg: Package = crate::pkg::lua::parser::parse_lua_package(
                resolved_source.path.to_str().unwrap(),
                None,
            )
            .unwrap();
            if let Some(repo_name) = resolved_source.repo_name {
                pkg.repo = repo_name;
            }
            pkg.version = Some(
                resolve::get_default_version(&pkg, resolved_source.registry_handle.as_deref())
                    .unwrap_or_else(|_| "N/A".to_string()),
            );

            let installed_manifest = match local::is_package_installed(&pkg.name, pkg.scope) {
                Ok(manifest) => manifest,
                Err(e) => {
                    eprintln!("Warning: could not check installation status: {}", e);
                    None
                }
            };

            print_beautiful(&pkg, installed_manifest.as_ref());
        }
        Err(e) => eprintln!("{}: {}", "Error".red(), e),
    }
}

fn print_beautiful(pkg: &crate::pkg::types::Package, installed_manifest: Option<&InstallManifest>) {
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
    println!("{}", pkg.description);

    if let Some(manifest) = installed_manifest {
        println!(
            "{}: {} ({})",
            "Status".bold(),
            "Installed".green(),
            manifest.version
        );
    } else {
        println!("{}: {}", "Status".bold(), "Not Installed".red());
    }

    if !pkg.license.is_empty() {
        println!("{}: {}", "License".bold(), pkg.license);
        utils::check_license(&pkg.license);
    }

    let mut maintainer_line = format!(
        "{}: {} <{}>",
        "Maintainer".bold(),
        pkg.maintainer.name,
        pkg.maintainer.email
    );
    if let Some(website) = &pkg.maintainer.website {
        maintainer_line.push_str(&format!(" - {}", website.cyan().underline()));
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
        println!("{}", author_line);
    }

    let type_display = match pkg.package_type {
        crate::pkg::types::PackageType::Package => "Package",
        crate::pkg::types::PackageType::Collection => "Collection",
        crate::pkg::types::PackageType::Service => "Service",
        crate::pkg::types::PackageType::Config => "Config",
        crate::pkg::types::PackageType::App => "App",
        crate::pkg::types::PackageType::Extension => "Extension",
        crate::pkg::types::PackageType::Library => "Library",
        crate::pkg::types::PackageType::Script => "Script",
    };
    println!("{}: {}", "Type".bold(), type_display);

    let scope_display = match pkg.scope {
        crate::pkg::types::Scope::User => "User",
        crate::pkg::types::Scope::System => "System",
        crate::pkg::types::Scope::Project => "Project",
    };
    println!("{}: {}", "Scope".bold(), scope_display);

    if !pkg.tags.is_empty() {
        println!("{}: {}", "Tags".bold(), pkg.tags.join(", "));
    }

    if let Some(bins) = &pkg.bins
        && !bins.is_empty()
    {
        println!("{}: {}", "Provides".bold(), bins.join(", ").green());
    }

    if let Some(conflicts) = &pkg.conflicts
        && !conflicts.is_empty()
    {
        println!("{}: {}", "Conflicts".bold(), conflicts.join(", ").red());
    }

    if pkg.package_type == crate::pkg::types::PackageType::Package {
        println!("{}: {}", "Available types".bold(), pkg.types.join(", "));
    }

    if let Some(deps) = &pkg.dependencies {
        println!("{}:", "Dependencies".bold());

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
