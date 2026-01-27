//! Which-key service for timer management and popup control.
//!
//! This service provides the interface for the input handler to control
//! the which-key popup:
//! - `schedule_show()` - Start a timer to show the popup
//! - `cancel()` - Cancel the timer (key completed or escape)
//! - `show_immediate()` - Show popup immediately (? suffix)
//! - `hide()` - Hide the popup
//!
//! # Architecture
//!
//! The service is designed to be accessed from the input handler:
//! ```text
//! Input Handler → WhichKeyService → Timer → Cache Update → Render
//! ```
//!
//! The service manages:
//! - Timer task handles for cancellation
//! - Cache updates for visibility state
//! - Coordination with the saturator task

use std::{sync::Arc, time::Duration};

use {
    arc_swap::ArcSwap, reovim_driver_input::KeySequence, reovim_kernel::api::v1::ModeId,
    tokio::sync::mpsc,
};

use crate::state::{FilterRequest, WhichKeyCache, WhichKeyVisibility};

/// Service for controlling the which-key popup.
///
/// This service provides the API for external components (input handler)
/// to control the which-key popup timing and visibility.
pub struct WhichKeyService {
    /// Shared cache for state updates.
    cache: Arc<ArcSwap<WhichKeyCache>>,
    /// Handle to the saturator task.
    saturator: Option<mpsc::Sender<FilterRequest>>,
    /// Current timer handle (if any).
    timer_handle: Option<tokio::task::JoinHandle<()>>,
    /// Timeout duration.
    timeout: Duration,
}

impl WhichKeyService {
    /// Create a new which-key service.
    ///
    /// # Arguments
    ///
    /// * `cache` - Shared cache for state updates
    /// * `timeout_ms` - Timeout in milliseconds before showing popup
    #[must_use]
    pub fn new(cache: Arc<ArcSwap<WhichKeyCache>>, timeout_ms: u64) -> Self {
        Self {
            cache,
            saturator: None,
            timer_handle: None,
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    /// Set the saturator sender for filter requests.
    pub fn set_saturator(&mut self, tx: mpsc::Sender<FilterRequest>) {
        self.saturator = Some(tx);
    }

    /// Schedule showing the popup after the configured timeout.
    ///
    /// If a timer is already running, it is cancelled first. This allows
    /// the timer to reset when the prefix changes.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The current key sequence prefix
    /// * `mode` - The current mode
    pub fn schedule_show(&mut self, prefix: KeySequence, mode: ModeId) {
        // Cancel any existing timer
        self.cancel_timer();

        // Update cache to waiting state
        {
            let mut new_cache = (*self.cache.load_full()).clone();
            new_cache.visibility = WhichKeyVisibility::Waiting {
                prefix: prefix.clone(),
                mode: mode.clone(),
            };
            self.cache.store(Arc::new(new_cache));
        }

        // Clone what we need for the async task
        let cache = Arc::clone(&self.cache);
        let saturator = self.saturator.clone();
        let timeout = self.timeout;

        // Spawn timer task
        self.timer_handle = Some(tokio::spawn(async move {
            // Wait for timeout
            tokio::time::sleep(timeout).await;

            // Check if we're still in waiting state (not cancelled)
            let current = cache.load();
            let should_show = matches!(
                &current.visibility,
                WhichKeyVisibility::Waiting { prefix: p, mode: m }
                if *p == prefix && *m == mode
            );

            if should_show {
                // Transition to showing state
                let mut new_cache = (*cache.load_full()).clone();
                new_cache.visibility = WhichKeyVisibility::Showing {
                    prefix: prefix.clone(),
                    mode: mode.clone(),
                };
                cache.store(Arc::new(new_cache));

                // Request filter from saturator
                if let Some(tx) = saturator {
                    let _ = tx.try_send(FilterRequest::new(mode, prefix));
                }

                tracing::debug!("which-key: popup shown after timeout");
            }
        }));

        tracing::trace!("which-key: timer scheduled for {}ms", self.timeout.as_millis());
    }

    /// Show the popup immediately (for ? suffix).
    ///
    /// # Arguments
    ///
    /// * `prefix` - The key sequence prefix to show bindings for
    /// * `mode` - The current mode
    pub fn show_immediate(&mut self, prefix: KeySequence, mode: ModeId) {
        // Cancel any existing timer
        self.cancel_timer();

        // Update cache to showing state
        {
            let mut new_cache = (*self.cache.load_full()).clone();
            new_cache.visibility = WhichKeyVisibility::Showing {
                prefix: prefix.clone(),
                mode: mode.clone(),
            };
            new_cache.filter = KeySequence::new(); // Clear filter
            self.cache.store(Arc::new(new_cache));
        }

        // Request filter from saturator
        if let Some(tx) = &self.saturator {
            let _ = tx.try_send(FilterRequest::new(mode, prefix));
        }

        tracing::debug!("which-key: popup shown immediately");
    }

    /// Cancel the current timer (if any).
    ///
    /// Called when:
    /// - Key sequence completes (command executed)
    /// - Escape pressed (pending cleared)
    /// - Mode changes
    /// - New prefix started (timer restarts)
    pub fn cancel(&mut self) {
        self.cancel_timer();

        // If we were in waiting state, go back to hidden
        let current = self.cache.load();
        if current.visibility.is_waiting() {
            let mut new_cache = (*self.cache.load_full()).clone();
            new_cache.hide();
            self.cache.store(Arc::new(new_cache));
            tracing::trace!("which-key: timer cancelled, hidden");
        }
    }

    /// Hide the popup (if visible).
    ///
    /// Called when:
    /// - Escape pressed while popup is showing
    /// - Close command executed
    pub fn hide(&mut self) {
        self.cancel_timer();

        // Update cache to hidden state
        let mut new_cache = (*self.cache.load_full()).clone();
        new_cache.hide();
        self.cache.store(Arc::new(new_cache));

        tracing::debug!("which-key: popup hidden");
    }

    /// Check if the popup is currently visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.cache.load().visibility.is_visible()
    }

    /// Check if a timer is currently running.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.cache.load().visibility.is_waiting()
    }

