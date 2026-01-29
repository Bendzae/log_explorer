use crate::filter_field::FilterField;
use crate::opensearch::{self, LogEntry};

const ALL: &str = "ALL";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Profile,
    Application,
    Severity,
    Logs,
}

pub struct App {
    pub focused: Pane,

    pub profile_filter: FilterField,
    pub app_filter: FilterField,
    pub severity_filter: FilterField,

    pub logs: Vec<LogEntry>,
    pub log_index: usize,

    pub status: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            focused: Pane::Logs,
            profile_filter: FilterField::new(),
            app_filter: FilterField::new(),
            severity_filter: FilterField::new(),
            logs: Vec::new(),
            log_index: 0,
            status: "Loading filters...".to_string(),
        }
    }

    pub fn selected_env(&self) -> Option<&str> {
        self.profile_filter.selected_value()
    }

    pub fn selected_app(&self) -> Option<&str> {
        self.app_filter.selected_value()
    }

    pub fn selected_severity(&self) -> Option<&str> {
        self.severity_filter
            .selected_value()
            .filter(|v| *v != ALL)
    }

    pub fn active_filter_mut(&mut self) -> &mut FilterField {
        match self.focused {
            Pane::Profile => &mut self.profile_filter,
            Pane::Application => &mut self.app_filter,
            Pane::Severity => &mut self.severity_filter,
            Pane::Logs => unreachable!("active_filter_mut called while Logs is focused"),
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
                self.profile_filter.set_items(filters.environments);
                self.app_filter.set_items(filters.applications);

                let mut severities = vec![ALL.to_string()];
                severities.extend(filters.severities);
                self.severity_filter.set_items(severities);
            }
            Err(e) => {
                self.status = format!("Error loading filters: {}", e);
            }
        }
    }

    pub async fn fetch_logs(&mut self) {
        let Some(env) = self.selected_env().map(str::to_owned) else {
            self.status = "No environment selected".to_string();
            return;
        };
        let Some(app) = self.selected_app().map(str::to_owned) else {
            self.status = "No application selected".to_string();
            return;
        };
        let severity = self.selected_severity().map(str::to_owned);
        let label = match &severity {
            Some(sev) => format!("{} ({}) [{}]", app, env, sev),
            None => format!("{} ({})", app, env),
        };
        self.status = format!("Fetching logs from {}...", label);
        match opensearch::fetch_logs(&app, &env, severity.as_deref(), 100).await {
            Ok(logs) => {
                self.status = format!("Loaded {} logs from {}", logs.len(), label);
                self.logs = logs;
                self.log_index = 0;
                self.focused = Pane::Logs;
            }
            Err(e) => {
                self.status = format!("Error: {}", e);
            }
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
