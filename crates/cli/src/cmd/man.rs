//! Command for displaying manual pages for packages.
//!
//! This module provides a TUI (Terminal User Interface) and a pager-based
//! way to view manual pages, supporting both locally installed pages
//! and fetching pages from upstream registries.

use crate::pkg::{
    db, local, resolve,
    types::{self},
};
use anyhow::{Result, anyhow};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use pulldown_cmark::{Event as CmarkEvent, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    prelude::*,
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Wrap,
    },
};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::Path;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};
use walkdir::WalkDir;

/// State for the manual page viewer TUI.
struct App<'a> {
    /// The manual pages to display, parsed as TUI lines.
    pages: Vec<(String, Vec<Line<'a>>)>,
    /// Index of the currently displayed page.
    current_page: usize,
    /// Vertical scroll position.
    scroll: u16,
    /// Total height of the current page's content.
    content_height: u16,
}

impl App<'_> {
    /// Tries to create a new TUI app from a map of page names to content.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing any page fails or if no pages are provided.
    fn try_new(pages: BTreeMap<String, String>) -> Result<Self> {
        let mut parsed_pages = Vec::new();
        for (name, content) in pages {
            let lines = parse_markdown(&content)?;
            parsed_pages.push((name, lines));
        }

        if parsed_pages.is_empty() {
            return Err(anyhow!("No manual pages found."));
        }

        let content_height = u16::try_from(parsed_pages.first().map_or(0, |p| p.1.len())).unwrap_or(u16::MAX);
        Ok(Self {
            pages: parsed_pages,
            current_page: 0,
            scroll: 0,
            content_height,
        })
    }
}

/// Runs the manual page viewer for the specified package.
///
/// # Errors
///
/// Returns an error if:
/// - The package cannot be resolved.
/// - Manual pages cannot be gathered.
/// - The terminal cannot be initialized.
pub fn run(package_name: &str, upstream: bool, raw: bool, no_tui: bool) -> Result<()> {
    let (pkg, registry_handle) = resolve_package_for_man(package_name)?;

    let pages = gather_manual_pages(&pkg, registry_handle.as_deref(), upstream, raw)?;

    if pages.is_empty() {
        return Err(anyhow!(
            "Package '{}' does not have any manual pages.",
            pkg.name
        ));
    }

    if raw {
        let multi = pages.len() > 1;
        for (name, content) in pages {
            if multi {
                println!("--- {name} ---");
            }
            println!("{content}");
        }
        return Ok(());
    }

    if no_tui {
        use std::fmt::Write;
        let mut full_content = String::new();
        let multi = pages.len() > 1;
        for (name, content) in pages {
            if multi {
                let _ = writeln!(full_content, "--- {name} ---\n");
            }
            full_content.push_str(&content);
            full_content.push('\n');
        }
        run_pager(&full_content);
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::try_new(pages)?;
    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{err:?}");
    }

    Ok(())
}

/// Runs a pager to display the given content.
fn run_pager(content: &str) {
    let pager = std::env::var("PAGER").ok();

    if let Some(p) = pager
        && spawn_pager(&p, content).is_ok()
    {
        return;
    }

    if spawn_pager("less", content).is_ok() {
        return;
    }

    if spawn_pager("more", content).is_ok() {
        return;
    }

    println!("{content}");
}

/// Spawns a specific pager process and writes content to its stdin.
///
/// # Errors
///
/// Returns an error if the pager fails to spawn or if writing to its stdin fails.
fn spawn_pager(pager: &str, content: &str) -> Result<()> {
    let mut child = std::process::Command::new(pager)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn pager '{pager}': {e}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("Failed to open stdin for pager"))?;

    io::Write::write_all(&mut stdin, content.as_bytes())?;
    drop(stdin);

    child.wait()?;
    Ok(())
}

/// Resolves a package and optional registry handle for a given search term.
///
/// # Errors
///
/// Returns an error if:
/// - The package or binary cannot be found.
/// - The project configuration cannot be read.
pub fn resolve_package_for_man(term: &str) -> Result<(types::Package, Option<String>)> {
    if let Ok((pkg, _, _, _, registry_handle, _, _)) =
        resolve::resolve_package_and_version(term, None, false, false)
    {
        return Ok((pkg, registry_handle));
    }

    let config = crate::pkg::config::read_config()?;
    let mut registries = Vec::new();
    if let Some(default) = &config.default_registry {
        registries.push(default.handle.clone());
    }
    for reg in &config.added_registries {
        registries.push(reg.handle.clone());
    }

    for handle in registries {
        if let Ok(results) = db::find_provides(&handle, term)
            && let Some(result) = results.first()
        {
            return Ok((result.0.clone(), Some(handle)));
        }
    }

    Err(anyhow!(
        "Could not find package or binary named '{term}'."
    ))
}

