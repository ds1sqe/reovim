//! Notification plugin events
//!
//! Events for showing/dismissing notifications and progress.

use reovim_core::{declare_event_command, event_bus::Event};

use crate::notification::NotificationLevel;

/// Event to show a notification
#[derive(Debug, Clone)]
pub struct NotificationShow {
    /// Severity level
    pub level: NotificationLevel,
    /// Message content
    pub message: String,
    /// Duration in milliseconds (None = use default)
    pub duration_ms: Option<u64>,
    /// Optional source label
    pub source: Option<String>,
}

impl Event for NotificationShow {
    fn priority(&self) -> u32 {
        100
    }
}

impl NotificationShow {
    /// Create a new info notification
    #[must_use]
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Info,
            message: message.into(),
            duration_ms: None,
            source: None,
        }
    }

    /// Create a new success notification
    #[must_use]
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Success,
            message: message.into(),
            duration_ms: None,
            source: None,
        }
    }

    /// Create a new warning notification
    #[must_use]
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Warning,
            message: message.into(),
            duration_ms: None,
            source: None,
        }
    }

    /// Create a new error notification
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: NotificationLevel::Error,
            message: message.into(),
            duration_ms: None,
            source: None,
        }
    }
}

/// Event to dismiss a specific notification
#[derive(Debug, Clone)]
pub struct NotificationDismiss {
    /// ID of the notification to dismiss
    pub id: String,
}

impl Event for NotificationDismiss {
    fn priority(&self) -> u32 {
        100
    }
}

// Dismiss all command
declare_event_command! {
    NotificationDismissAll,
    id: "notification_dismiss_all",
    description: "Dismiss all notifications",
}

/// Event to update progress
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// Unique identifier for this progress
    pub id: String,
    /// Title of the operation
    pub title: String,
    /// Source of the progress
    pub source: String,
    /// Progress percentage (0-100) or None for spinner
    pub progress: Option<u8>,
    /// Optional detail text
    pub detail: Option<String>,
}

impl Event for ProgressUpdate {
    fn priority(&self) -> u32 {
        100
    }
}

impl ProgressUpdate {
    /// Create a new progress update
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            source: source.into(),
            progress: None,
            detail: None,
        }
    }

    /// Set the progress percentage
    #[must_use]
    pub const fn with_progress(mut self, progress: u8) -> Self {
        self.progress = Some(progress);
        self
    }

    /// Set the detail text
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Event to complete a progress
#[derive(Debug, Clone)]
pub struct ProgressComplete {
    /// ID of the progress to complete
    pub id: String,
    /// Optional completion message (will show as success toast)
    pub message: Option<String>,
}

impl Event for ProgressComplete {
    fn priority(&self) -> u32 {
        100
    }
}

impl ProgressComplete {
    /// Create a new progress complete event
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            message: None,
        }
    }

    /// Set the completion message
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}
