use crate::pkg::local;
use colored::*;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};

pub fn run(args: Vec<String>) {
    if args.is_empty() {
        eprintln!("{}: {}", "Error".red(), "Please provide a search term.");
        return;
    }

    let mut search_term = "";
    let mut repo_filter: Option<String> = None;

    for arg in &args {
        if arg.starts_with('@') {
            repo_filter = Some(arg.strip_prefix('@').unwrap().to_string());
        } else {
            search_term = arg;
        }
    }

    if search_term.is_empty() {
        eprintln!("{}: {}", "Error".red(), "Please provide a search term.");
        return;
    }

    println!(
        "{}{}{}",
        "--- Searching for packages matching '".yellow(),
        search_term.blue().bold(),
        "' ---".yellow()
    );

    let packages = if let Some(repo) = &repo_filter {
        local::get_packages_from_repo(repo)
    } else {
        local::get_all_available_packages()
    };

    match packages {
        Ok(all_packages) => {
            let search_term_lower = search_term.to_lowercase();

            let matches: Vec<_> = all_packages
                .into_iter()
                .filter(|pkg| {
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

                table.add_row(vec![pkg.name, pkg.version, pkg.repo, desc]);
            }

            println!("{table}");
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
        }
    }
}