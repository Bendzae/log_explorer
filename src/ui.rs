use crate::app::{App, Pane, CONTEXT_MENU_OPTIONS};
use crate::filter_field::FilterField;
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
            render_dropdown(f, chunks[0], chunks[1], 0, &app.profile_filter);
        }
        Pane::Application => {
            render_dropdown(f, chunks[0], chunks[1], 1, &app.app_filter);
        }
        Pane::Severity => {
            render_dropdown(f, chunks[0], chunks[1], 2, &app.severity_filter);
        }
        Pane::TimeRange => {
            render_dropdown(f, chunks[0], chunks[1], 3, &app.time_filter);
        }
        Pane::Limit => {
            render_dropdown(f, chunks[0], chunks[1], 4, &app.limit_filter);
        }
        Pane::SearchMode => {
            render_dropdown(f, chunks[0], chunks[1], 6, &app.search_mode_filter);
        }
        Pane::Search | Pane::Logs => {}
        Pane::LogContext => {
            render_log_context_menu(f, chunks[1], app);
        }
    }
}

// --- Filter bar (collapsed) ---

const FILTER_CONSTRAINTS: [Constraint; 7] = [
    Constraint::Length(25),
    Constraint::Length(30),
    Constraint::Length(18),
    Constraint::Length(20),
    Constraint::Length(16),
    Constraint::Fill(1),
    Constraint::Length(18),
];

fn render_filter_bar(f: &mut Frame, area: Rect, app: &App) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(FILTER_CONSTRAINTS)
        .split(area);

    render_filter_chip(
        f,
        panes[0],
        "Profile",
        'P',
        app.focused == Pane::Profile,
        app.profile_filter.selected_value().unwrap_or("—"),
    );
    render_filter_chip(
        f,
        panes[1],
        "Application",
        'A',
        app.focused == Pane::Application,
        app.app_filter.selected_value().unwrap_or("—"),
    );
    render_filter_chip(
        f,
        panes[2],
        "Severity",
        'S',
        app.focused == Pane::Severity,
        app.severity_filter.selected_value().unwrap_or("—"),
    );
    render_filter_chip(
        f,
        panes[3],
        "Time Range",
        'T',
        app.focused == Pane::TimeRange,
        app.time_filter.selected_value().unwrap_or("—"),
    );
    render_filter_chip(
        f,
        panes[4],
        "Limit",
        'N',
        app.focused == Pane::Limit,
        app.limit_filter.selected_value().unwrap_or("—"),
    );
    render_search_chip(f, panes[5], app);
    render_filter_chip(
        f,
        panes[6],
        "Mode",
        'M',
        app.focused == Pane::SearchMode,
        app.search_mode_filter.selected_value().unwrap_or("—"),
    );
}

fn render_filter_chip(
    f: &mut Frame,
    area: Rect,
    name: &str,
    hotkey: char,
    focused: bool,
    value: &str,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(focused))
        .title(pane_title(name, hotkey, focused));
    let widget = Paragraph::new(format!(" {}", value)).block(block);
    f.render_widget(widget, area);
}

fn render_search_chip(f: &mut Frame, area: Rect, app: &App) {
    let focused = app.focused == Pane::Search;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(focused))
        .title(pane_title("Search", '/', focused));

    let content = if focused {
        Line::from(vec![
            Span::raw(format!(" {}", app.search_text)),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ])
    } else if app.search_text.is_empty() {
        Line::from(Span::styled(" —", Style::default().fg(Color::DarkGray)))
    } else {
        Line::from(format!(" {}", app.search_text))
    };

    f.render_widget(Paragraph::new(content).block(block), area);
}

// --- Filter dropdown popup ---

fn render_dropdown(
    f: &mut Frame,
    filter_area: Rect,
    logs_area: Rect,
    pane_index: u16,
    field: &FilterField,
) {
    let filtered = field.filtered_items();
    if filtered.is_empty() && field.filter_text().is_empty() {
        return;
    }

    let filter_panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(FILTER_CONSTRAINTS)
        .split(filter_area);

    let anchor = filter_panes[pane_index as usize];
    let width = anchor.width.max(20);
    let max_height = logs_area.height.saturating_sub(1);
    // +3 = borders (2) + search input row (1)
    let height = (filtered.len() as u16 + 3).min(max_height).max(4);

    // Clamp so popup doesn't extend past the right edge of the screen
    let right_edge = logs_area.x + logs_area.width;
    let x = anchor.x.min(right_edge.saturating_sub(width));
    let clamped_width = width.min(right_edge.saturating_sub(x));

    let popup = Rect::new(x, logs_area.y, clamped_width, height);
    f.render_widget(Clear, popup);

    // Split popup: search row + list
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .margin(1)
        .split(popup);

    // Outer border
    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(border, popup);

    // Search input row
    let search_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Yellow)),
        Span::raw(field.filter_text()),
        Span::styled("█", Style::default().fg(Color::Cyan)),
    ]);
    f.render_widget(Paragraph::new(search_line), inner[0]);

    // Filtered items list
    let list_items: Vec<ListItem> = filtered
        .iter()
        .map(|&i| ListItem::new(i))
        .collect();
    let list = List::new(list_items)
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .bold(),
        )
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always);

    let mut state = ListState::default().with_selected(Some(field.cursor()));
    f.render_stateful_widget(list, inner[1], &mut state);
}

