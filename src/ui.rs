use crate::app::{App, Pane};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Cell, Clear, HighlightSpacing, List, ListItem, ListState, Paragraph, Row,
    Table, TableState,
};

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // filter bar
            Constraint::Min(5),   // logs table
            Constraint::Length(3), // status bar
        ])
        .split(f.area());

    render_filter_bar(f, chunks[0], app);
    render_logs_table(f, chunks[1], app);
    render_status_bar(f, chunks[2], app);

    // Render dropdown popup if a filter pane is focused
    match app.focused {
        Pane::Profile => {
            render_dropdown(f, chunks[0], chunks[1], 0, &app.environments, app.env_index);
        }
        Pane::Application => {
            render_dropdown(f, chunks[0], chunks[1], 1, &app.applications, app.app_index);
        }
        Pane::Logs => {}
    }
}

fn render_filter_bar(f: &mut Frame, area: Rect, app: &App) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(25), Constraint::Fill(1)])
        .split(area);

    // Profile filter
    let profile_focused = app.focused == Pane::Profile;
    let profile_value = app.selected_env().unwrap_or("—");
    let profile_block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(profile_focused))
        .title(filter_title("Profile", 'P', profile_focused));
    let profile = Paragraph::new(format!(" {}", profile_value)).block(profile_block);
    f.render_widget(profile, panes[0]);

    // Application filter
    let app_focused = app.focused == Pane::Application;
    let app_value = app.selected_app().unwrap_or("—");
    let app_block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(app_focused))
        .title(filter_title("Application", 'A', app_focused));
    let application = Paragraph::new(format!(" {}", app_value)).block(app_block);
    f.render_widget(application, panes[1]);
}

fn filter_title(name: &str, hotkey: char, focused: bool) -> Line<'static> {
    let style = if focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let hotkey_style = if focused {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default().fg(Color::Yellow)
    };
    Line::from(vec![
        Span::styled(format!(" {} [", name), style),
        Span::styled(hotkey.to_string(), hotkey_style),
        Span::styled("] ", style),
    ])
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn render_dropdown(
    f: &mut Frame,
    filter_area: Rect,
    logs_area: Rect,
    pane_index: u16,
    items: &[String],
    selected: usize,
) {
    if items.is_empty() {
        return;
    }

    // Calculate position: dropdown appears below the filter, overlaying the logs area
    let filter_panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(25), Constraint::Fill(1)])
        .split(filter_area);

    let anchor = filter_panes[pane_index as usize];
    let width = anchor.width.max(20);
    let max_height = logs_area.height.saturating_sub(1);
    let height = (items.len() as u16 + 2).min(max_height).max(3); // +2 for borders

    let popup = Rect::new(anchor.x, logs_area.y, width, height);

    f.render_widget(Clear, popup);

    let list_items: Vec<ListItem> = items.iter().map(|i| ListItem::new(i.as_str())).collect();
    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .bold(),
        )
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always);

    let mut state = ListState::default().with_selected(Some(selected));
    f.render_stateful_widget(list, popup, &mut state);
}

fn render_logs_table(f: &mut Frame, area: Rect, app: &App) {
    let logs_focused = app.focused == Pane::Logs;

    let header = Row::new(vec![
        Cell::from("Timestamp").style(Style::default().bold()),
        Cell::from("Level").style(Style::default().bold()),
        Cell::from("Logger").style(Style::default().bold()),
        Cell::from("Message").style(Style::default().bold()),
    ])
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .logs
        .iter()
        .map(|log| {
            let severity_style = match log.severity.as_str() {
                "ERROR" => Style::default().fg(Color::Red).bold(),
                "WARN" => Style::default().fg(Color::Yellow),
                "INFO" => Style::default().fg(Color::Green),
                "DEBUG" => Style::default().fg(Color::Blue),
                _ => Style::default(),
            };

            let short_logger = log.logger.rsplit('.').next().unwrap_or(&log.logger);

            let time = log
                .timestamp
                .find('T')
                .and_then(|t_pos| {
                    let after_t = &log.timestamp[t_pos + 1..];
                    let end = after_t
                        .find('+')
                        .or_else(|| after_t.rfind('-'))
                        .unwrap_or(after_t.len());
                    Some(after_t[..end.min(12)].to_string())
                })
                .unwrap_or_else(|| log.timestamp.clone());

            Row::new(vec![
                Cell::from(time),
                Cell::from(log.severity.clone()).style(severity_style),
                Cell::from(short_logger.to_string()),
                Cell::from(log.message.clone()),
            ])
        })
        .collect();

    let title_style = if logs_focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let hotkey_style = if logs_focused {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default().fg(Color::Yellow)
    };
    let title = Line::from(vec![
        Span::styled(" Logs [", title_style),
        Span::styled("L", hotkey_style),
        Span::styled("] ", title_style),
    ]);

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(7),
            Constraint::Length(35),
            Constraint::Fill(1),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(logs_focused))
            .title(title),
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray))
    .highlight_symbol("▶ ");

    let mut state = TableState::default().with_selected(Some(app.log_index));
    f.render_stateful_widget(table, area, &mut state);
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mut spans: Vec<Span> = Vec::new();

    for (key, desc) in [("P", "profile"), ("A", "application"), ("L", "logs")] {
        spans.push(Span::styled(
            format!(" {} ", key),
            Style::default().fg(Color::Yellow).bold(),
        ));
        spans.push(Span::raw(format!("{}  ", desc)));
    }

    spans.push(Span::styled(
        " ↑↓/jk ",
        Style::default().fg(Color::Yellow).bold(),
    ));
    spans.push(Span::raw("navigate  "));
    spans.push(Span::styled(
        " Enter ",
        Style::default().fg(Color::Yellow).bold(),
    ));
    spans.push(Span::raw("select  "));
    spans.push(Span::styled(
        " Esc ",
        Style::default().fg(Color::Yellow).bold(),
    ));
    spans.push(Span::raw("back  "));
    spans.push(Span::styled(
        " q ",
        Style::default().fg(Color::Yellow).bold(),
    ));
    spans.push(Span::raw("quit  "));

    spans.push(Span::styled("│ ", Style::default().fg(Color::DarkGray)));
    spans.push(Span::raw(&app.status));

    let bar = Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::ALL));
    f.render_widget(bar, area);
}
