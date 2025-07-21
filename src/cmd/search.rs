use crate::pkg::local;
use colored::*;
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL};

pub fn run(search_term: &str) {
    println!(
        "{}{}{}",
        "--- Searching for packages matching '".yellow(),
        search_term.blue().bold(),
        "' ---".yellow()
    );

    match local::get_all_available_packages() {
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
                .set_header(vec!["Package", "Version", "Description"]);

            for pkg in matches {
                let mut desc = pkg.description.replace('\n', " ");
                if desc.len() > 70 {
                    desc.truncate(67);
                    desc.push_str("...");
                }

                table.add_row(vec![pkg.name.green().to_string(), pkg.version, desc]);
            }

            println!("{table}");
        }
        Err(e) => {
            eprintln!("\n{}: {}", "Error".red().bold(), e);
        }
    }
}
