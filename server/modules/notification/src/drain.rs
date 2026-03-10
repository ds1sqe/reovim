//! Notification drain implementation.
//!
//! Bridges [`PendingNotificationQueue`] → [`NotificationState`] so that
//! background-thread notifications appear as toast messages.
//!
//! # Architecture (#542)
//!
//! This implements the [`NotificationDrain`] trait from `reovim-driver-session`,
//! allowing other modules (e.g., completion) to trigger notification draining
//! without depending on `reovim-module-notification`.

use reovim_driver_session::{
    ChangeTracker, ExtensionApi, NotificationDrain, PendingLevel, PendingNotificationQueue,
    SessionRuntime,
};

use crate::state::{NotificationLevel, NotificationState};

/// Drains pending notifications into [`NotificationState`].
pub struct NotificationDrainImpl;

#[cfg_attr(coverage_nightly, coverage(off))]
impl NotificationDrain for NotificationDrainImpl {
    fn drain_pending(&self, runtime: &mut SessionRuntime<'_>) {
        let queue = runtime.kernel().services.get::<PendingNotificationQueue>();
        let Some(queue) = queue else { return };
        let pending = queue.drain();
        if pending.is_empty() {
            return;
        }

        let state = runtime.ext_mut::<NotificationState>();
        for notification in &pending {
            let level = match notification.level {
                PendingLevel::Info => NotificationLevel::Info,
                PendingLevel::Success => NotificationLevel::Success,
                PendingLevel::Warning => NotificationLevel::Warning,
                PendingLevel::Error => NotificationLevel::Error,
            };
            state.push(level, &notification.title);
        }
        runtime
            .take_changes()
            .record_extension_change("notification".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_impl_implements_trait() {
        let drain = NotificationDrainImpl;
        let _: &dyn NotificationDrain = &drain;
    }
}
