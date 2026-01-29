/// Reusable filterable dropdown field.
///
/// Holds a list of items, a type-to-filter search string, and tracks both
/// the confirmed selection (shown when closed) and the cursor position
/// within the filtered results (shown when open).
pub struct FilterField {
    items: Vec<String>,
    /// Index into `items` of the confirmed (committed) selection.
    selected_index: usize,
    /// Current search/filter text typed by the user.
    filter_text: String,
    /// Indices into `items` that match `filter_text`.
    filtered_indices: Vec<usize>,
    /// Cursor position within `filtered_indices`.
    cursor: usize,
}

impl FilterField {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected_index: 0,
            filter_text: String::new(),
            filtered_indices: Vec::new(),
            cursor: 0,
        }
    }

    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.selected_index = 0;
        self.refilter();
    }

    /// Select the item matching `value`, if present.
    pub fn select_value(&mut self, value: &str) {
        if let Some(idx) = self.items.iter().position(|item| item == value) {
            self.selected_index = idx;
        }
    }

    /// The confirmed/committed value shown in the filter bar.
    pub fn selected_value(&self) -> Option<&str> {
        self.items.get(self.selected_index).map(|s| s.as_str())
    }

    /// Called when the dropdown opens: reset filter, position cursor on current selection.
    pub fn open(&mut self) {
        self.filter_text.clear();
        self.refilter();
        self.cursor = self
            .filtered_indices
            .iter()
            .position(|&i| i == self.selected_index)
            .unwrap_or(0);
    }

    /// Commit the currently highlighted item as the confirmed selection.
    pub fn confirm(&mut self) {
        if let Some(&idx) = self.filtered_indices.get(self.cursor) {
            self.selected_index = idx;
        }
    }

    pub fn type_char(&mut self, c: char) {
        self.filter_text.push(c);
        self.refilter();
    }

    pub fn backspace(&mut self) {
        self.filter_text.pop();
        self.refilter();
    }

    pub fn next(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.cursor = (self.cursor + 1).min(self.filtered_indices.len() - 1);
        }
    }

    pub fn previous(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    pub fn filtered_items(&self) -> Vec<&str> {
        self.filtered_indices
            .iter()
            .map(|&i| self.items[i].as_str())
            .collect()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn refilter(&mut self) {
        let query = self.filter_text.to_lowercase();
        self.filtered_indices = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| query.is_empty() || item.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        if self.filtered_indices.is_empty() {
            self.cursor = 0;
        } else {
            self.cursor = self.cursor.min(self.filtered_indices.len() - 1);
        }
    }
}
