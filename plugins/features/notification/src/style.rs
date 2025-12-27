//! Notification styles
//!
//! Plugin-local styles for notifications (not in core Theme).

use {reovim_core::highlight::Style, reovim_sys::style::Color};

use crate::notification::NotificationLevel;

/// Styles for notification rendering
#[derive(Debug, Clone)]
pub struct NotificationStyles {
    /// Info level style
    pub info: Style,
    /// Success level style
    pub success: Style,
    /// Warning level style
    pub warning: Style,
    /// Error level style
    pub error: Style,
    /// Border style
    pub border: Style,
    /// Source label style
    pub source: Style,
    /// Progress bar filled portion style
    pub progress_bar: Style,
    /// Progress bar empty portion style
    pub progress_track: Style,
}

impl Default for NotificationStyles {
    fn default() -> Self {
        Self {
            info: Style::new().fg(Color::Cyan),
            success: Style::new().fg(Color::Green),
            warning: Style::new().fg(Color::Yellow),
            error: Style::new().fg(Color::Red),
            border: Style::new().fg(Color::DarkGrey),
            source: Style::new().fg(Color::DarkGrey),
            progress_bar: Style::new().fg(Color::Blue),
            progress_track: Style::new().fg(Color::DarkGrey),
        }
    }
}

impl NotificationStyles {
    /// Get the style for a notification level
    #[must_use]
    pub const fn for_level(&self, level: NotificationLevel) -> &Style {
        match level {
            NotificationLevel::Info => &self.info,
            NotificationLevel::Success => &self.success,
            NotificationLevel::Warning => &self.warning,
            NotificationLevel::Error => &self.error,
        }
    }
}
