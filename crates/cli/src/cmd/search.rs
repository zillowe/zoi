//! Command for searching packages or files in registries.
//!
//! This module provides functionality to search for packages by name,
//! description, or tags, as well as searching for specific files
//! within packages. It supports both a standard table output and
//! an interactive TUI.

use std::io::{self};

use anyhow::{Result, anyhow};
use colored::Colorize;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, ContentArrangement, Table};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode,
    KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color as RatatuiColor, Modifier, Style as RatatuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, List, ListItem, ListState, Paragraph, Wrap,
};
use rayon::prelude::*;

use crate::pkg::types::{Package, PackageType};
use crate::pkg::{config, local};

/// Runs the search command with the provided terms and options.
///
/// # Errors
///
/// Returns an error if:
/// - File search fails.
/// - Configuration reading fails.
/// - Database search fails.
/// - Package resolution fails.
///
/// # Panics
///
/// Panics if the interactive TUI fails to initialize or if layout calculations
/// fail. # Errors
///
/// Returns an error if the search operation fails.
pub fn run(
    search_term: &str,
    registry_filter: Option<&str,>,
    repo: Option<&str,>,
    package_type: Option<&str,>,
    tags: Option<Vec<String,>,>,
    sort_by: &str,
    files: bool,
    interactive: bool,
) -> Result<(),> {
    if !interactive {
        let mode = if files { "files" } else { "packages" };
        println!(
            "{} Searching for {} matching '{}'...",
            "::".bold().blue(),
            mode.yellow(),
            search_term.cyan().bold()
        );
    }

    if files {
        if interactive {
            return Err(anyhow!(
                "Interactive mode is not supported for file search."
            ),);
        }
        return run_file_search(
            search_term,
            registry_filter,
            repo,
            package_type,
        );
    }

    let config = config::read_config()?;

    let mut all_packages = Vec::new();
    let mut db_failed = false;

    if let Some(reg_handle,) = registry_filter {
        match crate::pkg::db::search_packages(reg_handle, search_term,) {
            Ok(pkgs,) => all_packages.extend(pkgs,),
            Err(_,) => db_failed = true,
        }
    } else {
        let mut registries = Vec::new();
        if let Some(default,) = &config.default_registry {
            registries.push(default.handle.clone(),);
        }
        for reg in &config.added_registries {
            registries.push(reg.handle.clone(),);
        }

        let results: Vec<Result<Vec<Package,>,>,> = registries
            .into_par_iter()
            .map(
                |handle| crate::pkg::db::search_packages(&handle, search_term,),
            )
            .collect();

        for res in results {
            if let Ok(pkgs,) = res {
                all_packages.extend(pkgs,)
            } else {
                db_failed = true;
                break;
            }
        }
    }

    let packages = if db_failed
        || (all_packages.is_empty() && registry_filter.is_none())
    {
        if let Some(reg_handle,) = registry_filter {
            let all_repo_names = config::get_all_repos()?;
            let full_repos: Vec<String,> = all_repo_names
                .into_iter()
                .map(|r_name| format!("{reg_handle}/{r_name}"),)
                .filter(|full_repo_name| {
                    if let Some(repo_f,) = repo {
                        if repo_f.contains('/',) {
                            full_repo_name == repo_f
                        } else {
                            full_repo_name
                                .split('/',)
                                .any(|part| part == repo_f,)
                        }
                    } else {
                        true
                    }
                },)
                .collect();
            local::get_packages_from_repos(&full_repos,)
        } else if let Some(repo_filter,) = repo {
            let handle = if let Some(reg,) = &config.default_registry {
                reg.handle.clone()
            } else {
                return Err(anyhow!("Default registry not configured."),);
            };
            if handle.is_empty() {
                return Err(anyhow!(
                    "Default registry handle is not set. Please run 'zoi \
                     sync'.."
                ),);
            }
            let all_repo_names = config::get_all_repos()?;
            let repos_to_search: Vec<String,> = all_repo_names
                .into_iter()
                .map(|r_name| format!("{handle}/{r_name}"),)
                .filter(|full_repo_name| {
                    if repo_filter.contains('/',) {
                        full_repo_name == repo_filter
                    } else {
                        full_repo_name
                            .split('/',)
                            .any(|part| part == repo_filter,)
                    }
                },)
                .collect();
            local::get_packages_from_repos(&repos_to_search,)
        } else {
            local::get_all_available_packages()
        }
    } else {
        Ok(all_packages,)
    };

    let handle_for_version = registry_filter.or(config
        .default_registry
        .as_ref()
        .map(|reg| reg.handle.as_str(),),);

    match packages {
        Ok(all_packages,) => {
            let search_term_lower = search_term.to_lowercase();

            let type_filter =
                package_type.and_then(|s| match s.to_lowercase().as_str() {
                    "package" => Some(PackageType::Package,),
                    "collection" => Some(PackageType::Collection,),
                    "app" => Some(PackageType::App,),
                    "extension" => Some(PackageType::Extension,),
                    _ => None,
                },);

            let wanted_tags: Vec<String,> = tags
                .unwrap_or_default()
                .into_iter()
                .map(|t| t.to_lowercase(),)
                .collect();

            let mut matches: Vec<_,> = all_packages
                .into_iter()
                .filter(|pkg| {
                    if let Some(ptype,) = type_filter
                        && pkg.package_type != ptype
                    {
                        return false;
                    }

                    if !wanted_tags.is_empty() {
                        if pkg.tags.is_empty() {
                            return false;
                        }
                        let pkg_tags_lower: Vec<String,> = pkg
                            .tags
                            .iter()
                            .map(|t| t.to_lowercase(),)
                            .collect();
                        let has_any = wanted_tags.iter().any(|wanted| {
                            pkg_tags_lower.iter().any(|pt| pt == wanted,)
                        },);
                        if !has_any {
                            return false;
                        }
                    }

                    let name_match =
                        pkg.name.to_lowercase().contains(&search_term_lower,);
                    let description_match = pkg
                        .description
                        .to_lowercase()
                        .contains(&search_term_lower,);
                    let tags_match = if pkg.tags.is_empty() {
                        false
                    } else {
                        pkg.tags.iter().any(|t| {
                            t.to_lowercase().contains(&search_term_lower,)
                        },)
                    };
                    name_match || description_match || tags_match
                },)
                .collect();

            if matches.is_empty() {
                if !interactive {
                    println!(
                        "\n{} No packages found matching your query.",
                        "::".bold().yellow()
                    );
                }
                return Ok((),);
            }

            match sort_by {
                "name" => matches.sort_by(|a, b| a.name.cmp(&b.name,),),
                "repo" => matches.sort_by(|a, b| a.repo.cmp(&b.repo,),),
                "type" => matches.sort_by(|a, b| {
                    format!("{:?}", a.package_type)
                        .cmp(&format!("{:?}", b.package_type),)
                },),
                _ => matches.sort_by(|a, b| a.name.cmp(&b.name,),),
            }

            if interactive {
                return run_tui(&matches, handle_for_version,);
            }

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL,)
                .set_content_arrangement(ContentArrangement::Dynamic,)
                .set_header(vec![
                    Cell::new("Package",).add_attribute(Attribute::Bold,),
                    Cell::new("Version",).add_attribute(Attribute::Bold,),
                    Cell::new("Repo",).add_attribute(Attribute::Bold,),
                    Cell::new("License",).add_attribute(Attribute::Bold,),
                    Cell::new("Tags",).add_attribute(Attribute::Bold,),
                    Cell::new("Description",).add_attribute(Attribute::Bold,),
                ],);

            for pkg in matches {
                let mut desc = pkg.description.replace('\n', " ",);
                if desc.len() > 60 {
                    desc.truncate(57,);
                    desc.push_str("...",);
                }

                let version = crate::pkg::resolve::get_default_version(
                    &pkg,
                    handle_for_version,
                )
                .unwrap_or_else(|_| "N/A".to_string(),);

                let version_display = if pkg.revision == "1" {
                    version.clone()
                } else {
                    format!("{}-{}", version, pkg.revision)
                };

                let repo_display = &pkg.repo;

                let tags_display = if pkg.tags.is_empty() {
                    String::new()
                } else {
                    let mut tags = pkg.tags.clone();
                    tags.sort();
                    if tags.len() > 4 {
                        let first_four: Vec<_,> = tags
                            .iter()
                            .take(4,)
                            .map(std::string::String::as_str,)
                            .collect();
                        format!("{}…", first_four.join(", "))
                    } else {
                        tags.join(", ",)
                    }
                };

                table.add_row(vec![
                    Cell::new(pkg.name,).fg(comfy_table::Color::Cyan,),
                    Cell::new(version_display,).fg(comfy_table::Color::Yellow,),
                    Cell::new(repo_display,).fg(comfy_table::Color::Green,),
                    Cell::new(pkg.license,),
                    Cell::new(tags_display,).fg(comfy_table::Color::DarkGrey,),
                    Cell::new(desc,),
                ],);
            }

            println!("{table}");
        }
        Err(e,) => {
            return Err(e,);
        }
    }
    Ok((),)
}

