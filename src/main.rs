mod app;
mod config;
mod filter_field;
mod opensearch;
mod ui;

use anyhow::Result;
use app::{App, Pane, CONTEXT_MENU_OPTIONS};
use arboard::Clipboard;
use config::AppConfig;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use std::io;
use std::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    let config = match config::load_config() {
        Ok(Some(cfg)) => cfg,
        Ok(None) => {
            match run_setup_dialog(None)? {
                Some(cfg) => cfg,
                None => return Ok(()),
            }
        }
        Err(e) => {
            match run_setup_dialog(Some(&format!("Config error: {}", e)))? {
                Some(cfg) => cfg,
                None => return Ok(()),
            }
        }
    };

    let mut app = App::new(config);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Show loading state, then fetch filters
    terminal.draw(|f| ui::render(f, &app))?;
    app.load_filters().await;
    terminal.draw(|f| ui::render(f, &app))?;
    app.fetch_logs().await;

    // Main loop
    let result = run(&mut terminal, &mut app).await;

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

struct SetupState {
    url: String,
    region: String,
    active_field: usize, // 0 = URL, 1 = Region
    error_message: Option<String>,
}

fn run_setup_dialog(error: Option<&str>) -> Result<Option<AppConfig>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = SetupState {
        url: String::new(),
        region: "eu-central-1".to_string(),
        active_field: 0,
        error_message: error.map(String::from),
    };

    let result = loop {
        terminal.draw(|f| render_setup_dialog(f, &state))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Tab | KeyCode::Down => {
                        state.active_field = (state.active_field + 1) % 2;
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        state.active_field = if state.active_field == 0 { 1 } else { 0 };
                    }
                    KeyCode::Char(c) => {
                        match state.active_field {
                            0 => state.url.push(c),
                            _ => state.region.push(c),
                        }
                    }
                    KeyCode::Backspace => {
                        match state.active_field {
                            0 => { state.url.pop(); }
                            _ => { state.region.pop(); }
                        }
                    }
                    KeyCode::Enter => {
                        if !state.url.is_empty() {
                            let cfg = AppConfig {
                                endpoint_url: state.url.clone(),
                                aws_region: if state.region.is_empty() {
                                    "eu-central-1".to_string()
                                } else {
                                    state.region.clone()
                                },
                            };
                            if let Err(e) = config::save_config(&cfg) {
                                state.error_message = Some(format!("Failed to save config: {}", e));
                            } else {
                                break Some(cfg);
                            }
                        }
                    }
                    KeyCode::Esc => {
                        break None;
                    }
                    _ => {}
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(result)
}

