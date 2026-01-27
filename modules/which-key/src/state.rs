//! State management for the which-key module.
//!
//! Uses `ArcSwap` for lock-free reads from the render thread while allowing
//! background updates from the saturator task.
//!
//! # Session Integration
//!
//! The module provides two layers of state:
//!
//! 1. **Shared cache** (`WhichKeyCache` in `ArcSwap`) - Lock-free, shared across
//!    the render thread and input handlers
//!
//! 2. **Per-session timer state** (`WhichKeySessionExt`) - Stored in session's
//!    `ExtensionMap`, tracks timer handles for each session

use std::{sync::Arc, time::Duration};

use {
    arc_swap::ArcSwap,
    reovim_driver_input::{KeyEvent, KeySequence},
    reovim_driver_session::SessionExtension,
    reovim_kernel::api::v1::ModeId,
    tokio::sync::mpsc,
};

/// A single binding entry to display in the popup.
#[derive(Debug, Clone)]
pub struct BindingEntry {
    /// The remaining keys after the prefix (suffix).
    pub suffix: KeySequence,

    /// Human-readable description of what the binding does.
    pub description: String,

    /// Optional category for grouping.
    pub category: Option<String>,
}

impl BindingEntry {
    /// Create a new binding entry.
    #[must_use]
    pub fn new(suffix: KeySequence, description: impl Into<String>) -> Self {
        Self {
            suffix,
            description: description.into(),
            category: None,
        }
    }

    /// Set the category for this binding.
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

/// The visibility state of the which-key popup.
#[derive(Debug, Clone, Default)]
pub enum WhichKeyVisibility {
    /// Popup is hidden, no timer running.
    #[default]
    Hidden,

    /// Timer is running, waiting to show popup.
    Waiting {
        /// The prefix key sequence.
        prefix: KeySequence,
        /// The mode when the prefix was entered.
        mode: ModeId,
    },

    /// Popup is visible.
    Showing {
        /// The prefix key sequence.
        prefix: KeySequence,
        /// The mode when the prefix was entered.
        mode: ModeId,
    },
}

impl WhichKeyVisibility {
    /// Check if the popup is currently visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        matches!(self, Self::Showing { .. })
    }

    /// Check if a timer is running.
    #[must_use]
    pub const fn is_waiting(&self) -> bool {
        matches!(self, Self::Waiting { .. })
    }

    /// Get the current prefix, if any.
    #[must_use]
    pub fn prefix(&self) -> Option<&KeySequence> {
        match self {
            Self::Hidden => None,
            Self::Waiting { prefix, .. } | Self::Showing { prefix, .. } => Some(prefix),
        }
    }

    /// Get the current mode, if any.
    #[must_use]
    pub const fn mode(&self) -> Option<&ModeId> {
        match self {
            Self::Hidden => None,
            Self::Waiting { mode, .. } | Self::Showing { mode, .. } => Some(mode),
        }
    }
}

/// The cached state for the which-key popup.
///
/// This is stored in an `ArcSwap` for lock-free reads from the render thread.
#[derive(Debug, Clone, Default)]
pub struct WhichKeyCache {
    /// Current visibility state.
    pub visibility: WhichKeyVisibility,

    /// Current filter (additional keys typed after popup shown).
    pub filter: KeySequence,

    /// Filtered bindings to display.
    pub bindings: Vec<BindingEntry>,
}

impl WhichKeyCache {
    /// Create a new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the cache to show the popup with a prefix.
    pub fn show(&mut self, prefix: KeySequence, mode: ModeId) {
        self.visibility = WhichKeyVisibility::Showing { prefix, mode };
        self.filter = KeySequence::new();
    }

    /// Update the cache to hide the popup.
    pub fn hide(&mut self) {
        self.visibility = WhichKeyVisibility::Hidden;
        self.filter = KeySequence::new();
        self.bindings.clear();
    }

    /// Add a key to the filter.
    pub fn push_filter(&mut self, key: KeyEvent) {
        self.filter.push(key);
    }

    /// Remove the last key from the filter.
    ///
    /// Returns `true` if a key was removed, `false` if the filter was empty.
    pub fn pop_filter(&mut self) -> bool {
        if self.filter.is_empty() {
            return false;
        }
        // KeySequence doesn't have pop(), so we rebuild without the last key
        let keys = self.filter.as_slice();
        if keys.len() <= 1 {
            self.filter = KeySequence::new();
        } else {
            self.filter = KeySequence::from_keys(&keys[..keys.len() - 1]);
        }
        true
    }

