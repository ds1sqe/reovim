use {
    crate::check::{HealthCheck, HealthCheckResult, HealthCheckResultWithMeta},
    std::{collections::HashSet, sync::Arc},
};

#[derive(Debug, Clone)]
pub struct HealthCheckManager {
    checks: Vec<Arc<dyn HealthCheck>>,
    last_results: Option<Vec<HealthCheckResultWithMeta>>,
    visible: bool,
    selected_index: usize,
    scroll_offset: usize,
    /// Track which checks are expanded (by row index)
    expanded_rows: HashSet<usize>,
}

impl Default for HealthCheckManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCheckManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
            last_results: None,
            visible: false,
            selected_index: 0,
            scroll_offset: 0,
            expanded_rows: HashSet::new(),
        }
    }

    pub fn register_check(&mut self, check: Arc<dyn HealthCheck>) {
        tracing::debug!("Registering health check: {}", check.name());
        self.checks.push(check);
    }

    pub fn run_all_checks(&mut self) {
        tracing::info!("Running {} health checks", self.checks.len());

        let mut results = Vec::with_capacity(self.checks.len());

        for check in &self.checks {
            // Wrap in panic handler to prevent crashes
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check.run()));

            let result = result.unwrap_or_else(|_| {
                tracing::error!("Health check '{}' panicked", check.name());
                HealthCheckResult::error("Check panicked")
            });

            results.push(HealthCheckResultWithMeta {
                name: check.name().to_string(),
                category: check.category().to_string(),
                result,
            });
        }

        // Sort by category, then by name
        results.sort_by(|a, b| {
            a.category
                .cmp(&b.category)
                .then_with(|| a.name.cmp(&b.name))
        });

        self.last_results = Some(results);
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.expanded_rows.clear();
    }

    #[must_use]
    pub const fn last_results(&self) -> &Option<Vec<HealthCheckResultWithMeta>> {
        &self.last_results
    }

    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    #[must_use]
    pub const fn selected_index(&self) -> usize {
        self.selected_index
    }

    #[must_use]
    pub const fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn select_next(&mut self) {
        let Some(results) = &self.last_results else {
            return;
        };

        if results.is_empty() {
            return;
        }

        // Count total displayable rows (category headers + checks)
        let mut total_rows = 0;
        let mut current_category = None;
        for result in results {
            if current_category != Some(&result.category) {
                total_rows += 1; // Category header
                current_category = Some(&result.category);
            }
            total_rows += 1; // Check result
        }

        if total_rows == 0 {
            return;
        }

        self.selected_index = (self.selected_index + 1).min(total_rows - 1);
    }

    pub const fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Check if a row is expanded
    #[must_use]
    pub fn is_expanded(&self, row: usize) -> bool {
        self.expanded_rows.contains(&row)
    }

    /// Toggle expansion for the currently selected row
    pub fn toggle_current(&mut self) {
        tracing::info!(
            "Toggling row {}, currently {} rows expanded",
            self.selected_index,
            self.expanded_rows.len()
        );

        if self.expanded_rows.contains(&self.selected_index) {
            self.expanded_rows.remove(&self.selected_index);
            tracing::info!("Collapsed row {}", self.selected_index);
        } else {
            self.expanded_rows.insert(self.selected_index);
            tracing::info!("Expanded row {}", self.selected_index);
        }
    }

    #[must_use]
    pub fn check_count(&self) -> usize {
        self.checks.len()
    }
}
