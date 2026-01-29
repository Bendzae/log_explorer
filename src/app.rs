use crate::opensearch::{self, LogEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Profile,
    Application,
    Logs,
}

pub struct App {
    pub focused: Pane,

    pub environments: Vec<String>,
    pub applications: Vec<String>,
    pub env_index: usize,
    pub app_index: usize,

    pub logs: Vec<LogEntry>,
    pub log_index: usize,

    pub status: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            focused: Pane::Logs,
            environments: Vec::new(),
            applications: Vec::new(),
            env_index: 0,
            app_index: 0,
            logs: Vec::new(),
            log_index: 0,
            status: "Loading filters...".to_string(),
        }
    }

    pub fn selected_env(&self) -> Option<&str> {
        self.environments.get(self.env_index).map(|s| s.as_str())
    }

    pub fn selected_app(&self) -> Option<&str> {
        self.applications.get(self.app_index).map(|s| s.as_str())
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
                self.environments = filters.environments;
                self.applications = filters.applications;
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
        self.status = format!("Fetching logs from {} ({})...", app, env);
        match opensearch::fetch_logs(&app, &env, 100).await {
            Ok(logs) => {
                self.status = format!("Loaded {} logs from {} ({})", logs.len(), app, env);
                self.logs = logs;
                self.log_index = 0;
                self.focused = Pane::Logs;
            }
            Err(e) => {
                self.status = format!("Error: {}", e);
            }
        }
    }

    pub fn next(&mut self) {
        match self.focused {
            Pane::Profile => {
                if !self.environments.is_empty() {
                    self.env_index = (self.env_index + 1).min(self.environments.len() - 1);
                }
            }
            Pane::Application => {
                if !self.applications.is_empty() {
                    self.app_index = (self.app_index + 1).min(self.applications.len() - 1);
                }
            }
            Pane::Logs => {
                if !self.logs.is_empty() {
                    self.log_index = (self.log_index + 1).min(self.logs.len() - 1);
                }
            }
        }
    }

    pub fn previous(&mut self) {
        match self.focused {
            Pane::Profile => {
                self.env_index = self.env_index.saturating_sub(1);
            }
            Pane::Application => {
                self.app_index = self.app_index.saturating_sub(1);
            }
            Pane::Logs => {
                self.log_index = self.log_index.saturating_sub(1);
            }
        }
    }
}