    /// Check if the popup is visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visibility.is_visible()
    }

    /// Get the current prefix being shown.
    #[must_use]
    pub fn prefix(&self) -> Option<&KeySequence> {
        self.visibility.prefix()
    }
}

/// A request to filter bindings.
#[derive(Debug, Clone)]
pub struct FilterRequest {
    /// The mode to query bindings for.
    pub mode: ModeId,

    /// The prefix to filter by.
    pub prefix: KeySequence,
}

impl FilterRequest {
    /// Create a new filter request.
    #[must_use]
    pub const fn new(mode: ModeId, prefix: KeySequence) -> Self {
        Self { mode, prefix }
    }
}

/// Handle to the saturator task.
///
/// Stores both the sender and the task handle for health monitoring.
pub struct SaturatorHandle {
    /// Sender to queue filter requests.
    pub tx: mpsc::Sender<FilterRequest>,

    /// Handle to the background task for health monitoring.
    pub task: tokio::task::JoinHandle<()>,
}

impl SaturatorHandle {
    /// Create a new saturator handle.
    #[must_use]
    pub const fn new(tx: mpsc::Sender<FilterRequest>, task: tokio::task::JoinHandle<()>) -> Self {
        Self { tx, task }
    }

    /// Check if the saturator task is still running.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        !self.task.is_finished()
    }

    /// Try to send a filter request to the saturator.
    ///
    /// Returns `true` if the request was sent successfully,
    /// `false` if the task has died or the channel is full.
    pub fn try_send(&self, req: FilterRequest) -> bool {
        if self.task.is_finished() {
            tracing::warn!("which-key: saturator task has died");
            return false;
        }
        self.tx.try_send(req).is_ok()
    }
}

/// Shared state for the which-key module.
pub struct WhichKeyState {
    /// Lock-free cache for render thread access.
    pub cache: Arc<ArcSwap<WhichKeyCache>>,

    /// Handle to the saturator task (set in `on_all_loaded`).
    pub saturator: Option<SaturatorHandle>,

    /// Handle to the timeout timer (if running).
    pub timer_handle: Option<tokio::task::JoinHandle<()>>,
}

impl WhichKeyState {
    /// Create a new state with an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(ArcSwap::new(Arc::new(WhichKeyCache::new()))),
            saturator: None,
            timer_handle: None,
        }
    }

    /// Get a clone of the cache Arc for sharing with other tasks.
    #[must_use]
    pub fn cache_handle(&self) -> Arc<ArcSwap<WhichKeyCache>> {
        Arc::clone(&self.cache)
    }

    /// Cancel the current timer if running.
    pub fn cancel_timer(&mut self) {
        // SAFETY: tokio::task::JoinHandle::abort() is safe to call multiple times,
        // including on already-completed tasks. It's a no-op if the task has finished.
        if let Some(handle) = self.timer_handle.take() {
            handle.abort();
        }
    }

    /// Update the cache atomically.
    pub fn update_cache<F>(&self, f: F)
    where
        F: FnOnce(&mut WhichKeyCache),
    {
        let mut new_cache = (*self.cache.load_full()).clone();
        f(&mut new_cache);
        self.cache.store(Arc::new(new_cache));
    }
}

impl Default for WhichKeyState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Service Wrapper for ServiceRegistry
// ============================================================================

/// Wrapper for registering the which-key cache in ServiceRegistry.
///
/// This allows the input handler to access the cache without
/// knowing about the which-key module internals.
pub struct WhichKeyCacheHandle(pub Arc<ArcSwap<WhichKeyCache>>);

impl WhichKeyCacheHandle {
    /// Create a new cache handle.
    #[must_use]
    pub fn new(cache: Arc<ArcSwap<WhichKeyCache>>) -> Self {
        Self(cache)
    }

    /// Get the inner cache handle.
    #[must_use]
    pub fn inner(&self) -> &Arc<ArcSwap<WhichKeyCache>> {
        &self.0
    }

    /// Get a clone of the cache handle.
    #[must_use]
    pub fn clone_inner(&self) -> Arc<ArcSwap<WhichKeyCache>> {
        Arc::clone(&self.0)
    }
}

