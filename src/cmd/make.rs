use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use serde_json::{Value, json};
use std::{error::Error, io, path::Path};

struct App<'a> {
    inputs: Vec<String>,
    focused_input: usize,
    labels: Vec<&'a str>,
}

impl<'a> App<'a> {
    fn new(pkg_name: Option<String>) -> Self {
        let mut inputs = vec![String::new(); 20];
        if let Some(name) = pkg_name {
            inputs[0] = name;
        }
        Self {
            inputs,
            focused_input: 0,
            labels: vec![
                "Name",
                "Repo",
                "Version",
                "Description",
                "Git URL",
                "Website (optional)",
                "License (optional)",
                "Tags (optional, comma-separated)",
                "Bins (optional, comma-separated)",
                "Conflicts (optional, comma-separated)",
                "Package Type (package, collection, service, config, app)",
                "Scope (user, system)",
                "Maintainer Name",
                "Maintainer Email",
                "Maintainer Website (optional)",
                "Maintainer Key (optional)",
                "Author Name (optional)",
                "Author Email (optional)",
                "Author Website (optional)",
                "Author Key (optional)",
            ],
        }
    }

    fn next(&mut self) {
        self.focused_input = (self.focused_input + 1) % self.inputs.len();
    }

    fn previous(&mut self) {
        if self.focused_input > 0 {
            self.focused_input -= 1;
        } else {
            self.focused_input = self.inputs.len() - 1;
        }
    }
}

pub fn run(package_name: Option<String>) -> Result<(), Box<dyn Error>> {
    let app = App::new(package_name);
    let final_pkg_val = {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = run_app(&mut terminal, app);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        res?
    };

    if let Some(pkg_val) = final_pkg_val {
        let filename = format!(
            "{}.pkg.yaml",
            pkg_val
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("package")
        );

        if Path::new(&filename).exists() {
            println!("File '{}' already exists.", filename);
            let overwrite = dialoguer::Confirm::new()
                .with_prompt("Do you want to overwrite it?")
                .default(false)
                .interact()?;
            if !overwrite {
                println!("Operation cancelled.");
                return Ok(());
            }
        }

        let yaml = serde_yaml::to_string(&pkg_val)?;
        let schema_line = "# yaml-language-server: $schema=https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/pkg.schema.json";
        let final_yaml = format!("{}\n{}", schema_line, yaml);
        std::fs::write(&filename, final_yaml)?;
        println!("Package file created: {}", filename);
    }

    Ok(())
}

fn run_app<'a>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App<'a>,
) -> io::Result<Option<Value>> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            match key {
                KeyEvent {
                    code: KeyCode::Char('d'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    let pkg = build_package_json(&app.inputs);
                    return Ok(Some(pkg));
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    if app.focused_input == app.inputs.len() - 1 {
                        let pkg = build_package_json(&app.inputs);
                        return Ok(Some(pkg));
                    }
                    app.next();
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    ..
                } => {
                    app.inputs[app.focused_input].push(c);
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    app.inputs[app.focused_input].pop();
                }
                KeyEvent {
                    code: KeyCode::Tab, ..
                } => app.next(),
                KeyEvent {
                    code: KeyCode::BackTab,
                    ..
                } => app.previous(),
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => return Ok(None),
                _ => {}
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(1),
            ]
            .as_ref(),
        )
        .split(f.area());

    let (msg, style) = (
        vec![
            "Press ".into(),
            "Esc".bold(),
            " to exit, ".into(),
            "Tab".bold(),
            " to move, ".into(),
            "Ctrl-D".bold(),
            " to confirm.".into(),
        ],
        Style::default().add_modifier(Modifier::RAPID_BLINK),
    );
    let text = Text::from(Line::from(msg)).patch_style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, chunks[0]);

    let mut input_rects = Vec::new();
    let num_inputs = app.inputs.len();
    let num_columns = 2;
    let num_rows = num_inputs.div_ceil(num_columns);

    let form_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[2]);

    for i in 0..num_columns {
        let col_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(std::iter::repeat_n(Constraint::Length(3), num_rows).collect::<Vec<_>>())
            .split(form_chunks[i]);

        for j in 0..num_rows {
            let index = j * num_columns + i;
            if index < num_inputs {
                input_rects.push(col_chunks[j]);
            }
        }
    }

    for (i, &rect) in input_rects.iter().enumerate() {
        let input = Paragraph::new(app.inputs[i].as_str())
            .style(if app.focused_input == i {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            })
            .block(Block::default().borders(Borders::ALL).title(app.labels[i]))
            .wrap(Wrap { trim: true });
        f.render_widget(input, rect);
    }
}

fn build_package_json(inputs: &[String]) -> Value {
    let tags = if inputs[7].is_empty() {
        vec![]
    } else {
        inputs[7].split(',').map(|s| s.trim().to_string()).collect()
    };
    let bins = if inputs[8].is_empty() {
        None
    } else {
        Some(
            inputs[8]
                .split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<String>>(),
        )
    };
    let conflicts = if inputs[9].is_empty() {
        None
    } else {
        Some(
            inputs[9]
                .split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<String>>(),
        )
    };
    let package_type = match inputs[10].to_lowercase().as_str() {
        "collection" => "collection",
        "service" => "service",
        "config" => "config",
        "app" => "app",
        "" => "package",
        _ => "package",
    };
    let scope = match inputs[11].to_lowercase().as_str() {
        "system" => "system",
        "" => "user",
        _ => "user",
    };

    let mut map = serde_json::Map::new();
    map.insert("name".to_string(), json!(inputs[0]));
    map.insert("repo".to_string(), json!(inputs[1]));
    map.insert("version".to_string(), json!(inputs[2]));
    map.insert("description".to_string(), json!(inputs[3]));
    map.insert("git".to_string(), json!(inputs[4]));
    if !inputs[5].is_empty() {
        map.insert("website".to_string(), json!(inputs[5]));
    }
    if !inputs[6].is_empty() {
        map.insert("license".to_string(), json!(inputs[6]));
    }
    map.insert("tags".to_string(), json!(tags));
    if let Some(b) = bins {
        map.insert("bins".to_string(), json!(b));
    }
    if let Some(c) = conflicts {
        map.insert("conflicts".to_string(), json!(c));
    }
    map.insert("type".to_string(), json!(package_type));
    map.insert("scope".to_string(), json!(scope));

    let maintainer = json!({
        "name": inputs[12],
        "email": inputs[13],
        "website": Some(inputs[14].clone()).filter(|s| !s.is_empty()),
        "key": Some(inputs[15].clone()).filter(|s| !s.is_empty()),
    });
    map.insert("maintainer".to_string(), maintainer);

    if !inputs[16].is_empty() {
        let author = json!({
            "name": inputs[16],
            "email": Some(inputs[17].clone()).filter(|s| !s.is_empty()),
            "website": Some(inputs[18].clone()).filter(|s| !s.is_empty()),
            "key": Some(inputs[19].clone()).filter(|s| !s.is_empty()),
        });
        map.insert("author".to_string(), author);
    }

    Value::Object(map)
}
