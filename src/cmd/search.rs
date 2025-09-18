use crate::pkg::{local, types::PackageType};
use colored::*;
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL};
use std::io::{self, Write};
use std::process::{Command, Stdio};

fn print_with_pager(content: &str) -> io::Result<()> {
    let pager = if crate::utils::command_exists("less") {
        "less"
    } else if crate::utils::command_exists("more") {
        "more"
    } else {
        print!("{}", content);
        return Ok(());
    };

    let mut command = Command::new(pager);
    if pager == "less" {
        command.arg("-R");
    }

    let mut child = command.stdin(Stdio::piped()).spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(content.as_bytes());
    }

    child.wait()?;
    Ok(())
}

pub fn run(
    search_term: String,
    repo: Option<String>,
    package_type: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}{}{}",
        "--- Searching for packages matching '".yellow(),
        search_term.blue().bold(),
        "' ---".yellow()
    );

    let packages = if let Some(repo_name) = &repo {
        let all_repos = crate::pkg::config::get_all_repos()?;
        let repos_to_search: Vec<String> = all_repos
            .into_iter()
            .filter(|r| r.starts_with(repo_name))
            .collect();
        local::get_packages_from_repos(&repos_to_search)
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
                "app" => Some(PackageType::App),
                "extension" => Some(PackageType::Extension),
                "library" => Some(PackageType::Library),
                "script" => Some(PackageType::Script),
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
                    if let Some(ptype) = type_filter
                        && pkg.package_type != ptype
                    {
                        return false;
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
                return Ok(());
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

                let mut parts = pkg.repo.splitn(2, '/');
                let repo_display = parts.next().unwrap_or(&pkg.repo);

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
                table.add_row(vec![
                    pkg.name,
                    version,
                    repo_display.to_string(),
                    tags_display,
                    desc,
                ]);
            }

            print_with_pager(&table.to_string())?;
        }
        Err(e) => {
            return Err(e);
        }
    }
    Ok(())
}
