use crate::pkg::{local, resolve};
use anyhow::anyhow;
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
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};
use std::fs;
use std::io;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

struct App<'a> {
    lines: Vec<Line<'a>>,
    scroll: u16,
    content_height: u16,
}

impl<'a> App<'a> {
    fn new(content: &'a str) -> Self {
        let lines = parse_markdown(content);
        let content_height = lines.len() as u16;
        Self {
            lines,
            scroll: 0,
            content_height,
        }
    }
}

pub fn run(
    package_name: &str,
    upstream: bool,
    raw: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (pkg, _version, _, _, registry_handle) =
        resolve::resolve_package_and_version(package_name)?;

    let fetch_from_upstream = || -> Result<String, Box<dyn std::error::Error>> {
        if let Some(url) = pkg.man.as_ref() {
            if !raw {
                println!("Fetching manual from {}...", url);
            }
            Ok(reqwest::blocking::get(url)?.text()?)
        } else {
            Err(anyhow!("Package '{}' does not have a manual URL.", package_name).into())
        }
    };

    let content = if upstream {
        fetch_from_upstream()?
    } else {
        let handle = registry_handle.as_deref().unwrap_or("local");
        let scopes_to_check = [
            crate::pkg::types::Scope::Project,
            crate::pkg::types::Scope::User,
            crate::pkg::types::Scope::System,
        ];
        let mut found_manual = None;

        for scope in scopes_to_check {
            if let Ok(package_dir) = local::get_package_dir(scope, handle, &pkg.repo, &pkg.name) {
                let latest_dir = package_dir.join("latest");
                if !latest_dir.exists() {
                    continue;
                }

                let man_md_path = latest_dir.join("man.md");
                let man_txt_path = latest_dir.join("man.txt");

                if man_md_path.exists() {
                    if !raw {
                        println!(
                            "Displaying locally installed manual (Markdown) from {:?} scope...",
                            scope
                        );
                    }
                    found_manual = Some(fs::read_to_string(man_md_path)?);
                    break;
                } else if man_txt_path.exists() {
                    if !raw {
                        println!(
                            "Displaying locally installed manual (text) from {:?} scope...",
                            scope
                        );
                    }
                    found_manual = Some(fs::read_to_string(man_txt_path)?);
                    break;
                }
            }
        }

        if let Some(manual_content) = found_manual {
            manual_content
        } else {
            fetch_from_upstream()?
        }
    };

    if raw {
        print!("{}", content);
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(&content);
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

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        match event::read()? {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
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
                        _ => {}
                    }
                }
            }
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
    let text = Text::from(app.lines.clone());

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Manual"))
        .wrap(Wrap { trim: true })
        .scroll((app.scroll, 0));

    f.render_widget(paragraph, size);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    let mut scrollbar_state =
        ScrollbarState::new(app.content_height as usize).position(app.scroll as usize);

    f.render_stateful_widget(
        scrollbar,
        size.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

fn parse_markdown(content: &str) -> Vec<Line<'_>> {
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
                    current_line.push(Span::styled("> ", *style_stack.last().unwrap()));
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
                    style_stack.push((*style_stack.last().unwrap()).add_modifier(Modifier::ITALIC));
                }
                Tag::Strong => {
                    style_stack.push((*style_stack.last().unwrap()).add_modifier(Modifier::BOLD));
                }
                Tag::Strikethrough => {
                    style_stack
                        .push((*style_stack.last().unwrap()).add_modifier(Modifier::CROSSED_OUT));
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
                                let ranges: Vec<(SyntectStyle, &str)> =
                                    h.highlight_line(line, &ss).unwrap();
                                let spans: Vec<Span> = ranges
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
                    current_line.push(Span::styled(text.to_string(), *style_stack.last().unwrap()));
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

    lines
}