impl reovim_kernel::api::v1::Service for WhichKeyCacheHandle {}

// ============================================================================
// Session Extension (per-session timer state)
// ============================================================================

/// Per-session state for which-key timer management.
///
/// This is stored in the session's `ExtensionMap` and provides timer
/// control for each client session. The shared cache is accessed via
/// the module's state.
///
/// # Design
///
/// - **Timer handle**: Per-session, stored here
/// - **Cache**: Shared across sessions, stored in module state
/// - **Saturator channel**: Shared, accessed via module state
///
/// # Usage
///
/// ```ignore
/// // In input handler, access via extensions
/// let wk = extensions.get_or_insert::<WhichKeySessionExt>();
/// wk.schedule_show(prefix, mode, cache, saturator_tx, timeout);
/// ```
pub struct WhichKeySessionExt {
    /// Handle to the current timer task (if running).
    timer_handle: Option<tokio::task::JoinHandle<()>>,

    /// Timeout duration for the popup (copied from config).
    timeout: Duration,

    /// Window ID of the overlay (when visible).
    overlay_window_id: Option<reovim_kernel::api::v1::WindowId>,

    /// Shared cache handle (set when timer is scheduled).
    cache: Option<Arc<ArcSwap<WhichKeyCache>>>,
}

impl Default for WhichKeySessionExt {
    fn default() -> Self {
        Self {
            timer_handle: None,
            timeout: Duration::from_millis(500), // Default timeout
            overlay_window_id: None,
            cache: None,
        }
    }
}

impl SessionExtension for WhichKeySessionExt {
    fn create() -> Self {
        Self::default()
    }
}

impl WhichKeySessionExt {
    /// Create with a custom timeout.
    #[must_use]
    pub const fn with_timeout(timeout: Duration) -> Self {
        Self {
            timer_handle: None,
            timeout,
            overlay_window_id: None,
            cache: None,
        }
    }

    /// Get the overlay window ID (if visible).
    #[must_use]
    pub const fn overlay_window_id(&self) -> Option<reovim_kernel::api::v1::WindowId> {
        self.overlay_window_id
    }

    /// Set the overlay window ID.
    pub fn set_overlay_window_id(&mut self, id: reovim_kernel::api::v1::WindowId) {
        self.overlay_window_id = Some(id);
    }

    /// Clear the overlay window ID.
    pub fn clear_overlay_window_id(&mut self) {
        self.overlay_window_id = None;
    }

    /// Set the shared cache handle.
    pub fn set_cache(&mut self, cache: Arc<ArcSwap<WhichKeyCache>>) {
        self.cache = Some(cache);
    }

    /// Get a snapshot of the current cache state.
    ///
    /// Returns `None` if no cache has been set.
    #[must_use]
    pub fn cache_snapshot(&self) -> Option<Arc<WhichKeyCache>> {
        self.cache.as_ref().map(|c| c.load_full())
    }