/// Gathers manual pages for a package, checking locally and then upstream.
///
/// # Errors
///
/// Returns an error if:
/// - Local manual pages cannot be found.
/// - Upstream manual pages cannot be gathered.
pub fn gather_manual_pages(
    pkg: &types::Package,
    registry_handle: Option<&str>,
    upstream: bool,
    raw: bool,
) -> Result<BTreeMap<String, String>> {
    let mut pages = BTreeMap::new();

    if !upstream {
        let handle = registry_handle.unwrap_or("local");
        let scopes_to_check = [
            types::Scope::Project,
            types::Scope::User,
            types::Scope::System,
        ];

        for scope in scopes_to_check {
            if let Ok(package_dir) = local::get_package_dir(scope, handle, &pkg.repo, &pkg.name) {
                let latest_dir = package_dir.join("latest");
                if latest_dir.exists() {
                    let local_pages = find_local_man_pages(&latest_dir)?;
                    if !local_pages.is_empty() {
                        if !raw {
                            println!(
                                "Displaying locally installed manual from {scope:?} scope..."
                            );
                        }
                        pages.extend(local_pages);
                        break;
                    }
                }
            }

            // Also check standard system locations if scope is system
            if scope == types::Scope::System {
                let system_man = Path::new("/usr/share/man");
                if system_man.exists() {
                    let system_pages = find_man_pages_in_hierarchy(system_man, &pkg.name)?;
                    if !system_pages.is_empty() {
                        if !raw {
                            println!("Displaying manual from system /usr/share/man...");
                        }
                        pages.extend(system_pages);
                        break;
                    }
                }
            }
        }
    }

    if pages.is_empty() {
        if !raw {
            println!("Package not installed or local manual not found. Fetching from upstream...");
        }
        let upstream_pages = gather_manual_pages_from_upstream(pkg, registry_handle)?;
        pages.extend(upstream_pages);
    }

    Ok(pages)
}

/// Recursively finds manual pages in a directory hierarchy.
///
/// # Errors
///
/// Returns an error if directory traversal or file reading fails.
fn find_man_pages_in_hierarchy(root: &Path, term: &str) -> Result<BTreeMap<String, String>> {
    let mut pages = BTreeMap::new();
    if !root.exists() {
        return Ok(pages);
    }

    for entry in WalkDir::new(root).max_depth(3) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if name.starts_with(term) {
                let content = fs::read_to_string(entry.path())?;
                pages.insert(
                    name.to_string(),
                    if content.starts_with('.') {
                        parse_roff(&content)
                    } else {
                        content
                    },
                );
            }
        }
    }
    Ok(pages)
}

