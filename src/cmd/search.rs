use crate::pkg::{local, types::PackageType};
use colored::*;
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL};

pub fn run(
    search_term: String,
    repo: Option<String>,
    package_type: Option<String>,
    tags: Option<Vec<String>>,
) {
    println!(
        "{}{}{}",
        "--- Searching for packages matching '".yellow(),
        search_term.blue().bold(),
        "' ---".yellow()
    );

    let packages = local::get_all_available_packages();

    match packages {
        Ok(mut all_packages) => {
            if let Some(repo_name) = &repo {
                all_packages.retain(|pkg| pkg.repo.starts_with(repo_name));
            }

            let search_term_lower = search_term.to_lowercase();

            let type_filter = package_type.and_then(|s| match s.to_lowercase().as_str() {
                "package" => Some(PackageType::Package),
                "collection" => Some(PackageType::Collection),
                "service" => Some(PackageType::Service),
                "config" => Some(PackageType::Config),
                _ => None,
            });

            let wanted_tags: Vec<String> = tags
                .unwrap_or_default()
                .into_iter()
                .map(|t| t.to_lowercase())
                .collect();

            let matches: Vec<_> = all_packages
                .into_iter()
                .filter(|pkg| {
                    if let Some(ptype) = type_filter {
                        if pkg.package_type != ptype {
                            return false;
                        }
                    }

                    if !wanted_tags.is_empty() {
                        if pkg.tags.is_empty() {
                            return false;
                        }
                        let pkg_tags_lower: Vec<String> =
                            pkg.tags.iter().map(|t| t.to_lowercase()).collect();
                        let has_any = wanted_tags
                            .iter()
                            .any(|wanted| pkg_tags_lower.iter().any(|pt| pt == wanted));
                        if !has_any {
                            return false;
                        }
                    }

                    let name_match = pkg.name.to_lowercase().contains(&search_term_lower);
                    let description_match =
                        pkg.description.to_lowercase().contains(&search_term_lower);
                    let tags_match = if pkg.tags.is_empty() {
                        false
                    } else {
                        pkg.tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(&search_term_lower))
                    };
                    name_match || description_match || tags_match
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
                .set_header(vec!["Package", "Version", "Repo", "Tags", "Description"]);

            for pkg in matches {
                let mut desc = pkg.description.replace('\n', " ");
                if desc.len() > 60 {
                    desc.truncate(57);
                    desc.push_str("...");
                }

                let version = crate::pkg::resolve::get_default_version(&pkg)
                    .unwrap_or_else(|_| "N/A".to_string());
                let tags_display = if pkg.tags.is_empty() {
                    String::from("")
                } else {
                    let mut tags = pkg.tags.clone();
                    tags.sort();
                    if tags.len() > 4 {
                        format!("{}…", tags[..4].join(", "))
                    } else {
                        tags.join(", ")
                    }
                };
                table.add_row(vec![pkg.name, version, pkg.repo, tags_display, desc]);
            }

            println!("{}", table);
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
        }
    }
}
