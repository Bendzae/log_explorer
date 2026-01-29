mod app;
mod filter_field;
mod opensearch;
mod ui;

use anyhow::Result;
use app::{App, Pane};
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use std::io;

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

    // Main loop
    let result = run(&mut terminal, &mut app).await;

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
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
                        KeyCode::Char('N') => {
                            app.limit_filter.open();
                            app.focused = Pane::Limit;
                        }
                        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                        _ => {}
                    },

                    // --- Filter dropdown focused (typing mode) ---
                    Pane::Profile | Pane::Application | Pane::Severity | Pane::Limit => match key.code {
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
