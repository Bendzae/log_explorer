use crate::filter_field::FilterField;
use crate::opensearch::{self, LogEntry};

const ALL: &str = "ALL";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Profile,
    Application,
    Severity,
    TimeRange,
    Limit,
    Search,
    SearchMode,
    Logs,
    LogContext,
}

pub const CONTEXT_MENU_OPTIONS: &[&str] = &["Copy to clipboard", "Open in editor"];

pub struct App {
    pub focused: Pane,

    pub profile_filter: FilterField,
    pub app_filter: FilterField,
    pub severity_filter: FilterField,
    pub time_filter: FilterField,
    pub limit_filter: FilterField,
    pub search_text: String,
    pub search_mode_filter: FilterField,

    pub logs: Vec<LogEntry>,
    pub log_index: usize,
    pub total_hits: u64,
    pub page: u64,
    pub context_cursor: usize,

    pub status: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            focused: Pane::Logs,
            profile_filter: FilterField::new(),
            app_filter: FilterField::new(),
            severity_filter: FilterField::new(),
            time_filter: FilterField::new(),
            limit_filter: FilterField::new(),
            search_text: String::new(),
            search_mode_filter: {
                let mut f = FilterField::new();
                f.set_items(vec!["Each word".to_string(), "Exact".to_string()]);
                f
            },
            logs: Vec::new(),
            log_index: 0,
            total_hits: 0,
            page: 1,
            context_cursor: 0,
            status: "Loading filters...".to_string(),
        }
    }

    pub fn selected_env(&self) -> Option<&str> {
        self.profile_filter.selected_value()
    }

    pub fn selected_app(&self) -> Option<&str> {
        self.app_filter.selected_value().filter(|v| *v != ALL)
    }

    pub fn selected_severity(&self) -> Option<&str> {
        self.severity_filter
            .selected_value()
            .filter(|v| *v != ALL)
    }

    pub fn selected_time_range(&self) -> &str {
        self.time_filter
            .selected_value()
            .map(|v| match v {
                "1m" => "now-1m",
                "5m" => "now-5m",
                "15m" => "now-15m",
                "30m" => "now-30m",
                "1h" => "now-1h",
                "3h" => "now-3h",
                "6h" => "now-6h",
                "12h" => "now-12h",
                "24h" => "now-24h",
                "3d" => "now-3d",
                "7d" => "now-7d",
                _ => "now-5m",
            })
            .unwrap_or("now-5m")
    }

    pub fn selected_limit(&self) -> i64 {
        self.limit_filter
            .selected_value()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100)
    }

    pub fn search_exact(&self) -> bool {
        self.search_mode_filter.selected_value() == Some("Exact")
    }

    pub fn total_pages(&self) -> u64 {
        let limit = self.selected_limit() as u64;
        if limit == 0 {
            return 1;
        }
        self.total_hits.div_ceil(limit).max(1)
    }

    pub fn active_filter_mut(&mut self) -> &mut FilterField {
        match self.focused {
            Pane::Profile => &mut self.profile_filter,
            Pane::Application => &mut self.app_filter,
            Pane::Severity => &mut self.severity_filter,
            Pane::TimeRange => &mut self.time_filter,
            Pane::Limit => &mut self.limit_filter,
            Pane::SearchMode => &mut self.search_mode_filter,
            Pane::Search | Pane::Logs | Pane::LogContext => unreachable!("active_filter_mut called while Search/Logs/LogContext is focused"),
        }
    }

    pub async fn load_filters(&mut self) {
        self.status = "Fetching available filters...".to_string();
        match opensearch::fetch_available_filters().await {
            Ok(filters) => {
                self.status = format!(
                    "{} environments, {} applications — select filters and press Enter",
                    filters.environments.len(),
                    filters.applications.len()
                );
                let environments: Vec<String> = filters.environments.into_iter()
                    .filter(|e| e != "ACTIVE_PROFILE_IS_UNDEFINED")
                    .collect();
                self.profile_filter.set_items(environments);
                self.profile_filter.select_value("production");

                let mut applications = vec![ALL.to_string()];
                applications.extend(
                    filters.applications.into_iter()
                        .filter(|a| a != "APPLICATION_NAME_IS_UNDEFINED"),
                );
                self.app_filter.set_items(applications);

                let mut severities = vec![ALL.to_string()];
                severities.extend(filters.severities);
                self.severity_filter.set_items(severities);

                let time_ranges: Vec<String> =
                    ["1m", "5m", "15m", "30m", "1h", "3h", "6h", "12h", "24h", "3d", "7d"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                self.time_filter.set_items(time_ranges);
                self.time_filter.select_value("5m");

                let limits: Vec<String> = ["50", "100", "200", "500", "1000"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                self.limit_filter.set_items(limits);
                self.limit_filter.select_value("50");
            }
            Err(e) => {
                self.status = format!("Error loading filters: {}", e);
            }
        }
    }

    pub async fn fetch_logs(&mut self) {
        self.fetch_page(1).await;
    }

    pub async fn fetch_page(&mut self, page: u64) {
        let Some(env) = self.selected_env().map(str::to_owned) else {
            self.status = "No environment selected".to_string();
            return;
        };
        let app = self.selected_app().map(str::to_owned);
        let severity = self.selected_severity().map(str::to_owned);
        let time_range = self.selected_time_range().to_owned();
        let limit = self.selected_limit();
        let from = (page - 1) as i64 * limit;
        let app_label = app.as_deref().unwrap_or("ALL");
        let label = match &severity {
            Some(sev) => format!("{} ({}) [{}]", app_label, env, sev),
            None => format!("{} ({})", app_label, env),
        };
        let search = if self.search_text.is_empty() { None } else { Some(self.search_text.as_str()) };
        let search_exact = self.search_exact();
        self.status = format!("Fetching page {} from {}...", page, label);
        match opensearch::fetch_logs(app.as_deref(), &env, severity.as_deref(), &time_range, search, search_exact, limit, from).await
        {
            Ok(result) => {
                self.status = format!("Loaded {} logs from {}", result.logs.len(), label);
                self.total_hits = result.total;
                self.page = page;
                self.logs = result.logs;
                self.log_index = 0;
                self.focused = Pane::Logs;
            }
            Err(e) => {
                self.status = format!("Error: {}", e);
            }
        }
    }

    pub async fn next_page(&mut self) {
        if self.page < self.total_pages() {
            self.fetch_page(self.page + 1).await;
        }
    }

    pub async fn prev_page(&mut self) {
        if self.page > 1 {
            self.fetch_page(self.page - 1).await;
        }
    }

    pub fn scroll_down(&mut self) {
        if !self.logs.is_empty() {
            self.log_index = (self.log_index + 1).min(self.logs.len() - 1);
        }
    }

    pub fn scroll_up(&mut self) {
        self.log_index = self.log_index.saturating_sub(1);
    }
}
