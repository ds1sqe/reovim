//! Progress notification types
//!
//! Defines progress bars and spinners for long-running operations.

/// A progress notification
#[derive(Debug, Clone)]
pub struct ProgressNotification {
    /// Unique identifier
    pub id: String,
    /// Title of the operation (e.g., "Building", "Indexing")
    pub title: String,
    /// Source of the progress (e.g., "cargo", "rust-analyzer")
    pub source: String,
    /// Progress percentage (0-100) or None for indeterminate spinner
    pub progress: Option<u8>,
    /// Optional detail text (e.g., "4/250 (core)")
    pub detail: Option<String>,
    /// Whether the operation is complete
    pub complete: bool,
}

impl ProgressNotification {
    /// Create a new progress notification
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            source: source.into(),
            progress: None,
            detail: None,
            complete: false,
        }
    }

    /// Set the progress percentage
    #[must_use]
    pub fn with_progress(mut self, progress: u8) -> Self {
        self.progress = Some(progress.min(100));
        self
    }

    /// Set the detail text
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Mark as complete
    #[must_use]
    pub const fn completed(mut self) -> Self {
        self.complete = true;
        self.progress = Some(100);
        self
    }

    /// Check if this is an indeterminate (spinner) progress
    #[must_use]
    pub const fn is_indeterminate(&self) -> bool {
        self.progress.is_none()
    }
}

/// Configuration for progress bar rendering
#[derive(Debug, Clone)]
pub struct ProgressBarConfig {
    /// Width in characters
    pub width: u16,
    /// Character for filled portion
    pub filled_char: char,
    /// Character for empty portion
    pub empty_char: char,
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            width: 20,
            filled_char: '█',
            empty_char: '░',
        }
    }
}

impl ProgressBarConfig {
    /// Render a progress bar string
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn render(&self, progress: Option<u8>) -> String {
        progress.map_or_else(
            || "⣷⣯⣟⡿⢿⣻⣽⣾".chars().next().unwrap_or('◌').to_string(),
            |pct| {
                let filled = (u16::from(pct.min(100)) * self.width / 100) as usize;
                let empty = (self.width as usize).saturating_sub(filled);
                format!(
                    "{}{}",
                    self.filled_char.to_string().repeat(filled),
                    self.empty_char.to_string().repeat(empty)
                )
            },
        )
    }
}
