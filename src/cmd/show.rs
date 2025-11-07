use crate::pkg::{local, resolve, types};
use crate::utils;
use colored::*;
use std::fs;

fn print_dependency_group(group: &types::DependencyGroup, indent: usize) {
    let prefix = " ".repeat(indent * 2);
    let mut count = 0;

    let required = group.get_required_simple();
    if !required.is_empty() {
        for dep in required {
            println!("{}- {} (required)", prefix, dep);
            count += 1;
        }
    }

    let options = group.get_required_options();
    if !options.is_empty() {
        for opt_group in options {
            println!(
                "{}{}: {} (choose {})",
                prefix,
                opt_group.name.bold(),
                opt_group.desc,
                if opt_group.all { "any" } else { "one" }
            );
            for dep in &opt_group.depends {
                println!("{}  - {}", prefix, dep);
                count += 1;
            }
        }
    }

    let optional = group.get_optional();
    if !optional.is_empty() {
        for dep in optional {
            println!("{}- {} (optional)", prefix, dep);
            count += 1;
        }
    }

    if count == 0 {
        println!("{}- {}", prefix, "None".italic());
    }
}

pub fn run(source: &str, raw: bool) {
    let source = source.trim();
    match resolve::resolve_source(source, false) {
        Ok(resolved_source) => {
            if raw {
                let content = fs::read_to_string(&resolved_source.path).unwrap();
                println!("{content}");
                return;
            }
            let mut pkg: types::Package = crate::pkg::lua::parser::parse_lua_package(
                resolved_source.path.to_str().unwrap(),
                None,
                false,
            )
            .unwrap();
            if let Some(repo_name) = resolved_source.repo_name {
                pkg.repo = repo_name;
            }
            pkg.version = Some(
                resolve::get_default_version(&pkg, resolved_source.registry_handle.as_deref())
                    .unwrap_or_else(|_| "N/A".to_string()),
            );

            let request = resolve::parse_source_string(source).unwrap();

            let installed_manifest = match local::is_package_installed(
                &pkg.name,
                request.sub_package.as_deref(),
                pkg.scope,
            ) {
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

fn print_beautiful(
    pkg: &crate::pkg::types::Package,
    installed_manifest: Option<&types::InstallManifest>,
) {
    println!(
        "{} {} - {}",
        pkg.name.bold().green(),
        pkg.version.as_deref().unwrap_or_default().dimmed(),
        pkg.repo
    );
    if let Some(website) = &pkg.website {
        println!("Website: {}", website.cyan().underline());
    }
    if !pkg.git.is_empty() {
        println!("Git Repo: {}", pkg.git.cyan().underline());
    }
    println!("{}", pkg.description);

    if let Some(subs) = &pkg.sub_packages {
        println!("{}: {}", "Sub-packages".bold(), subs.join(", "));
        if let Some(main_subs) = &pkg.main_subs {
            println!("{}: {}", "Main sub-packages".bold(), main_subs.join(", "));
        }
    }

    if let Some(manifest) = installed_manifest {
        let status_text = if let Some(sub) = &manifest.sub_package {
            format!("Installed ({})", sub)
        } else {
            "Installed".to_string()
        };
        println!(
            "{}: {} ({})",
            "Status".bold(),
            status_text.green(),
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
        crate::pkg::types::PackageType::App => "App",
        crate::pkg::types::PackageType::Extension => "Extension",
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
        println!("\n{}:", "Dependencies".bold());

        if let Some(runtime) = &deps.runtime {
            println!("  Runtime:");
            print_dependency_group(runtime, 2);
        }

        if let Some(build_deps) = &deps.build {
            println!("  Build Dependencies:");
            match build_deps {
                types::BuildDependencies::Group(group) => {
                    print_dependency_group(group, 2);
                }
                types::BuildDependencies::Typed(typed_build_deps) => {
                    for (name, group) in &typed_build_deps.types {
                        println!("    {}:", name.cyan());
                        print_dependency_group(group, 3);
                    }
                }
            }
        }
    }
}