/// Fetches manual pages for a package from the upstream registry.
///
/// # Errors
///
/// Returns an error if:
/// - Dependency resolution fails.
/// - The package archive cannot be downloaded or extracted.
/// - File reading fails.
///
/// # Panics
///
/// Panics if the internal dependency graph is inconsistent.
fn gather_manual_pages_from_upstream(
    pkg: &types::Package,
    registry_handle: Option<&str>,
) -> Result<BTreeMap<String, String>> {
    // Resolve the package to get its archive source
    let source = registry_handle.map_or_else(
        || pkg.name.clone(),
        |handle| format!("#{}@{}", handle, pkg.name),
    );

    let (mut graph, _) = crate::pkg::install::resolver::resolve_dependency_graph(
        &[source],
        None,
        false,
        true,
        true,
        None,
        true,
        None,
    )?;

    if graph.nodes.is_empty() {
        return Ok(BTreeMap::new());
    }

    let node_id = graph
        .nodes
        .keys()
        .next()
        .expect("Graph should not be empty")
        .clone();
    let node = graph
        .nodes
        .remove(&node_id)
        .expect("Node should exist in graph");

    let install_plan = crate::pkg::install::plan::create_install_plan(
        &HashMap::from([(node_id.clone(), node.clone())]),
        None,
        false,
    )?;

    let action = install_plan
        .get(&node_id)
        .ok_or_else(|| anyhow!("No install action for package"))?;

    // Prepare the node (download/build)
    let prepared = crate::pkg::install::installer::prepare_node(&node, action, None, None, false)?;

    // Extract to a temp directory
    let temp_dir = tempfile::Builder::new()
        .prefix("zoi-man-extract-")
        .tempdir()?;
    let extract_path = temp_dir.path();

    if prepared.archive_path.exists() {
        let file = fs::File::open(&prepared.archive_path)?;
        let decoder = zstd::stream::read::Decoder::new(file)?;
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(extract_path)?;
    }

    // Look for man pages in the extracted content
    // We check:
    // - manifest.json (for pooled ZPA)
    // - data/pkgstore/man/
    // - data/usrroot/usr/share/man/
    // - any .pkg.lua in the root

    let mut pages = BTreeMap::new();

    let pooled_manifest = extract_path.join("manifest.json");
    if pooled_manifest.exists() {
        let content = fs::read_to_string(&pooled_manifest)?;
        let manifest: types::PooledZpaManifest = serde_json::from_str(&content)?;
        let pool_dir = extract_path.join("pool");

        for (sub_name, sub_mapping) in manifest.mappings {
            for (scope, scope_mapping) in sub_mapping.scopes {
                for file in scope_mapping.files {
                    if file.dest.contains("/man/")
                        || file.dest.ends_with(".1")
                        || file.dest.ends_with(".5")
                    {
                        let pool_file = pool_dir.join(&file.hash);
                        if pool_file.exists() {
                            let content = fs::read_to_string(pool_file)?;
                            let file_name = Path::new(&file.dest)
                                .file_name()
                                .expect("Dest should have a file name")
                                .to_string_lossy();
                            let display_name = format!("{file_name}[{sub_name}:{scope:?}]");
                            pages.insert(
                                display_name,
                                if content.starts_with('.') {
                                    parse_roff(&content)
                                } else {
                                    content
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    let legacy_man = extract_path.join("data/pkgstore/man");
    if legacy_man.exists() {
        pages.extend(find_local_man_pages(&extract_path.join("data/pkgstore"))?);
    }

    Ok(pages)
}

/// Finds manual pages in a package's installation directory.
fn find_local_man_pages(latest_dir: &Path) -> Result<BTreeMap<String, String>> {
    let mut pages = BTreeMap::new();

    let md_path = latest_dir.join("man.md");
    let txt_path = latest_dir.join("man.txt");

    if md_path.exists() {
        pages.insert("main".to_string(), fs::read_to_string(md_path)?);
        return Ok(pages);
    }

    if txt_path.exists() {
        pages.insert("main".to_string(), fs::read_to_string(txt_path)?);
        return Ok(pages);
    }

    let search_dirs = [latest_dir.join("share").join("man"), latest_dir.join("man")];

    for dir in search_dirs {
        if dir.exists() {
            for entry in WalkDir::new(dir) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .expect("Path should have a file name")
                        .to_string_lossy()
                        .to_string();
                    let content = fs::read_to_string(path)?;
                    if name.to_lowercase().ends_with(".md") {
                        pages.insert(name, content);
                    } else if content.starts_with('.') {
                        pages.insert(name, parse_roff(&content));
                    } else {
                        pages.insert(name, content);
                    }
                }
            }
        }
    }

    Ok(pages)
}

/// Parses a ROFF-formatted string (traditional man page) into a simplified Markdown string.
#[must_use]
pub fn parse_roff(content: &str) -> String {
    use std::fmt::Write;
    let mut md = String::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(".TH") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(part) = parts.get(1) {
                let _ = writeln!(md, "# {part}\n");
            }
        } else if line.starts_with(".SH") {
            let title = line.trim_start_matches(".SH").trim();
            let _ = writeln!(md, "## {title}\n");
        } else if line.starts_with(".SS") {
            let title = line.trim_start_matches(".SS").trim();
            let _ = writeln!(md, "### {title}\n");
        } else if line.starts_with(".PP") || line.starts_with(".P") || line.starts_with(".LP") {
            md.push_str("\n\n");
        } else if line.starts_with(".B ") {
            let _ = write!(md, "**{}**", line.trim_start_matches(".B ").trim());
        } else if line.starts_with(".I ") {
            let _ = write!(md, "*{}*", line.trim_start_matches(".I ").trim());
        } else if line.starts_with(".BR ") {
            let parts: Vec<&str> = line.split_whitespace().skip(1).collect();
            if let Some(first) = parts.first() {
                let _ = write!(md, "**{first}**");
                for p in parts.iter().skip(1) {
                    md.push_str(p);
                }
            }
        } else if line.starts_with('.') {
        } else {
            md.push_str(line);
            md.push('\n');
        }
    }
    md
}

/// Main loop for the TUI application.
fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => {
                    app.scroll = app.scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.scroll = app.scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    app.scroll = app.scroll.saturating_add(terminal.size()?.height);
                }
                KeyCode::PageUp => {
                    app.scroll = app.scroll.saturating_sub(terminal.size()?.height);
                }
                KeyCode::Home => app.scroll = 0,
                KeyCode::End => app.scroll = app.content_height,
                KeyCode::Tab => {
                    app.current_page = (app.current_page + 1) % app.pages.len();
                    app.scroll = 0;
                    app.content_height = u16::try_from(
                        app.pages
                            .get(app.current_page)
                            .map_or(0, |p| p.1.len()),
                    )
                    .unwrap_or(u16::MAX);
                }
                KeyCode::BackTab => {
                    app.current_page = if app.current_page == 0 {
                        app.pages.len() - 1
                    } else {
                        app.current_page - 1
                    };
                    app.scroll = 0;
                    app.content_height = u16::try_from(
                        app.pages
                            .get(app.current_page)
                            .map_or(0, |p| p.1.len()),
                    )
                    .unwrap_or(u16::MAX);
                }
                _ => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => app.scroll = app.scroll.saturating_sub(3),
                MouseEventKind::ScrollDown => app.scroll = app.scroll.saturating_add(3),
                _ => {}
            },
            _ => {}
        }
    }
}