    /// Get the current prefix (if any).
    #[must_use]
    pub fn current_prefix(&self) -> Option<KeySequence> {
        self.cache.load().visibility.prefix().cloned()
    }

    /// Cancel the timer task.
    fn cancel_timer(&mut self) {
        if let Some(handle) = self.timer_handle.take() {
            handle.abort();
        }
    }
}

impl Drop for WhichKeyService {
    fn drop(&mut self) {
        // Ensure timer is cancelled on drop
        self.cancel_timer();
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_cache() -> Arc<ArcSwap<WhichKeyCache>> {
        Arc::new(ArcSwap::new(Arc::new(WhichKeyCache::new())))
    }

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_prefix() -> KeySequence {
        KeySequence::parse("g").unwrap()
    }

    #[test]
    fn test_service_new() {
        let cache = test_cache();
        let service = WhichKeyService::new(cache, 500);

        assert!(!service.is_visible());
        assert!(!service.is_waiting());
    }

    #[tokio::test]
    async fn test_schedule_show_sets_waiting() {
        let cache = test_cache();
        let mut service = WhichKeyService::new(cache.clone(), 500);

        service.schedule_show(test_prefix(), test_mode());

        assert!(service.is_waiting());
        assert!(!service.is_visible());
        assert!(service.current_prefix().is_some());
    }

    #[tokio::test]
    async fn test_show_immediate_sets_visible() {
        let cache = test_cache();
        let mut service = WhichKeyService::new(cache.clone(), 500);

        service.show_immediate(test_prefix(), test_mode());

        assert!(service.is_visible());
        assert!(!service.is_waiting());
    }

    #[tokio::test]
    async fn test_cancel_clears_waiting() {
        let cache = test_cache();
        let mut service = WhichKeyService::new(cache.clone(), 500);

        service.schedule_show(test_prefix(), test_mode());
        assert!(service.is_waiting());

        service.cancel();
        assert!(!service.is_waiting());
        assert!(!service.is_visible());
    }

    #[tokio::test]
    async fn test_hide_clears_visible() {
        let cache = test_cache();
        let mut service = WhichKeyService::new(cache.clone(), 500);

        service.show_immediate(test_prefix(), test_mode());
        assert!(service.is_visible());

        service.hide();
        assert!(!service.is_visible());
    }

    #[tokio::test]
    async fn test_schedule_show_cancels_previous() {
        let cache = test_cache();
        let mut service = WhichKeyService::new(cache.clone(), 500);

        // Schedule first timer
        let prefix1 = KeySequence::parse("g").unwrap();
        service.schedule_show(prefix1, test_mode());

        // Schedule second timer (should cancel first)
        let prefix2 = KeySequence::parse("z").unwrap();
        service.schedule_show(prefix2.clone(), test_mode());

        // Current prefix should be the second one
        assert_eq!(service.current_prefix(), Some(prefix2));
    }

    #[tokio::test]
    async fn test_timer_shows_popup_after_timeout() {
        let cache = test_cache();
        let mut service = WhichKeyService::new(cache.clone(), 50); // Short timeout for test

        service.schedule_show(test_prefix(), test_mode());

        // Wait for timeout plus buffer
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should now be visible
        assert!(service.is_visible());
    }

    #[tokio::test]
    async fn test_cancel_before_timeout_prevents_show() {
        let cache = test_cache();
        let mut service = WhichKeyService::new(cache.clone(), 100);

        service.schedule_show(test_prefix(), test_mode());

        // Cancel before timeout
        tokio::time::sleep(Duration::from_millis(30)).await;
        service.cancel();

        // Wait past the original timeout
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should still be hidden
        assert!(!service.is_visible());
    }
}
