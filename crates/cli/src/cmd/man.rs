use crate::pkg::{
    db, local, resolve,
    types::{self, ManSpec},
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
use std::collections::BTreeMap;
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

struct App<'a> {
    pages: Vec<(String, Vec<Line<'a>>)>,
    current_page: usize,
    scroll: u16,
    content_height: u16,
}

impl<'a> App<'a> {
    fn try_new(pages: BTreeMap<String, String>) -> Result<Self> {
        let mut parsed_pages = Vec::new();
        for (name, content) in pages {
            let lines = parse_markdown(&content)?;
            parsed_pages.push((name, lines));
        }

        if parsed_pages.is_empty() {
            return Err(anyhow!("No manual pages found."));
        }

        let content_height = parsed_pages[0].1.len() as u16;
        Ok(Self {
            pages: parsed_pages,
            current_page: 0,
            scroll: 0,
            content_height,
        })
    }
}

pub fn run(package_name: &str, upstream: bool, raw: bool, no_tui: bool) -> Result<()> {
    let (pkg, registry_handle) = resolve_package_for_man(package_name)?;

    let pages = gather_manual_pages(&pkg, &registry_handle, upstream, raw)?;

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
                println!("--- {} ---", name);
            }
            println!("{}", content);
        }
        return Ok(());
    }

    if no_tui {
        let mut full_content = String::new();
        let multi = pages.len() > 1;
        for (name, content) in pages {
            if multi {
                full_content.push_str(&format!("--- {} ---\n\n", name));
            }
            full_content.push_str(&content);
            full_content.push('\n');
        }
        return run_pager(&full_content);
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
        eprintln!("{:?}", err)
    }

    Ok(())
}

fn run_pager(content: &str) -> Result<()> {
    let pager = std::env::var("PAGER").ok();

    if let Some(p) = pager
        && spawn_pager(&p, content).is_ok()
    {
        return Ok(());
    }

    if spawn_pager("less", content).is_ok() {
        return Ok(());
    }

    if spawn_pager("more", content).is_ok() {
        return Ok(());
    }

    println!("{}", content);
    Ok(())
}

fn spawn_pager(pager: &str, content: &str) -> Result<()> {
    let mut child = std::process::Command::new(pager)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn pager '{}': {}", pager, e))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("Failed to open stdin for pager"))?;

    use std::io::Write;
    stdin.write_all(content.as_bytes())?;
    drop(stdin);

    child.wait()?;
    Ok(())
}

pub fn resolve_package_for_man(term: &str) -> Result<(types::Package, Option<String>)> {
    if let Ok((pkg, _, _, _, registry_handle, _, _)) =
        resolve::resolve_package_and_version(term, false, false)
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
            && !results.is_empty()
        {
            return Ok((results[0].0.clone(), Some(handle)));
        }
    }

    Err(anyhow!(
        "Could not find package or binary named '{}'.",
        term
    ))
}

pub fn gather_manual_pages(
    pkg: &types::Package,
    registry_handle: &Option<String>,
    upstream: bool,
    raw: bool,
) -> Result<BTreeMap<String, String>> {
    let mut pages = BTreeMap::new();

    if !upstream {
        let handle = registry_handle.as_deref().unwrap_or("local");
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
                                "Displaying locally installed manual from {:?} scope...",
                                scope
                            );
                        }
                        pages.extend(local_pages);
                        break;
                    }
                }
            }
        }
    }

    if pages.is_empty()
        && let Some(man_spec) = &pkg.man
    {
        if !raw {
            println!("Fetching manual from upstream...");
        }
        match man_spec {
            ManSpec::Single(url) => {
                pages.insert("main".to_string(), fetch_url(url)?);
            }
            ManSpec::Multiple(urls) => {
                for (i, url) in urls.iter().enumerate() {
                    pages.insert(format!("page{}", i + 1), fetch_url(url)?);
                }
            }
            ManSpec::Map(map) => {
                for (name, url) in map {
                    pages.insert(name.clone(), fetch_url(url)?);
                }
            }
        }
    }

    Ok(pages)
}

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

    let share_man = latest_dir.join("share").join("man");
    if share_man.exists() {
        for entry in WalkDir::new(share_man) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let path = entry.path();
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let content = fs::read_to_string(path)?;
                if name.ends_with(".md") {
                    pages.insert(name, content);
                } else if content.starts_with('.') {
                    pages.insert(name, parse_roff(&content));
                } else {
                    pages.insert(name, content);
                }
            }
        }
    }

    Ok(pages)
}

fn fetch_url(url: &str) -> Result<String> {
    Ok(reqwest::blocking::get(url)?.text()?)
}

pub fn parse_roff(content: &str) -> String {
    let mut md = String::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(".TH") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                md.push_str(&format!("# {}\n\n", parts[1]));
            }
        } else if line.starts_with(".SH") {
            let title = line.trim_start_matches(".SH").trim();
            md.push_str(&format!("## {}\n\n", title));
        } else if line.starts_with(".SS") {
            let title = line.trim_start_matches(".SS").trim();
            md.push_str(&format!("### {}\n\n", title));
        } else if line.starts_with(".PP") || line.starts_with(".P") || line.starts_with(".LP") {
            md.push_str("\n\n");
        } else if line.starts_with(".B ") {
            md.push_str(&format!("**{}**", line.trim_start_matches(".B ").trim()));
        } else if line.starts_with(".I ") {
            md.push_str(&format!("*{}*", line.trim_start_matches(".I ").trim()));
        } else if line.starts_with(".BR ") {
            let parts: Vec<&str> = line.split_whitespace().skip(1).collect();
            if !parts.is_empty() {
                md.push_str(&format!("**{}**", parts[0]));
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
                    app.content_height = app.pages[app.current_page].1.len() as u16;
                }
                KeyCode::BackTab => {
                    app.current_page = if app.current_page == 0 {
                        app.pages.len() - 1
                    } else {
                        app.current_page - 1
                    };
                    app.scroll = 0;
                    app.content_height = app.pages[app.current_page].1.len() as u16;
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

        f.render_widget(list, chunks[0]);
        chunks[1]
    } else {
        size
    };

    let (name, lines) = &app.pages[app.current_page];
    let text = Text::from(lines.clone());

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Manual: {}", name)),
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
                Tag::Paragraph => {}
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
                        highlighter = Some((
                            HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]),
                            String::new(),
                        ));
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
                            format!("{}. ", index)
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
                                    .map_err(|e| anyhow!("Syntax highlighting failed: {}", e))?;
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
                            format!("]({})", link_url),
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
