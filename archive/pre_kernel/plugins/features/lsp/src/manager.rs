//! LSP manager for buffer synchronization.
//!
//! Manages the LSP client connection and document synchronization state.

use std::{
    sync::{Arc, RwLock},
    time::Instant,
};

use {
    reovim_core::event::RuntimeEvent,
    reovim_lsp::{DiagnosticCache, LspSaturatorHandle},
    tokio::sync::mpsc,
};

use crate::{document::DocumentManager, hover::HoverCache};

/// Virtual text display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VirtualTextShowMode {
    /// Show first diagnostic only (default)
    #[default]
    First,
    /// Show highest severity diagnostic
    Highest,
    /// Show all diagnostics concatenated with separator
    All,
}

/// Configuration for virtual text display.
#[derive(Debug, Clone)]
pub struct VirtualTextConfig {
    /// Whether virtual text is enabled
    pub enabled: bool,
    /// Custom prefix for virtual text (empty = use severity icon)
    pub prefix: String,
    /// Maximum length before truncation
    pub max_length: u16,
    /// Which diagnostics to show
    pub show_mode: VirtualTextShowMode,
}

impl Default for VirtualTextConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefix: String::new(),
            max_length: 80,
            show_mode: VirtualTextShowMode::First,
        }
    }
}

/// Thread-safe wrapper for LSP state.
///
/// Registered in `PluginStateRegistry` for cross-plugin access.
pub struct SharedLspManager {
    inner: RwLock<LspManager>,
}

impl SharedLspManager {
    /// Create a new shared LSP manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(LspManager::new()),
        }
    }

    /// Access the manager immutably.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&LspManager) -> R,
    {
        let guard = self.inner.read().unwrap();
        f(&guard)
    }

    /// Access the manager mutably.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut LspManager) -> R,
    {
        let mut guard = self.inner.write().unwrap();
        f(&mut guard)
    }
}

impl Default for SharedLspManager {
    fn default() -> Self {
        Self::new()
    }
}

/// LSP manager state.
pub struct LspManager {
    /// Document state tracking.
    pub documents: DocumentManager,
    /// Handle to send requests to the saturator.
    pub handle: Option<LspSaturatorHandle>,
    /// Diagnostic cache for lock-free reads.
    pub cache: Option<Arc<DiagnosticCache>>,
    /// Hover cache for lock-free reads.
    pub hover_cache: HoverCache,
    /// Event sender for triggering re-renders.
    pub event_tx: Option<mpsc::Sender<RuntimeEvent>>,
    /// Whether the LSP server is running.
    pub running: bool,
    /// Last time a document was synced.
    pub last_sync: Option<Instant>,
    /// Virtual text configuration.
    pub virtual_text_config: VirtualTextConfig,
}

impl LspManager {
    /// Create a new LSP manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            documents: DocumentManager::new(),
            handle: None,
            cache: None,
            hover_cache: HoverCache::new(),
            event_tx: None,
            running: false,
            last_sync: None,
            virtual_text_config: VirtualTextConfig::default(),
        }
    }

    /// Update the last document sync timestamp.
    pub fn mark_sync_sent(&mut self) {
        self.last_sync = Some(Instant::now());
    }

    /// Set the event sender for triggering re-renders.
    pub fn set_event_tx(&mut self, event_tx: mpsc::Sender<RuntimeEvent>) {
        self.event_tx = Some(event_tx);
    }

    /// Send a render signal to trigger a re-render.
    pub fn send_render_signal(&self) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.try_send(RuntimeEvent::render_signal());
        }
    }

    /// Set the LSP handle and cache after startup.
    pub fn set_connection(&mut self, handle: LspSaturatorHandle, cache: Arc<DiagnosticCache>) {
        tracing::debug!("LSP: set_connection called, handle being set");
        self.handle = Some(handle);
        self.cache = Some(cache);
        self.running = true;
    }

    /// Check if the LSP server is running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Get the saturator handle.
    #[must_use]
    pub const fn handle(&self) -> Option<&LspSaturatorHandle> {
        self.handle.as_ref()
    }

    /// Get the diagnostic cache.
    #[must_use]
    pub const fn cache(&self) -> Option<&Arc<DiagnosticCache>> {
        self.cache.as_ref()
    }

    /// Shutdown the LSP server.
    pub fn shutdown(&mut self) {
        if let Some(handle) = &self.handle {
            handle.shutdown();
        }
        self.running = false;
    }
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}
