use crate::pkg::{config, db};
use anyhow::Result;
use colored::Colorize;
use comfy_table::{Attribute, Cell, ContentArrangement, Table, presets::UTF8_FULL};
use std::io::{self, Write};
use std::process::{Command, Stdio};

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

    let mut all_results = Vec::new();
    for handle in registries {
        if let Ok(res) = db::find_provides(&handle, term) {
            all_results.extend(res);
        }
    }

    if all_results.is_empty() {
        println!(
            "
{}",
            "No packages found providing this item.".yellow()
        );
        println!("Hint: Ensure you have run 'zoi sync --files' to index remote file lists.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Package").add_attribute(Attribute::Bold),
            Cell::new("Version").add_attribute(Attribute::Bold),
            Cell::new("Matches").add_attribute(Attribute::Bold),
            Cell::new("Repo").add_attribute(Attribute::Bold),
        ]);

    for (pkg, matched_path) in all_results {
        let repo_display = pkg.repo.split_once('/').map(|x| x.1).unwrap_or(&pkg.repo);
        table.add_row(vec![
            Cell::new(pkg.name).fg(comfy_table::Color::Cyan),
            Cell::new(pkg.version.unwrap_or_else(|| "N/A".to_string()))
                .fg(comfy_table::Color::Yellow),
            Cell::new(matched_path).fg(comfy_table::Color::Green),
            Cell::new(repo_display).fg(comfy_table::Color::DarkGrey),
        ]);
    }

    print_with_pager(&table.to_string())?;

    Ok(())
}

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
