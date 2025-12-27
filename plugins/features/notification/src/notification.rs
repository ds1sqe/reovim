//! Notification types
//!
//! Defines notification levels and structure.

use std::time::Instant;

/// Notification severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationLevel {
    /// Informational message
    #[default]
    Info,
    /// Success/completion message
    Success,
    /// Warning message
    Warning,
    /// Error message
    Error,
}

impl NotificationLevel {
    /// Get display icon for the level
    #[must_use]
    pub const fn icon(&self) -> &'static str {
        match self {
            Self::Info => "󰋽 ",
            Self::Success => "󰄬 ",
            Self::Warning => "󰀦 ",
            Self::Error => "󰅚 ",
        }
    }
}

/// A single notification
#[derive(Debug, Clone)]
pub struct Notification {
    /// Unique identifier
    pub id: String,
    /// Severity level
    pub level: NotificationLevel,
    /// Message content
    pub message: String,
    /// Duration to display in milliseconds
    pub duration_ms: u64,
    /// When the notification was created
    pub created_at: Instant,
    /// Optional source (e.g., "LSP", "rust-analyzer")
    pub source: Option<String>,
}

impl Notification {
    /// Create a new notification
    #[must_use]
    pub fn new(level: NotificationLevel, message: impl Into<String>) -> Self {
        Self {
            id: format!("{:?}", Instant::now()),
            level,
            message: message.into(),
            duration_ms: 3000,
            created_at: Instant::now(),
            source: None,
        }
    }

    /// Set the duration
    #[must_use]
    pub const fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Set the source
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Check if this notification has expired
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_millis() as u64 >= self.duration_ms
    }
}