/// Searches for files matching the given term.
fn run_file_search(
    term: &str,
    registry_filter: Option<&str,>,
    repo: Option<&str,>,
    package_type: Option<&str,>,
) -> Result<(),> {
    let config = config::read_config()?;
    let mut registries = Vec::new();
    if let Some(reg,) = registry_filter {
        registries.push(reg.to_string(),);
    } else {
        if let Some(default,) = &config.default_registry {
            registries.push(default.handle.clone(),);
        }
        for reg in &config.added_registries {
            registries.push(reg.handle.clone(),);
        }
    }

    let mut results = Vec::new();
    for handle in registries {
        if let Ok(res,) = crate::pkg::db::search_files(&handle, term,) {
            results.extend(res,);
        }
    }

    let type_filter =
        package_type.and_then(|s| match s.to_lowercase().as_str() {
            "package" => Some(PackageType::Package,),
            "collection" => Some(PackageType::Collection,),
            "app" => Some(PackageType::App,),
            "extension" => Some(PackageType::Extension,),
            _ => None,
        },);

    results.retain(|(pkg, _,)| {
        if let Some(pt,) = type_filter
            && pkg.package_type != pt
        {
            return false;
        }
        if let Some(rf,) = &repo {
            if rf.contains('/',) {
                if pkg.repo != *rf {
                    return false;
                }
            } else if !pkg.repo.split('/',).any(|part| part == *rf,) {
                return false;
            }
        }
        true
    },);

    if results.is_empty() {
        println!("\n{}", "No files found matching your query.".yellow());
        println!(
            "Hint: Ensure you have run 'zoi sync --files' to index remote \
             file lists."
        );
        return Ok((),);
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL,)
        .set_content_arrangement(ContentArrangement::Dynamic,)
        .set_header(vec![
            Cell::new("Package",).add_attribute(Attribute::Bold,),
            Cell::new("File Path",).add_attribute(Attribute::Bold,),
            Cell::new("Repo",).add_attribute(Attribute::Bold,),
        ],);

    for (pkg, path,) in results {
        let repo_display = &pkg.repo;
        table.add_row(vec![
            Cell::new(pkg.name,).fg(comfy_table::Color::Cyan,),
            Cell::new(path,).fg(comfy_table::Color::Yellow,),
            Cell::new(repo_display.clone(),).fg(comfy_table::Color::Green,),
        ],);
    }

    println!("{table}");
    Ok((),)
}

