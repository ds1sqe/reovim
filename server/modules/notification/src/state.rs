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
mod tests {
    use super::*;

    #[test]
    fn test_session_extension_create() {
        let state = NotificationState::create();
        assert!(!state.is_active());
        assert!(state.entries().is_empty());
    }

    #[test]
    fn test_push_returns_sequential_ids() {
        let mut state = NotificationState::create();
        let id0 = state.push(NotificationLevel::Info, "first");
        let id1 = state.push(NotificationLevel::Info, "second");
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
    }

    #[test]
    fn test_push_stores_entry() {
        let mut state = NotificationState::create();
        state.push(NotificationLevel::Success, "saved");
        assert!(state.is_active());
        assert_eq!(state.entries().len(), 1);
        assert_eq!(state.entries()[0].title, "saved");
        assert_eq!(state.entries()[0].level, NotificationLevel::Success);
        assert!(state.entries()[0].body.is_empty());
        assert!(state.entries()[0].progress.is_none());
    }

    #[test]
    fn test_push_with_body() {
        let mut state = NotificationState::create();
        let id = state.push_with_body(NotificationLevel::Error, "Error", "details here");
        assert_eq!(id, 0);
        assert_eq!(state.entries()[0].body, "details here");
        assert_eq!(state.entries()[0].level, NotificationLevel::Error);
    }

    #[test]
    fn test_push_progress() {
        let mut state = NotificationState::create();
        let id = state.push_progress("Building", 35, "3/10 files");
        assert_eq!(id, 0);
        let entry = &state.entries()[0];
        assert_eq!(entry.level, NotificationLevel::Info);
        assert_eq!(entry.title, "Building");
        let progress = entry.progress.as_ref().unwrap();
        assert_eq!(progress.percent, 35);
        assert_eq!(progress.detail, "3/10 files");
    }

    #[test]
    fn test_push_progress_clamps_percent() {
        let mut state = NotificationState::create();
        state.push_progress("test", 200, "over");
        assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 100);
    }

    #[test]
    fn test_update_progress_found() {
        let mut state = NotificationState::create();
        let id = state.push_progress("Building", 10, "starting");
        assert!(state.update_progress(id, 50, "halfway"));
        let progress = state.entries()[0].progress.as_ref().unwrap();
        assert_eq!(progress.percent, 50);
        assert_eq!(progress.detail, "halfway");
    }

    #[test]
    fn test_update_progress_not_found() {
        let mut state = NotificationState::create();
        assert!(!state.update_progress(999, 50, "nope"));
    }

    #[test]
    fn test_update_progress_clamps_percent() {
        let mut state = NotificationState::create();
        let id = state.push(NotificationLevel::Info, "test");
        assert!(state.update_progress(id, 150, "over"));
        assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 100);
    }

    #[test]
    fn test_dismiss_found() {
        let mut state = NotificationState::create();
        let id = state.push(NotificationLevel::Info, "gone");
        assert!(state.dismiss(id));
        assert!(!state.is_active());
    }

    #[test]
    fn test_dismiss_not_found() {
        let mut state = NotificationState::create();
        assert!(!state.dismiss(999));
    }

    #[test]
    fn test_dismiss_preserves_others() {
        let mut state = NotificationState::create();
        let id0 = state.push(NotificationLevel::Info, "first");
        let _id1 = state.push(NotificationLevel::Info, "second");
        state.dismiss(id0);
        assert_eq!(state.entries().len(), 1);
        assert_eq!(state.entries()[0].title, "second");
    }

    #[test]
    fn test_is_active_empty() {
        let state = NotificationState::create();
        assert!(!state.is_active());
    }

    #[test]
    fn test_is_active_after_push() {
        let mut state = NotificationState::create();
        state.push(NotificationLevel::Info, "test");
        assert!(state.is_active());
    }

    #[test]
    fn test_max_entries_eviction() {
        let mut state = NotificationState::create();
        // Push 51 entries (max is 50)
        for i in 0..51 {
            state.push(NotificationLevel::Info, format!("msg-{i}"));
        }
        assert_eq!(state.entries().len(), 50);
        // Oldest (msg-0) should be evicted, first entry is msg-1
        assert_eq!(state.entries()[0].title, "msg-1");
        assert_eq!(state.entries()[49].title, "msg-50");
    }

    #[test]
    fn test_notification_level_debug() {
        let level = NotificationLevel::Warning;
        assert_eq!(format!("{level:?}"), "Warning");
    }

    #[test]
    fn test_notification_level_clone_copy() {
        let level = NotificationLevel::Error;
        let cloned = level;
        assert_eq!(level, cloned);
    }

    #[test]
    fn test_notification_level_all_variants() {
        assert_ne!(NotificationLevel::Info, NotificationLevel::Success);
        assert_ne!(NotificationLevel::Success, NotificationLevel::Warning);
        assert_ne!(NotificationLevel::Warning, NotificationLevel::Error);
    }

    #[test]
    fn test_progress_debug_clone_eq() {
        let p = Progress {
            percent: 50,
            detail: "half".to_string(),
        };
        let p2 = p.clone();
        assert_eq!(p, p2);
        assert_eq!(format!("{p:?}"), "Progress { percent: 50, detail: \"half\" }");
    }

    #[test]
    fn test_notification_debug_clone() {
        let n = Notification {
            id: 1,
            level: NotificationLevel::Info,
            title: "test".to_string(),
            body: String::new(),
            progress: None,
        };
        let n2 = n.clone();
        assert_eq!(n2.id, 1);
        assert!(format!("{n:?}").contains("test"));
    }

    #[test]
    fn test_state_debug() {
        let state = NotificationState::create();
        assert!(format!("{state:?}").contains("NotificationState"));
    }

    #[test]
    fn test_push_progress_exact_100() {
        let mut state = NotificationState::create();
        state.push_progress("test", 100, "done");
        assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 100);
    }

    #[test]
    fn test_update_progress_exact_100() {
        let mut state = NotificationState::create();
        let id = state.push_progress("test", 0, "starting");
        assert!(state.update_progress(id, 100, "done"));
        assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 100);
    }

    #[test]
    fn test_max_entries_boundary_no_eviction() {
        let mut state = NotificationState::create();
        // Push exactly 50 (max_entries) — no eviction should occur
        for i in 0..50 {
            state.push(NotificationLevel::Info, format!("msg-{i}"));
        }
        assert_eq!(state.entries().len(), 50);
        assert_eq!(state.entries()[0].title, "msg-0"); // oldest preserved
        assert_eq!(state.entries()[49].title, "msg-49");
    }
}
