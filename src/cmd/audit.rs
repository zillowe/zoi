use crate::pkg::{config, db, local, types};
use anyhow::Result;
use colored::*;
use comfy_table::{Attribute, Cell, ContentArrangement, Table, presets::UTF8_FULL};
use semver::{Version, VersionReq};

pub fn run(all: bool, registry_filter: Option<String>, repo_filter: Option<String>) -> Result<()> {
    if !all {
        println!(
            "{} Auditing installed packages for vulnerabilities...",
            "::".bold().blue()
        );
    } else {
        println!(
            "{} Listing all known vulnerabilities...",
            "::".bold().blue()
        );
    }

    let config = config::read_config()?;
    let mut registries = Vec::new();
    if let Some(reg) = registry_filter {
        registries.push(reg);
    } else {
        if let Some(default) = &config.default_registry {
            registries.push(default.handle.clone());
        }
        for reg in &config.added_registries {
            registries.push(reg.handle.clone());
        }
    }

    let mut all_advisories = Vec::new();
    for handle in registries {
        if let Ok(advisories) = db::list_all_advisories(&handle) {
            for (adv, repo) in advisories {
                all_advisories.push((adv, repo, handle.clone()));
            }
        }
    }

    if let Some(rf) = &repo_filter {
        all_advisories.retain(|(_, repo, _)| {
            if rf.contains('/') {
                repo == rf
            } else {
                repo.split('/').any(|part| part == rf)
            }
        });
    }

    if all_advisories.is_empty() {
        println!(
            "\n{}",
            "No vulnerabilities found matching your criteria.".green()
        );
        return Ok(());
    }

    if all {
        print_advisories_table(all_advisories)?;
    } else {
        let installed = local::get_installed_packages()?;
        let mut vulnerable_installed = Vec::new();

        for manifest in installed {
            for (adv, repo, reg) in &all_advisories {
                if adv.package == manifest.name
                    && *repo == manifest.repo
                    && *reg == manifest.registry_handle
                    && let Ok(version) = Version::parse(&manifest.version)
                    && let Ok(req) = VersionReq::parse(&adv.affected_range)
                    && req.matches(&version)
                {
                    vulnerable_installed.push((adv.clone(), manifest.clone()));
                }
            }
        }

        if vulnerable_installed.is_empty() {
            println!(
                "\n{}",
                "No vulnerabilities found in installed packages.".green()
            );
        } else {
            println!(
                "\n{} Found {} vulnerabilities in installed packages:",
                "Warning".red().bold(),
                vulnerable_installed.len()
            );
            print_vulnerable_table(vulnerable_installed)?;
        }
    }

    Ok(())
}

fn print_advisories_table(advisories: Vec<(types::Advisory, String, String)>) -> Result<()> {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("Package").add_attribute(Attribute::Bold),
            Cell::new("Severity").add_attribute(Attribute::Bold),
            Cell::new("Affected").add_attribute(Attribute::Bold),
            Cell::new("Fixed In").add_attribute(Attribute::Bold),
            Cell::new("Summary").add_attribute(Attribute::Bold),
        ]);

    for (adv, _, _) in advisories {
        let severity_cell = match adv.severity {
            types::Severity::Low => Cell::new("Low").fg(comfy_table::Color::Blue),
            types::Severity::Medium => Cell::new("Medium").fg(comfy_table::Color::Yellow),
            types::Severity::High => Cell::new("High").fg(comfy_table::Color::Red),
            types::Severity::Critical => Cell::new("Critical")
                .fg(comfy_table::Color::Magenta)
                .add_attribute(Attribute::Bold),
        };

        table.add_row(vec![
            Cell::new(adv.id).fg(comfy_table::Color::Cyan),
            Cell::new(adv.package),
            severity_cell,
            Cell::new(adv.affected_range),
            Cell::new(adv.fixed_in.unwrap_or_else(|| "N/A".to_string()))
                .fg(comfy_table::Color::Green),
            Cell::new(adv.summary),
        ]);
    }

    println!("{table}");
    Ok(())
}

fn print_vulnerable_table(
    vulnerable: Vec<(types::Advisory, types::InstallManifest)>,
) -> Result<()> {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Package").add_attribute(Attribute::Bold),
            Cell::new("Installed").add_attribute(Attribute::Bold),
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("Severity").add_attribute(Attribute::Bold),
            Cell::new("Fixed In").add_attribute(Attribute::Bold),
            Cell::new("Summary").add_attribute(Attribute::Bold),
        ]);

    for (adv, manifest) in vulnerable {
        let severity_cell = match adv.severity {
            types::Severity::Low => Cell::new("Low").fg(comfy_table::Color::Blue),
            types::Severity::Medium => Cell::new("Medium").fg(comfy_table::Color::Yellow),
            types::Severity::High => Cell::new("High").fg(comfy_table::Color::Red),
            types::Severity::Critical => Cell::new("Critical")
                .fg(comfy_table::Color::Magenta)
                .add_attribute(Attribute::Bold),
        };

        table.add_row(vec![
            Cell::new(manifest.name).fg(comfy_table::Color::Cyan),
            Cell::new(manifest.version).fg(comfy_table::Color::Red),
            Cell::new(adv.id).fg(comfy_table::Color::DarkGrey),
            severity_cell,
            Cell::new(adv.fixed_in.unwrap_or_else(|| "N/A".to_string()))
                .fg(comfy_table::Color::Green),
            Cell::new(adv.summary),
        ]);
    }

    println!("{table}");
    Ok(())
}