/// Runs the interactive TUI for package search results.
fn run_tui(
    packages: &[Package],
    handle_for_version: Option<&str,>,
) -> Result<(),> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout,);
    let mut terminal = Terminal::new(backend,)?;

    let mut state = ListState::default();
    state.select(Some(0,),);

    let res =
        run_tui_loop(&mut terminal, packages, &mut state, handle_for_version,);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err,) = res {
        eprintln!("{err:?}");
    }

    Ok((),)
}

/// Main loop for the search TUI.
fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout,>,>,
    packages: &[Package],
    state: &mut ListState,
    handle_for_version: Option<&str,>,
) -> Result<(),> {
    loop {
        terminal.draw(|f| {
            let layout = Layout::default()
                .direction(Direction::Vertical,)
                .constraints([Constraint::Min(0,), Constraint::Length(1,),],)
                .split(f.area(),);

            let chunks = Layout::default()
                .direction(Direction::Horizontal,)
                .constraints([
                    Constraint::Percentage(30,),
                    Constraint::Percentage(70,),
                ],)
                .split(
                    *layout
                        .first()
                        .expect("layout should have at least one chunk",),
                );

            let items: Vec<ListItem,> = packages
                .iter()
                .map(|p| {
                    ListItem::new(Line::from(vec![Span::styled(
                        p.name.clone(),
                        RatatuiStyle::default().fg(RatatuiColor::Cyan,),
                    )],),)
                },)
                .collect();

            let list = List::new(items,)
                .block(
                    Block::default().borders(Borders::ALL,).title("Packages",),
                )
                .highlight_style(
                    RatatuiStyle::default()
                        .bg(RatatuiColor::DarkGray,)
                        .add_modifier(Modifier::BOLD,),
                )
                .highlight_symbol(">> ",);

            f.render_stateful_widget(
                list,
                *chunks
                    .first()
                    .expect("chunks should have at least one item",),
                state,
            );

            if let Some(selected,) = state.selected() {
                let pkg = packages
                    .get(selected,)
                    .expect("selected index should be within bounds",);
                let version = crate::pkg::resolve::get_default_version(
                    pkg,
                    handle_for_version,
                )
                .unwrap_or_else(|_| "N/A".to_string(),);

                let version_display = if pkg.revision == "1" {
                    version
                } else {
                    format!("{}-{}", version, pkg.revision)
                };

                let details = vec![
                    Line::from(vec![
                        Span::styled(
                            "Name: ",
                            RatatuiStyle::default()
                                .add_modifier(Modifier::BOLD,),
                        ),
                        Span::styled(
                            pkg.name.clone(),
                            RatatuiStyle::default().fg(RatatuiColor::Cyan,),
                        ),
                    ],),
                    Line::from(vec![
                        Span::styled(
                            "Version: ",
                            RatatuiStyle::default()
                                .add_modifier(Modifier::BOLD,),
                        ),
                        Span::styled(
                            version_display,
                            RatatuiStyle::default().fg(RatatuiColor::Yellow,),
                        ),
                    ],),
                    Line::from(vec![
                        Span::styled(
                            "Repo: ",
                            RatatuiStyle::default()
                                .add_modifier(Modifier::BOLD,),
                        ),
                        Span::styled(
                            pkg.repo.clone(),
                            RatatuiStyle::default().fg(RatatuiColor::Green,),
                        ),
                    ],),
                    Line::from(vec![
                        Span::styled(
                            "License: ",
                            RatatuiStyle::default()
                                .add_modifier(Modifier::BOLD,),
                        ),
                        Span::raw(pkg.license.clone(),),
                    ],),
                    Line::from(vec![
                        Span::styled(
                            "Type: ",
                            RatatuiStyle::default()
                                .add_modifier(Modifier::BOLD,),
                        ),
                        Span::raw(format!("{:?}", pkg.package_type),),
                    ],),
                    Line::from(vec![
                        Span::styled(
                            "Tags: ",
                            RatatuiStyle::default()
                                .add_modifier(Modifier::BOLD,),
                        ),
                        Span::raw(pkg.tags.join(", ",),),
                    ],),
                    Line::from("",),
                    Line::from(Span::styled(
                        "Description:",
                        RatatuiStyle::default().add_modifier(Modifier::BOLD,),
                    ),),
                    Line::from(pkg.description.clone(),),
                ];

                let details_paragraph = Paragraph::new(details,)
                    .block(
                        Block::default()
                            .borders(Borders::ALL,)
                            .title("Details",),
                    )
                    .wrap(Wrap { trim: true, },);

                f.render_widget(
                    details_paragraph,
                    *chunks.get(1,).expect("chunks should have a second item",),
                );
            }

            let help_text = Line::from(vec![
                Span::styled(
                    " q",
                    RatatuiStyle::default().add_modifier(Modifier::BOLD,),
                ),
                Span::raw(": quit | ",),
                Span::styled(
                    "j/↓",
                    RatatuiStyle::default().add_modifier(Modifier::BOLD,),
                ),
                Span::raw(": down | ",),
                Span::styled(
                    "k/↑",
                    RatatuiStyle::default().add_modifier(Modifier::BOLD,),
                ),
                Span::raw(": up",),
            ],);
            f.render_widget(
                Paragraph::new(help_text,),
                *layout.get(1,).expect("layout should have a second chunk",),
            );
        },)?;

        if event::poll(std::time::Duration::from_millis(100,),)?
            && let Event::Key(key,) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q',) | KeyCode::Esc => return Ok((),),
                KeyCode::Down | KeyCode::Char('j',) => {
                    let i = match state.selected() {
                        Some(i,) => {
                            if i >= packages.len() - 1 {
                                0
                            } else {
                                i + 1
                            }
                        }
                        None => 0,
                    };
                    state.select(Some(i,),);
                }
                KeyCode::Up | KeyCode::Char('k',) => {
                    let i = match state.selected() {
                        Some(i,) => {
                            if i == 0 {
                                packages.len() - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    state.select(Some(i,),);
                }
                _ => {}
            }
        }
    }
}
