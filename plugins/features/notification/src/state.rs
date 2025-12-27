//! Notification state management
//!
//! Provides thread-safe storage for notifications and progress.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, RwLock},
};

use crate::{
    notification::{Notification, NotificationLevel},
    progress::{ProgressBarConfig, ProgressNotification},
    style::NotificationStyles,
};

/// Position for notification display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationPosition {
    /// Top-right corner
    TopRight,
    /// Top-left corner
    TopLeft,
    /// Bottom-right corner (default - less intrusive for coding)
    #[default]
    BottomRight,
    /// Bottom-left corner
    BottomLeft,
    /// Top-center
    TopCenter,
    /// Bottom-center
    BottomCenter,
}

/// Configuration for notifications
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    /// Default display duration in milliseconds
    pub default_duration_ms: u64,
    /// Maximum number of notifications to display
    pub max_notifications: usize,
    /// Position on screen
    pub position: NotificationPosition,
    /// Progress bar configuration
    pub progress_bar: ProgressBarConfig,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            default_duration_ms: 3000,
            max_notifications: 5,
            position: NotificationPosition::BottomRight,
            progress_bar: ProgressBarConfig::default(),
        }
    }
}

/// Inner state for notification manager
#[derive(Default)]
struct NotificationManagerInner {
    notifications: VecDeque<Notification>,
    progress: HashMap<String, ProgressNotification>,
    config: NotificationConfig,
    styles: NotificationStyles,
}

/// Thread-safe notification manager
pub struct SharedNotificationManager {
    inner: RwLock<NotificationManagerInner>,
}

impl Default for SharedNotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedNotificationManager {
    /// Create a new notification manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(NotificationManagerInner::default()),
        }
    }

    /// Show a notification
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[allow(clippy::significant_drop_tightening)]
    pub fn show(
        &self,
        level: NotificationLevel,
        message: String,
        duration_ms: Option<u64>,
        source: Option<String>,
    ) {
        let mut inner = self.inner.write().unwrap();
        let duration = duration_ms.unwrap_or(inner.config.default_duration_ms);
        let max = inner.config.max_notifications;

        let mut notification = Notification::new(level, message).with_duration(duration);
        if let Some(src) = source {
            notification = notification.with_source(src);
        }

        inner.notifications.push_front(notification);

        // Trim to max
        while inner.notifications.len() > max {
            inner.notifications.pop_back();
        }
    }

    /// Dismiss a notification by ID
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn dismiss(&self, id: &str) {
        let mut inner = self.inner.write().unwrap();
        inner.notifications.retain(|n| n.id != id);
    }

    /// Dismiss all notifications
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn dismiss_all(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.notifications.clear();
    }

    /// Update a progress notification
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn update_progress(
        &self,
        id: String,
        title: String,
        source: String,
        progress: Option<u8>,
        detail: Option<String>,
    ) {
        let mut inner = self.inner.write().unwrap();

        let mut notification = ProgressNotification::new(&id, title, source);
        if let Some(pct) = progress {
            notification = notification.with_progress(pct);
        }
        if let Some(det) = detail {
            notification = notification.with_detail(det);
        }

        inner.progress.insert(id, notification);
    }

    /// Complete a progress notification
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[allow(clippy::significant_drop_tightening)]
    pub fn complete_progress(&self, id: &str, message: Option<String>) {
        let mut inner = self.inner.write().unwrap();
        let max = inner.config.max_notifications;

        inner.progress.remove(id);

        // Optionally show a completion toast
        if let Some(msg) = message {
            let notification =
                Notification::new(NotificationLevel::Success, msg).with_duration(2000);
            inner.notifications.push_front(notification);

            // Trim to max
            while inner.notifications.len() > max {
                inner.notifications.pop_back();
            }
        }
    }

    /// Remove expired notifications
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn cleanup_expired(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.notifications.retain(|n| !n.is_expired());
    }

    /// Get the current configuration
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn config(&self) -> NotificationConfig {
        self.inner.read().unwrap().config.clone()
    }

    /// Get the current styles
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn styles(&self) -> NotificationStyles {
        self.inner.read().unwrap().styles.clone()
    }

    /// Get the current notifications
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn notifications(&self) -> Vec<Notification> {
        self.inner
            .read()
            .unwrap()
            .notifications
            .iter()
            .cloned()
            .collect()
    }

    /// Get the current progress notifications
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn progress_items(&self) -> Vec<ProgressNotification> {
        self.inner
            .read()
            .unwrap()
            .progress
            .values()
            .cloned()
            .collect()
    }

    /// Check if there are any visible notifications or progress items
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn has_visible(&self) -> bool {
        let inner = self.inner.read().unwrap();
        !inner.notifications.is_empty() || !inner.progress.is_empty()
    }

    /// Set the position
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn set_position(&self, position: NotificationPosition) {
        self.inner.write().unwrap().config.position = position;
    }

    /// Set the default duration
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn set_default_duration(&self, duration_ms: u64) {
        self.inner.write().unwrap().config.default_duration_ms = duration_ms;
    }

    /// Set the max notifications
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[allow(clippy::significant_drop_tightening)]
    pub fn set_max_notifications(&self, max: usize) {
        let mut inner = self.inner.write().unwrap();
        inner.config.max_notifications = max;
        // Trim if needed
        while inner.notifications.len() > max {
            inner.notifications.pop_back();
        }
    }
}

/// Wrapper type for `Arc<SharedNotificationManager>`
pub type NotificationManagerHandle = Arc<SharedNotificationManager>;