// --- Logs table ---

fn render_logs_table(f: &mut Frame, area: Rect, app: &App) {
    let logs_focused = app.focused == Pane::Logs;

    let header = Row::new(vec![
        Cell::from("Timestamp").style(Style::default().bold()),
        Cell::from("Level").style(Style::default().bold()),
        Cell::from("Logger").style(Style::default().bold()),
        Cell::from("Message").style(Style::default().bold()),
        Cell::from("ST").style(Style::default().bold()),
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

            let message_cell = Cell::from(highlight_matches(&log.message, &app.search_text));

            let stacktrace_mark = if log.stacktrace.is_empty() { "" } else { "✘" };

            Row::new(vec![
                Cell::from(time),
                Cell::from(log.severity.clone()).style(severity_style),
                Cell::from(short_logger.to_string()),
                message_cell,
                Cell::from(stacktrace_mark).style(Style::default().fg(Color::Red)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(7),
            Constraint::Length(35),
            Constraint::Fill(1),
            Constraint::Length(4),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(logs_focused))
            .title(pane_title("Logs", 'L', logs_focused)),
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray))
    .highlight_symbol("▶ ");

    let mut state = TableState::default().with_selected(Some(app.log_index));
    f.render_stateful_widget(table, area, &mut state);
}

// --- Status bar ---

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mut spans: Vec<Span> = Vec::new();

    for (key, desc) in [
        ("P", "profile"),
        ("A", "application"),
        ("S", "severity"),
        ("T", "time"),
        ("N", "limit"),
        ("L", "logs"),
    ] {
        spans.push(Span::styled(
            format!(" {} ", key),
            Style::default().fg(Color::Yellow).bold(),
        ));
        spans.push(Span::raw(format!("{}  ", desc)));
    }

    for (key, desc) in [
        ("↑↓/jk", "navigate"),
        ("←→/hl", "page"),
        ("R", "refresh"),
        ("Enter", "select"),
        ("Esc", "back"),
        ("q", "quit"),
    ] {
        spans.push(Span::styled(
            format!(" {} ", key),
            Style::default().fg(Color::Yellow).bold(),
        ));
        spans.push(Span::raw(format!("{}  ", desc)));
    }

    spans.push(Span::styled("│ ", Style::default().fg(Color::DarkGray)));
    spans.push(Span::raw(&app.status));

    let position = if app.total_hits == 0 {
        " 0/0 ".to_string()
    } else {
        format!(
            " Page {}/{} ({}/{}) ",
            app.page,
            app.total_pages(),
            app.logs.len(),
            app.total_hits
        )
    };

    let bar = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Line::from(position).right_aligned()),
    );
    f.render_widget(bar, area);
}

// --- Log context menu popup ---

fn render_log_context_menu(f: &mut Frame, logs_area: Rect, app: &App) {
    let width = 24_u16;
    let height = (CONTEXT_MENU_OPTIONS.len() as u16 + 2).min(logs_area.height);

    let x = logs_area.x + (logs_area.width.saturating_sub(width)) / 2;
    let y = logs_area.y + (logs_area.height.saturating_sub(height)) / 2;

    let popup = Rect::new(x, y, width, height);
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = CONTEXT_MENU_OPTIONS
        .iter()
        .map(|&opt| ListItem::new(opt))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Actions "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .bold(),
        )
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always);

    let mut state = ListState::default().with_selected(Some(app.context_cursor));
    f.render_stateful_widget(list, popup, &mut state);
}

// --- Text highlighting ---

fn highlight_matches<'a>(text: &'a str, query: &str) -> Line<'a> {
    if query.is_empty() {
        return Line::from(text);
    }

    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();
    let highlight = Style::default().fg(Color::Black).bg(Color::Yellow).bold();

    let mut spans = Vec::new();
    let mut pos = 0;

    while let Some(match_start) = lower_text[pos..].find(&lower_query) {
        let abs_start = pos + match_start;
        let abs_end = abs_start + query.len();

        if abs_start > pos {
            spans.push(Span::raw(&text[pos..abs_start]));
        }
        spans.push(Span::styled(&text[abs_start..abs_end], highlight));
        pos = abs_end;
    }

    if pos < text.len() {
        spans.push(Span::raw(&text[pos..]));
    }

    Line::from(spans)
}

// --- Shared helpers ---

fn pane_title(name: &str, hotkey: char, focused: bool) -> Line<'static> {
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
