mod app;
mod filter_field;
mod opensearch;
mod ui;

use anyhow::Result;
use app::{App, Pane, CONTEXT_MENU_OPTIONS};
use arboard::Clipboard;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use std::io;
use std::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::new();

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
                        KeyCode::Char('E') => {
                            if !app.logs.is_empty() {
                                let content: String = app.logs.iter().map(|log| {
                                    format!("[{}] {} [{}] {}", log.timestamp, log.severity, log.logger, log.message)
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
                                        match Clipboard::new().and_then(|mut cb| cb.set_text(log.message.clone())) {
                                            Ok(_) => app.status = "Copied to clipboard".to_string(),
                                            Err(e) => app.status = format!("Clipboard error: {}", e),
                                        }
                                    }
                                    1 => {
                                        app.status = open_in_editor(terminal, &log.message, "log_explorer_entry.log")?;
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

                    // --- Filter dropdown focused (typing mode) ---
                    Pane::Profile | Pane::Application | Pane::Severity | Pane::TimeRange | Pane::Limit => match key.code {
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
                            app.active_filter_mut().confirm();
                            app.status = "Fetching logs...".to_string();
                            terminal.draw(|f| ui::render(f, app))?;
                            app.fetch_logs().await;
                        }

                        KeyCode::Esc => app.focused = Pane::Logs,
                        _ => {}
                    },
                }
            }
        }
    }
}