/// Renders the TUI.
fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let has_sidebar = app.pages.len() > 1;
    let main_area = if has_sidebar {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
            .split(size);

        let items: Vec<ListItem> = app
            .pages
            .iter()
            .enumerate()
            .map(|(i, (name, _))| {
                let style = if i == app.current_page {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(name.as_str()).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Pages"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        if let Some(sidebar_chunk) = chunks.first() {
            f.render_widget(list, *sidebar_chunk);
        }
        chunks.get(1).copied().unwrap_or(size)
    } else {
        size
    };

    let Some((name, lines)) = app.pages.get(app.current_page) else {
        return;
    };
    let text = Text::from(lines.clone());

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Manual: {name}")),
        )
        .wrap(Wrap { trim: true })
        .scroll((app.scroll, 0));

    f.render_widget(paragraph, main_area);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    let mut scrollbar_state =
        ScrollbarState::new(app.content_height as usize).position(app.scroll as usize);

    f.render_stateful_widget(
        scrollbar,
        main_area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

/// Parses Markdown content into TUI lines.
///
/// # Errors
///
/// Returns an error if:
/// - A style stack underflow occurs.
/// - Syntax highlighting fails.
///
/// # Panics
///
/// Panics if a style stack invariant is violated.
fn parse_markdown(content: &str) -> Result<Vec<Line<'static>>> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(content, options);

    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut style_stack = vec![Style::default()];
    let mut list_stack: Vec<(u64, char)> = Vec::new();

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let mut highlighter: Option<(HighlightLines, String)> = None;
    let mut link_url = String::new();

    for event in parser {
        match event {
            CmarkEvent::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    style_stack.push(
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(Color::Yellow),
                    );
                    let level_num = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    current_line.push(Span::raw("#".repeat(level_num) + " "));
                }
                Tag::BlockQuote(_) => {
                    style_stack.push(Style::default().fg(Color::Gray));
                    current_line.push(Span::styled(
                        "> ",
                        *style_stack
                            .last()
                            .ok_or_else(|| anyhow!("Style stack should never be empty"))?,
                    ));
                }
                Tag::CodeBlock(kind) => {
                    let lang = if let pulldown_cmark::CodeBlockKind::Fenced(lang) = kind {
                        lang.into_string()
                    } else {
                        "text".to_string()
                    };
                    if let Some(syntax) = ss.find_syntax_by_extension(&lang) {
                        if let Some(theme) = ts.themes.get("base16-ocean.dark") {
                            highlighter = Some((HighlightLines::new(syntax, theme), String::new()));
                        } else {
                            highlighter = None;
                        }
                    } else {
                        highlighter = None;
                    }
                }
                Tag::List(start_index) => {
                    list_stack.push((start_index.unwrap_or(1), '*'));
                }
                Tag::Item => {
                    let list_len = list_stack.len();
                    if let Some((index, _)) = list_stack.last_mut() {
                        let marker = if *index > 0 {
                            format!("{index}. ")
                        } else {
                            "* ".to_string()
                        };
                        current_line.push(Span::raw("  ".repeat(list_len - 1)));
                        current_line.push(Span::raw(marker));
                        *index += 1;
                    }
                }
                Tag::Emphasis => {
                    style_stack.push(
                        (*style_stack
                            .last()
                            .ok_or_else(|| anyhow!("Style stack should never be empty"))?)
                        .add_modifier(Modifier::ITALIC),
                    );
                }
                Tag::Strong => {
                    style_stack.push(
                        (*style_stack
                            .last()
                            .ok_or_else(|| anyhow!("Style stack should never be empty"))?)
                        .add_modifier(Modifier::BOLD),
                    );
                }
                Tag::Strikethrough => {
                    style_stack.push(
                        (*style_stack
                            .last()
                            .ok_or_else(|| anyhow!("Style stack should never be empty"))?)
                        .add_modifier(Modifier::CROSSED_OUT),
                    );
                }
                Tag::Link { dest_url, .. } => {
                    link_url = dest_url.to_string();
                    current_line.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
                    style_stack.push(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::UNDERLINED),
                    );
                }
                _ => {}
            },
            CmarkEvent::End(tag) => {
                match tag {
                    TagEnd::Paragraph
                    | TagEnd::Heading { .. }
                    | TagEnd::BlockQuote(_)
                    | TagEnd::Item => {
                        lines.push(Line::from(std::mem::take(&mut current_line)));
                    }
                    TagEnd::CodeBlock => {
                        if let Some((mut h, code)) = highlighter.take() {
                            for line in LinesWithEndings::from(&code) {
                                let ranges: Vec<(SyntectStyle, &str)> = h
                                    .highlight_line(line, &ss)
                                    .map_err(|e| anyhow!("Syntax highlighting failed: {e}"))?;
                                let spans: Vec<Span<'static>> = ranges
                                    .into_iter()
                                    .map(|(style, text)| {
                                        Span::styled(
                                            text.to_string(),
                                            Style::default()
                                                .fg(Color::Rgb(
                                                    style.foreground.r,
                                                    style.foreground.g,
                                                    style.foreground.b,
                                                ))
                                                .bg(Color::Rgb(
                                                    style.background.r,
                                                    style.background.g,
                                                    style.background.b,
                                                )),
                                        )
                                    })
                                    .collect();
                                lines.push(Line::from(spans));
                            }
                        }
                        lines.push(Line::from(vec![]));
                    }
                    TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                        style_stack.pop();
                    }
                    TagEnd::Link => {
                        style_stack.pop();
                        current_line.push(Span::styled(
                            format!("]({link_url})"),
                            Style::default().fg(Color::DarkGray),
                        ));
                        link_url.clear();
                    }
                    TagEnd::List(_) => {
                        list_stack.pop();
                        if list_stack.is_empty() {
                            lines.push(Line::from(vec![]));
                        }
                    }
                    _ => {}
                }
                if let TagEnd::Heading { .. } | TagEnd::BlockQuote(_) = tag {
                    style_stack.pop();
                }
            }
            CmarkEvent::Text(text) => {
                if let Some((_, code)) = &mut highlighter {
                    code.push_str(&text);
                } else {
                    current_line.push(Span::styled(
                        text.to_string(),
                        *style_stack
                            .last()
                            .ok_or_else(|| anyhow!("Style stack should never be empty"))?,
                    ));
                }
            }
            CmarkEvent::Code(text) => {
                current_line.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(Color::Green).bg(Color::DarkGray),
                ));
            }
            CmarkEvent::HardBreak => {
                lines.push(Line::from(std::mem::take(&mut current_line)));
            }
            CmarkEvent::SoftBreak => {
                current_line.push(Span::raw(" "));
            }
            CmarkEvent::Rule => {
                lines.push(Line::from("---"));
            }
            _ => {}
        }
    }
    if !current_line.is_empty() {
        lines.push(Line::from(std::mem::take(&mut current_line)));
    }

    Ok(lines)
}
