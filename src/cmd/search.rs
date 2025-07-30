use crate::pkg::{local, types::PackageType};
use colored::*;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};

pub fn run(search_term: String, repo: Option<String>, package_type: Option<String>) {
    println!(
        "{}{}{}",
        "--- Searching for packages matching '".yellow(),
        search_term.blue().bold(),
        "' ---".yellow()
    );

    let packages = if let Some(repo_name) = &repo {
        local::get_packages_from_repo(repo_name)
    } else {
        local::get_all_available_packages()
    };

    match packages {
        Ok(all_packages) => {
            let search_term_lower = search_term.to_lowercase();

            let type_filter = package_type.and_then(|s| match s.to_lowercase().as_str() {
                "package" => Some(PackageType::Package),
                "collection" => Some(PackageType::Collection),
                "service" => Some(PackageType::Service),
                "config" => Some(PackageType::Config),
                _ => None,
            });

            let matches: Vec<_> = all_packages
                .into_iter()
                .filter(|pkg| {
                    if let Some(ptype) = type_filter {
                        if pkg.package_type != ptype {
                            return false;
                        }
                    }

                    let name_match = pkg.name.to_lowercase().contains(&search_term_lower);
                    let description_match =
                        pkg.description.to_lowercase().contains(&search_term_lower);
                    name_match || description_match
                })
                .collect();

            if matches.is_empty() {
                println!("\n{}", "No packages found matching your query.".yellow());
                return;
            }

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec!["Package", "Version", "Repo", "Description"]);

            for pkg in matches {
                let mut desc = pkg.description.replace('\n', " ");
                if desc.len() > 60 {
                    desc.truncate(57);
                    desc.push_str("...");
                }

                let version =
                    crate::pkg::resolve::get_default_version(&pkg).unwrap_or_else(|_| "N/A".to_string());
                table.add_row(vec![pkg.name, version, pkg.repo, desc]);
            }

            println!("{table}");
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
        }
    }
}