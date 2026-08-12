//! Searching for packages that provide a specific file or command.

use anyhow::Result;
use colored::Colorize;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, ContentArrangement, Table};
use rayon::prelude::*;

use crate::pkg::{config, db};

/// Searches for packages that provide a specific file or command.
///
/// This queries the configured registries to find which packages contain the
/// given term in their file lists.
///
/// # Errors
///
/// This function will return an error if:
/// - It fails to read the Zoi configuration.
/// - It fails to connect to the package database or execute the query.
/// # Errors
///
/// Returns an error if the provides search fails.
pub fn run(term: &str) -> Result<()> {
    println!(
        "{} Searching for packages providing '{}'...",
        "::".bold().blue(),
        term.cyan().bold()
    );

    let config = config::read_config()?;
    let mut registries = Vec::new();
    if let Some(default) = &config.default_registry {
        registries.push(default.handle.clone());
    }
    for reg in &config.added_registries {
        registries.push(reg.handle.clone());
    }

    let all_results: Vec<(crate::pkg::types::Package, String)> = registries
        .into_par_iter()
        .filter_map(|handle| db::find_provides(&handle, term).ok())
        .flatten()
        .collect();

    if all_results.is_empty() {
        println!(
            "\n{} No packages found providing this item.",
            "::".bold().yellow()
        );
        println!(
            "   {} Ensure you have run 'zoi sync --files' to index remote \
             file lists.",
            "Hint:".cyan()
        );
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_style(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Package").add_attribute(Attribute::Bold),
            Cell::new("Version").add_attribute(Attribute::Bold),
            Cell::new("Matches").add_attribute(Attribute::Bold),
            Cell::new("Repo").add_attribute(Attribute::Bold),
        ]);

    for (pkg, matched_path) in all_results {
        let repo_display = &pkg.repo;
        table.add_row(vec![
            Cell::new(pkg.name).fg(comfy_table::Color::Cyan),
            Cell::new(pkg.version.unwrap_or_else(|| "N/A".to_string()))
                .fg(comfy_table::Color::Yellow),
            Cell::new(matched_path).fg(comfy_table::Color::Green),
            Cell::new(repo_display.clone()).fg(comfy_table::Color::DarkGrey),
        ]);
    }

    println!("{table}");

    Ok(())
}