fn render_setup_dialog(f: &mut Frame, state: &SetupState) {
    // Dark background
    f.render_widget(Block::default().style(Style::default().bg(Color::Black)), f.area());

    let area = f.area();
    let width = 60_u16.min(area.width.saturating_sub(4));
    let height = 14_u16.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Log Explorer Setup ");
    f.render_widget(block, popup);

    let inner = Rect::new(popup.x + 2, popup.y + 1, popup.width.saturating_sub(4), popup.height.saturating_sub(2));

    let mut lines: Vec<Line> = Vec::new();

    if let Some(ref err) = state.error_message {
        lines.push(Line::from(Span::styled(err.as_str(), Style::default().fg(Color::Red))));
        lines.push(Line::from(""));
    }

    // URL field
    let url_label_style = if state.active_field == 0 {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(Span::styled("OpenSearch Endpoint URL:", url_label_style)));

    let url_line = if state.active_field == 0 {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::raw(&state.url),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ])
    } else {
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::raw(&state.url),
        ])
    };
    lines.push(url_line);
    lines.push(Line::from(""));

    // Region field
    let region_label_style = if state.active_field == 1 {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(Span::styled("AWS Region:", region_label_style)));

    let region_line = if state.active_field == 1 {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::raw(&state.region),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ])
    } else {
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::raw(&state.region),
        ])
    };
    lines.push(region_line);
    lines.push(Line::from(""));

    // Help text
    lines.push(Line::from(vec![
        Span::styled(" Tab ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("switch field  "),
        Span::styled(" Enter ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("confirm  "),
        Span::styled(" Esc ", Style::default().fg(Color::Yellow).bold()),
        Span::raw("quit"),
    ]));

    let config_path = config::config_path();
    lines.push(Line::from(Span::styled(
        format!("Config: {}", config_path.display()),
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(Paragraph::new(lines), inner);
}

fn open_in_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    content: &str,
    filename: &str,
) -> Result<String> {
    let tmp = std::env::temp_dir().join(filename);
    std::fs::write(&tmp, content)?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "open".to_string());
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    let result = Command::new(&editor).arg(&tmp).status();

    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;

    match result {
        Ok(s) if s.success() => Ok("Editor closed".to_string()),
        Ok(s) => Ok(format!("Editor exited: {}", s)),
        Err(e) => Ok(format!("Failed to open editor: {}", e)),
    }
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match app.focused {
                    // --- Logs pane focused ---
                    Pane::Logs => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('P') => {
                            app.profile_filter.open();
                            app.focused = Pane::Profile;
                        }
                        KeyCode::Char('A') => {
                            app.app_filter.open();
                            app.focused = Pane::Application;
                        }
                        KeyCode::Char('S') => {
                            app.severity_filter.open();
                            app.focused = Pane::Severity;
                        }
                        KeyCode::Char('T') => {
                            app.time_filter.open();
                            app.focused = Pane::TimeRange;
                        }
                        KeyCode::Char('N') => {
                            app.limit_filter.open();
                            app.focused = Pane::Limit;
                        }
                        KeyCode::Char('R') => {
                            app.fetch_page(app.page).await;
                        }
                        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                        KeyCode::Right | KeyCode::Char('l') => {
                            app.next_page().await;
                        }
                        KeyCode::Left | KeyCode::Char('h') => {
                            app.prev_page().await;
                        }
                        KeyCode::Enter => {
                            if !app.logs.is_empty() {
                                app.context_cursor = 0;
                                app.focused = Pane::LogContext;
                            }
                        }
                        KeyCode::Char('/') => {
                            app.focused = Pane::Search;
                        }
                        KeyCode::Char('M') => {
                            app.search_mode_filter.open();
                            app.focused = Pane::SearchMode;
                        }
                        KeyCode::Char('F') => {
                            app.search_fields_filter.open();
                            app.focused = Pane::SearchFields;
                        }
                        KeyCode::Char('E') => {
                            if !app.logs.is_empty() {
                                let content: String = app.logs.iter().map(|log| {
                                    let mut line = format!("[{}] {} [{}] {}", log.timestamp, log.severity, log.logger, log.message);
                                    if !log.stacktrace.is_empty() {
                                        line.push('\n');
                                        line.push_str(&log.stacktrace);
                                    }
                                    line
                                }).collect::<Vec<_>>().join("\n");
                                app.status = open_in_editor(terminal, &content, "log_explorer_page.log")?;
                            }
                        }
                        _ => {}
                    },

                    // --- Log context menu ---
                    Pane::LogContext => match key.code {
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.context_cursor = (app.context_cursor + 1)
                                .min(CONTEXT_MENU_OPTIONS.len() - 1);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.context_cursor = app.context_cursor.saturating_sub(1);
                        }
                        KeyCode::Enter => {
                            if let Some(log) = app.logs.get(app.log_index) {
                                match app.context_cursor {
                                    0 => {
                                        let mut text = log.message.clone();
                                        if !log.stacktrace.is_empty() {
                                            text.push('\n');
                                            text.push_str(&log.stacktrace);
                                        }
                                        match Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                                            Ok(_) => app.status = "Copied to clipboard".to_string(),
                                            Err(e) => app.status = format!("Clipboard error: {}", e),
                                        }
                                    }
                                    1 => {
                                        let mut content = log.message.clone();
                                        if !log.stacktrace.is_empty() {
                                            content.push('\n');
                                            content.push_str(&log.stacktrace);
                                        }
                                        app.status = open_in_editor(terminal, &content, "log_explorer_entry.log")?;
                                    }
                                    _ => {}
                                }
                            }
                            app.focused = Pane::Logs;
                        }
                        KeyCode::Esc => {
                            app.focused = Pane::Logs;
                        }
                        _ => {}
                    },

                    // --- Search text input ---
                    Pane::Search => match key.code {
                        KeyCode::Char(c) => {
                            app.search_text.push(c);
                        }
                        KeyCode::Backspace => {
                            app.search_text.pop();
                        }
                        KeyCode::Enter => {
                            app.status = "Fetching logs...".to_string();
                            terminal.draw(|f| ui::render(f, app))?;
                            app.fetch_logs().await;
                        }
                        KeyCode::Esc => {
                            app.focused = Pane::Logs;
                        }
                        _ => {}
                    },

                    // --- Filter dropdown focused (typing mode) ---
                    Pane::Profile | Pane::Application | Pane::Severity | Pane::TimeRange | Pane::Limit | Pane::SearchMode | Pane::SearchFields => match key.code {
                        // Uppercase hotkeys always switch pane
                        KeyCode::Char('P') => {
                            app.profile_filter.open();
                            app.focused = Pane::Profile;
                        }
                        KeyCode::Char('A') => {
                            app.app_filter.open();
                            app.focused = Pane::Application;
                        }
                        KeyCode::Char('S') => {
                            app.severity_filter.open();
                            app.focused = Pane::Severity;
                        }
                        KeyCode::Char('T') => {
                            app.time_filter.open();
                            app.focused = Pane::TimeRange;
                        }
                        KeyCode::Char('L') => app.focused = Pane::Logs,
                        KeyCode::Char('/') => app.focused = Pane::Search,
                        KeyCode::Char('M') => {
                            app.search_mode_filter.open();
                            app.focused = Pane::SearchMode;
                        }
                        KeyCode::Char('F') => {
                            app.search_fields_filter.open();
                            app.focused = Pane::SearchFields;
                        }

                        // Any other character -> filter input
                        KeyCode::Char(c) => {
                            app.active_filter_mut().type_char(c);
                        }
                        KeyCode::Backspace => {
                            app.active_filter_mut().backspace();
                        }

                        KeyCode::Down => app.active_filter_mut().next(),
                        KeyCode::Up => app.active_filter_mut().previous(),

                        KeyCode::Enter => {
                            let pane = app.focused;
                            app.active_filter_mut().confirm();
                            if pane == Pane::SearchMode || pane == Pane::SearchFields {
                                app.focused = Pane::Logs;
                            } else {
                                app.status = "Fetching logs...".to_string();
                                terminal.draw(|f| ui::render(f, app))?;
                                app.fetch_logs().await;
                            }
                        }

                        KeyCode::Esc => app.focused = Pane::Logs,
                        _ => {}
                    },
                }
            }
        }
    }
}
