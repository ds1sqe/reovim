//! Notification drain implementation.
//!
//! Bridges [`PendingNotificationQueue`] → [`NotificationState`] so that
//! background-thread notifications appear as toast messages.
//!
//! # Architecture (#542, #691)
//!
//! This implements the [`NotificationDrain`] trait from `reovim-driver-session`,
//! allowing other modules (e.g., completion) to trigger notification draining
//! without depending on `reovim-module-notification`.
//!
//! Progress lifecycle operations (`ProgressBegin`/`Report`/`End`) are handled
//! via a token-to-ID mapping maintained in [`NotificationDrainImpl`].

use {
    parking_lot::Mutex,
    reovim_driver_text_session::{
        ChangeTracker, ExtensionApi, NotificationDrain, PendingEntry, PendingLevel,
        PendingNotificationQueue, PendingOp, SessionRuntime,
    },
    std::collections::HashMap,
    tracing::debug,
};

use crate::state::{NotificationLevel, NotificationState};

/// Shared progress token-to-notification-ID mapping (#691).
///
/// Registered as a [`Service`] so both `NotificationDrainImpl` (command-handler
/// path) and `NotificationBridge::tick()` (periodic path) share the same map.
#[derive(Debug)]
pub struct NotificationTokenMap {
    map: Mutex<HashMap<String, u64>>,
}

impl NotificationTokenMap {
    /// Lock and return mutable access to the token map.
    pub fn lock(&self) -> parking_lot::MutexGuard<'_, HashMap<String, u64>> {
        self.map.lock()
    }
}

impl Default for NotificationTokenMap {
    fn default() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }
}

impl reovim_kernel::api::v1::Service for NotificationTokenMap {}

/// Drains pending notifications into [`NotificationState`].
///
/// Reads the shared [`NotificationTokenMap`] from `ServiceRegistry` for
/// progress lifecycle operations (#691). The drain impl is stateless.
pub struct NotificationDrainImpl;

impl NotificationDrainImpl {
    /// Create a new drain.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NotificationDrainImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl NotificationDrain for NotificationDrainImpl {
    fn drain_pending(&self, runtime: &mut SessionRuntime<'_>) {
        let services = &runtime.kernel().services;
        let Some(queue) = services.get::<PendingNotificationQueue>() else {
            return;
        };
        let pending = queue.drain();
        if pending.is_empty() {
            return;
        }

        let token_map_svc = services.get_or_create::<NotificationTokenMap>();
        let state = runtime.ext_mut::<NotificationState>();
        let mut token_map = token_map_svc.lock();
        let changed = drain_entries(pending, state, &mut token_map);
        drop(token_map);
        if changed {
            runtime
                .take_changes()
                .record_extension_change("notification".into());
        }
    }
}

/// Convert a [`PendingLevel`] to a [`NotificationLevel`].
pub const fn convert_level(level: PendingLevel) -> NotificationLevel {
    match level {
        PendingLevel::Info => NotificationLevel::Info,
        PendingLevel::Success => NotificationLevel::Success,
        PendingLevel::Warning => NotificationLevel::Warning,
        PendingLevel::Error => NotificationLevel::Error,
    }
}

/// Pure helper: drain pending entries into notification state.
///
/// Extracted for testability — no `SessionRuntime` dependency.
/// Returns `true` if any state was modified.
pub fn drain_entries(
    pending: Vec<PendingEntry>,
    state: &mut NotificationState,
    token_map: &mut HashMap<String, u64>,
) -> bool {
    let mut changed = false;
    for entry in pending {
        match entry.op {
            PendingOp::Push { level, title } => {
                state.push_with_source(entry.source, convert_level(level), title);
                changed = true;
            }
            PendingOp::ProgressBegin {
                token,
                title,
                message,
                percentage,
            } => {
                let id =
                    state.push_progress_with_source(entry.source, &title, percentage, &message);
                token_map.insert(token, id);
                changed = true;
            }
            PendingOp::ProgressReport {
                token,
                message,
                percentage,
            } => {
                if let Some(&id) = token_map.get(&token) {
                    let percent = percentage.unwrap_or_else(|| {
                        state
                            .entries()
                            .iter()
                            .find(|e| e.id == id)
                            .and_then(|e| e.progress.as_ref())
                            .map_or(0, |p| p.percent)
                    });
                    let detail = message.unwrap_or_default();
                    state.update_progress(id, percent, detail);
                    changed = true;
                } else {
                    debug!(token = %token, "ProgressReport for unknown token, ignoring");
                }
            }
            PendingOp::ProgressEnd { token, message: _ } => {
                if let Some(id) = token_map.remove(&token) {
                    state.dismiss(id);
                    changed = true;
                } else {
                    debug!(token = %token, "ProgressEnd for unknown token, ignoring");
                }
            }
        }
    }
    changed
}

#[cfg(test)]
#[path = "drain_tests.rs"]
mod tests;
