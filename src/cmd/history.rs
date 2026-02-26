use crate::pkg::audit;
use anyhow::Result;
use colored::*;
use comfy_table::{Cell, Color, ContentArrangement, Table, presets::UTF8_FULL};

pub fn run() -> Result<()> {
    println!("{}", "--- Zoi Operation History ---".yellow().bold());

    let history = audit::get_history()?;

    if history.is_empty() {
        println!("No history recorded. Audit logging might be disabled.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Date/Time",
            "User",
            "Action",
            "Package",
            "Version",
            "Repo",
            "Type",
            "Scope",
        ]);

    for entry in history {
        let action_cell = match entry.action {
            audit::AuditAction::Install => Cell::new("Install").fg(Color::Green),
            audit::AuditAction::Uninstall => Cell::new("Uninstall").fg(Color::Red),
            audit::AuditAction::Upgrade => Cell::new("Upgrade").fg(Color::Yellow),
        };

        table.add_row(vec![
            Cell::new(
                entry
                    .timestamp
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S"),
            ),
            Cell::new(entry.user),
            action_cell,
            Cell::new(entry.package_name).fg(Color::Cyan),
            Cell::new(entry.version),
            Cell::new(entry.repo),
            Cell::new(format!("{:?}", entry.package_type)),
            Cell::new(format!("{:?}", entry.scope)),
        ]);
    }

    println!("{table}");

    Ok(())
}
