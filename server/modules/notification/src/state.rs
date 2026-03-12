//! Notification state - POLICY layer.
//!
//! [`NotificationState`] stores the list of active notifications.
//! Other modules push notifications via
//! `runtime.ext_mut::<NotificationState>().push(level, title)`.
//!
//! # Architecture (#443)
//!
//! This is a [`SessionExtension`] stored in the client's `ExtensionMap`.
//! The bridge reads it and serializes to JSON for gRPC transmission.
//! The TUI client manages display lifecycle (timeouts, stacking, dismissal).

use reovim_driver_session::SessionExtension;

/// Notification severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Informational message (blue/cyan).
    Info,
    /// Success confirmation (green).
    Success,
    /// Warning message (yellow).
    Warning,
    /// Error message (red).
    Error,
}

/// Progress indicator state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// Percentage complete (0..=100).
    pub percent: u8,
    /// Optional detail message (e.g., "3/10 files").
    pub detail: String,
}

/// A single notification entry.
#[derive(Debug, Clone)]
pub struct Notification {
    /// Unique identifier for deduplication by the client.
    pub id: u64,
    /// Severity level.
    pub level: NotificationLevel,
    /// Short title (required).
    pub title: String,
    /// Optional longer message body.
    pub body: String,
    /// Optional progress indicator.
    pub progress: Option<Progress>,
}

/// Session extension for notification state.
///
/// Accumulates notifications pushed by other modules.
/// The bridge serializes ALL entries; the TUI client deduplicates by ID
/// and manages display lifecycle (timeouts, stacking, dismissal).
///
/// # Usage
///
/// ```ignore
/// // In a command handler with SessionRuntime:
/// let state = runtime.ext_mut::<NotificationState>();
/// state.push(NotificationLevel::Success, "File saved");
/// changes.record_extension_change("notification".into());
/// ```
#[derive(Debug)]
pub struct NotificationState {
    /// Monotonically increasing counter for unique notification IDs.
    next_id: u64,
    /// Active notifications (newest last).
    entries: Vec<Notification>,
    /// Maximum entries to retain (prevents unbounded growth).
    max_entries: usize,
}

impl SessionExtension for NotificationState {
    fn create() -> Self {
        Self {
            next_id: 0,
            entries: Vec::new(),
            max_entries: 50,
        }
    }
}

impl NotificationState {
    /// Push a new notification. Returns the assigned ID.
    pub fn push(&mut self, level: NotificationLevel, title: impl Into<String>) -> u64 {
        self.push_full(level, title.into(), String::new(), None)
    }

    /// Push a notification with body text. Returns the assigned ID.
    pub fn push_with_body(
        &mut self,
        level: NotificationLevel,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> u64 {
        self.push_full(level, title.into(), body.into(), None)
    }

    /// Push a progress notification. Returns the assigned ID.
    pub fn push_progress(
        &mut self,
        title: impl Into<String>,
        percent: u8,
        detail: impl Into<String>,
    ) -> u64 {
        let progress = Progress {
            percent: percent.min(100),
            detail: detail.into(),
        };
        self.push_full(NotificationLevel::Info, title.into(), String::new(), Some(progress))
    }

    /// Update the progress on an existing notification.
    /// Returns true if the notification was found and updated.
    pub fn update_progress(&mut self, id: u64, percent: u8, detail: impl Into<String>) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.progress = Some(Progress {
                percent: percent.min(100),
                detail: detail.into(),
            });
            true
        } else {
            false
        }
    }

    /// Remove a notification by ID (e.g., when progress completes).
    /// Returns true if found and removed.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let len_before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len_before
    }

    /// Get all current entries (for bridge serialization).
    #[must_use]
    pub fn entries(&self) -> &[Notification] {
        &self.entries
    }

    /// Check whether there are any active entries.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Internal: push a fully-specified notification.
    fn push_full(
        &mut self,
        level: NotificationLevel,
        title: String,
        body: String,
        progress: Option<Progress>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        self.entries.push(Notification {
            id,
            level,
            title,
            body,
            progress,
        });

        // Evict oldest if over capacity
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }

        id
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