    /// Schedule showing the popup after the configured timeout.
    ///
    /// If a timer is already running, it is cancelled first.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The current key sequence prefix
    /// * `mode` - The current mode
    /// * `cache` - Shared cache for state updates
    /// * `saturator_tx` - Optional channel to request filtering
    pub fn schedule_show(
        &mut self,
        prefix: KeySequence,
        mode: ModeId,
        cache: Arc<ArcSwap<WhichKeyCache>>,
        saturator_tx: Option<mpsc::Sender<FilterRequest>>,
    ) {
        // Cancel any existing timer
        self.cancel_timer();

        // Update cache to waiting state
        {
            let mut new_cache = (*cache.load_full()).clone();
            new_cache.visibility = WhichKeyVisibility::Waiting {
                prefix: prefix.clone(),
                mode: mode.clone(),
            };
            cache.store(Arc::new(new_cache));
        }

        // Clone for async task
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
                if let Some(tx) = saturator_tx {
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
    /// * `cache` - Shared cache for state updates
    /// * `saturator_tx` - Optional channel to request filtering
    pub fn show_immediate(
        &mut self,
        prefix: KeySequence,
        mode: ModeId,
        cache: &Arc<ArcSwap<WhichKeyCache>>,
        saturator_tx: Option<&mpsc::Sender<FilterRequest>>,
    ) {
        // Cancel any existing timer
        self.cancel_timer();

        // Update cache to showing state
        {
            let mut new_cache = (*cache.load_full()).clone();
            new_cache.visibility = WhichKeyVisibility::Showing {
                prefix: prefix.clone(),
                mode: mode.clone(),
            };
            new_cache.filter = KeySequence::new(); // Clear filter
            cache.store(Arc::new(new_cache));
        }

        // Request filter from saturator
        if let Some(tx) = saturator_tx {
            let _ = tx.try_send(FilterRequest::new(mode, prefix));
        }

        tracing::debug!("which-key: popup shown immediately");
    }

    /// Cancel the current timer and hide if waiting.
    ///
    /// Called when:
    /// - Key sequence completes (command executed)
    /// - Escape pressed (pending cleared)
    /// - Mode changes
    pub fn cancel(&mut self, cache: &Arc<ArcSwap<WhichKeyCache>>) {
        self.cancel_timer();

        // If we were in waiting state, go back to hidden
        let current = cache.load();
        if current.visibility.is_waiting() {
            let mut new_cache = (*cache.load_full()).clone();
            new_cache.hide();
            cache.store(Arc::new(new_cache));
            tracing::trace!("which-key: timer cancelled, hidden");
        }
    }

    /// Hide the popup (if visible).
    ///
    /// Called when:
    /// - Escape pressed while popup is showing
    /// - Close command executed
    pub fn hide(&mut self, cache: &Arc<ArcSwap<WhichKeyCache>>) {
        self.cancel_timer();

        // Update cache to hidden state
        let mut new_cache = (*cache.load_full()).clone();
        new_cache.hide();
        cache.store(Arc::new(new_cache));

        tracing::debug!("which-key: popup hidden");
    }

    /// Check if a timer is currently running.
    #[must_use]
    pub fn is_timer_running(&self) -> bool {
        self.timer_handle.as_ref().is_some_and(|h| !h.is_finished())
    }

    /// Cancel the timer task.
    fn cancel_timer(&mut self) {
        if let Some(handle) = self.timer_handle.take() {
            handle.abort();
        }
    }
}

impl Drop for WhichKeySessionExt {
    fn drop(&mut self) {
        self.cancel_timer();
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_input::{KeyCode, KeyEvent},
        reovim_kernel::api::v1::ModuleId,
    };

    // Test module ID for which-key tests
    const TEST_MODULE: ModuleId = ModuleId::new("which-key-test");

    fn test_mode() -> ModeId {
        ModeId::new(TEST_MODULE, "normal")
    }

    #[test]
    fn test_cache_default_is_hidden() {
        let cache = WhichKeyCache::new();
        assert!(!cache.is_visible());
        assert!(cache.bindings.is_empty());
    }

    #[test]
    fn test_cache_show_hide() {
        let mut cache = WhichKeyCache::new();

        // Show
        cache.show(KeySequence::new(), test_mode());
        assert!(cache.is_visible());

        // Hide
        cache.hide();
        assert!(!cache.is_visible());
    }

    #[test]
    fn test_cache_filter_push_pop() {
        let mut cache = WhichKeyCache::new();
        cache.show(KeySequence::new(), test_mode());

        // Push a key
        let key = KeyEvent::new(KeyCode::Char('g'));
        cache.push_filter(key);
        assert_eq!(cache.filter.len(), 1);

        // Pop the key
        assert!(cache.pop_filter());
        assert_eq!(cache.filter.len(), 0);

        // Pop from empty returns false
        assert!(!cache.pop_filter());
    }

    #[test]
    fn test_cache_pop_empty_returns_false() {
        let mut cache = WhichKeyCache::new();
        assert!(!cache.pop_filter());
    }

    #[test]
    fn test_binding_entry_clone() {
        let entry = BindingEntry::new(KeySequence::new(), "test description");
        let cloned = entry.clone();
        assert_eq!(cloned.description, "test description");
    }

    #[test]
    fn test_binding_entry_with_category() {
        let entry = BindingEntry::new(KeySequence::new(), "test").with_category("navigation");
        assert_eq!(entry.category, Some("navigation".to_string()));
    }

    #[test]
    fn test_visibility_states() {
        let hidden = WhichKeyVisibility::Hidden;
        assert!(!hidden.is_visible());
        assert!(!hidden.is_waiting());
        assert!(hidden.prefix().is_none());

        let waiting = WhichKeyVisibility::Waiting {
            prefix: KeySequence::new(),
            mode: test_mode(),
        };
        assert!(!waiting.is_visible());
        assert!(waiting.is_waiting());
        assert!(waiting.prefix().is_some());

        let showing = WhichKeyVisibility::Showing {
            prefix: KeySequence::new(),
            mode: test_mode(),
        };
        assert!(showing.is_visible());
        assert!(!showing.is_waiting());
        assert!(showing.prefix().is_some());
    }

    #[test]
    fn test_state_new() {
        let state = WhichKeyState::new();
        assert!(state.saturator.is_none());
        assert!(state.timer_handle.is_none());
    }

    #[test]
    fn test_state_update_cache() {
        let state = WhichKeyState::new();

        state.update_cache(|cache| {
            cache.show(KeySequence::new(), test_mode());
        });

        let cache = state.cache.load();
        assert!(cache.is_visible());
    }

    #[test]
    fn test_filter_request() {
        let req = FilterRequest::new(test_mode(), KeySequence::new());
        assert_eq!(req.mode.name(), "normal");
    }

    // WhichKeySessionExt tests

    fn test_cache() -> Arc<ArcSwap<WhichKeyCache>> {
        Arc::new(ArcSwap::new(Arc::new(WhichKeyCache::new())))
    }

    fn test_prefix() -> KeySequence {
        KeySequence::parse("g").unwrap()
    }

    #[test]
    fn test_session_ext_default() {
        let ext = WhichKeySessionExt::default();
        assert!(!ext.is_timer_running());
    }

    #[test]
    fn test_session_ext_with_timeout() {
        let ext = WhichKeySessionExt::with_timeout(Duration::from_millis(1000));
        assert_eq!(ext.timeout, Duration::from_millis(1000));
    }

    #[tokio::test]
    async fn test_session_ext_schedule_show_sets_waiting() {
        let cache = test_cache();
        let mut ext = WhichKeySessionExt::default();

        ext.schedule_show(test_prefix(), test_mode(), cache.clone(), None);

        assert!(ext.is_timer_running());
        assert!(cache.load().visibility.is_waiting());
    }

    #[tokio::test]
    async fn test_session_ext_show_immediate_sets_visible() {
        let cache = test_cache();
        let mut ext = WhichKeySessionExt::default();

        ext.show_immediate(test_prefix(), test_mode(), &cache, None);

        assert!(!ext.is_timer_running()); // No timer for immediate
        assert!(cache.load().visibility.is_visible());
    }

    #[tokio::test]
    async fn test_session_ext_cancel_clears_waiting() {
        let cache = test_cache();
        let mut ext = WhichKeySessionExt::default();

        ext.schedule_show(test_prefix(), test_mode(), cache.clone(), None);
        assert!(cache.load().visibility.is_waiting());

        ext.cancel(&cache);
        assert!(!ext.is_timer_running());
        assert!(!cache.load().visibility.is_waiting());
    }

    #[tokio::test]
    async fn test_session_ext_hide_clears_visible() {
        let cache = test_cache();
        let mut ext = WhichKeySessionExt::default();

        ext.show_immediate(test_prefix(), test_mode(), &cache, None);
        assert!(cache.load().visibility.is_visible());

        ext.hide(&cache);
        assert!(!cache.load().visibility.is_visible());
    }

    #[tokio::test]
    async fn test_session_ext_timer_shows_popup_after_timeout() {
        let cache = test_cache();
        let mut ext = WhichKeySessionExt::with_timeout(Duration::from_millis(50));

        ext.schedule_show(test_prefix(), test_mode(), cache.clone(), None);

        // Wait for timeout plus buffer
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should now be visible
        assert!(cache.load().visibility.is_visible());
    }

    #[tokio::test]
    async fn test_session_ext_cancel_before_timeout_prevents_show() {
        let cache = test_cache();
        let mut ext = WhichKeySessionExt::with_timeout(Duration::from_millis(100));

        ext.schedule_show(test_prefix(), test_mode(), cache.clone(), None);

        // Cancel before timeout
        tokio::time::sleep(Duration::from_millis(30)).await;
        ext.cancel(&cache);

        // Wait past the original timeout
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should still be hidden
        assert!(!cache.load().visibility.is_visible());
    }
}
