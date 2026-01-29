mod app;
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
                match key.code {
                    KeyCode::Char('q') => return Ok(()),

                    // Pane focus hotkeys (uppercase)
                    KeyCode::Char('P') => app.focused = Pane::Profile,
                    KeyCode::Char('A') => app.focused = Pane::Application,
                    KeyCode::Char('L') => app.focused = Pane::Logs,

                    // Navigation
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),

                    // Confirm selection in filter dropdown -> fetch logs
                    KeyCode::Enter => {
                        if matches!(app.focused, Pane::Profile | Pane::Application) {
                            app.status = "Fetching logs...".to_string();
                            terminal.draw(|f| ui::render(f, app))?;
                            app.fetch_logs().await;
                        }
                    }

                    // Dismiss filter dropdown
                    KeyCode::Esc => {
                        if matches!(app.focused, Pane::Profile | Pane::Application) {
                            app.focused = Pane::Logs;
                        }
                    }

                    _ => {}
                }
            }
        }
    }
}
