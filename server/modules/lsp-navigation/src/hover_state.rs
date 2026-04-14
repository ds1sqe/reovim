//! Per-client hover popup state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! The hover popup displays LSP hover information near the hovered symbol.
//!
//! # Async hover pipeline (#662)
//!
//! `HoverSnapshot` holds formatted hover data produced by an async task.
//! `HoverCache` wraps it in `ArcSwap` for lock-free transfer from the
//! async task to `HoverBridge::tick()`.

use std::sync::Arc;

use {
    reovim_driver_text_session::SessionExtension,
    reovim_kernel::api::v1::{ArcSwap, Service},
};

/// Content format for hover display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverContentType {
    /// Plain text content.
    PlainText,
    /// Markdown-formatted content.
    Markdown,
}

/// Per-client hover popup state.
///
/// Tracks whether the hover popup is visible, the content to display,
/// the content type, and the origin position (buffer position where
/// the hover was triggered).
#[derive(Debug)]
pub struct HoverState {
    /// Whether the hover popup is visible.
    pub active: bool,
    /// Hover content text.
    pub content: String,
    /// Content format (plaintext or markdown).
    pub content_type: HoverContentType,
    /// Buffer ID where hover was triggered.
    pub origin_buffer_id: u64,
    /// Line where hover was triggered (0-indexed).
    pub origin_line: u32,
    /// Column where hover was triggered (0-indexed).
    pub origin_col: u32,
}

impl HoverState {
    /// Show hover content at the given position.
    pub fn show(
        &mut self,
        content: String,
        content_type: HoverContentType,
        buffer_id: u64,
        line: u32,
        col: u32,
    ) {
        self.active = true;
        self.content = content;
        self.content_type = content_type;
        self.origin_buffer_id = buffer_id;
        self.origin_line = line;
        self.origin_col = col;
    }

    /// Dismiss the hover popup.
    pub fn dismiss(&mut self) {
        self.active = false;
        self.content.clear();
    }
}

impl SessionExtension for HoverState {
    fn create() -> Self {
        Self {
            active: false,
            content: String::new(),
            content_type: HoverContentType::PlainText,
            origin_buffer_id: 0,
            origin_line: 0,
            origin_col: 0,
        }
    }
}

/// Formatted hover data ready for display.
///
/// Produced by the async task after the LSP response arrives.
/// Stored in [`HoverCache`] for `HoverBridge::tick()` to consume.
#[derive(Debug, Clone)]
pub struct HoverSnapshot {
    /// Formatted hover text.
    pub content: String,
    /// Content format.
    pub content_type: HoverContentType,
    /// Buffer ID where hover was triggered.
    pub buffer_id: u64,
    /// Line where hover was triggered (0-indexed).
    pub line: u32,
    /// Column where hover was triggered (0-indexed).
    pub col: u32,
}

/// Lock-free cache for async hover results (#662).
///
/// The async task stores a `HoverSnapshot` here; `HoverBridge::tick()`
/// takes it out and moves the data into `HoverState`.
///
/// Registered as a `Service` in `ServiceRegistry` during module init.
#[derive(Debug)]
pub struct HoverCache {
    inner: Arc<ArcSwap<Option<HoverSnapshot>>>,
}

impl Default for HoverCache {
    fn default() -> Self {
        Self {
            inner: Arc::new(ArcSwap::from_pointee(None)),
        }
    }
}

impl Service for HoverCache {}

impl HoverCache {
    /// Get a clone of the inner `Arc` for sharing with async tasks.
    #[must_use]
    pub fn shared(&self) -> Arc<ArcSwap<Option<HoverSnapshot>>> {
        Arc::clone(&self.inner)
    }

    /// Take the pending snapshot (if any), replacing it with `None`.
    #[must_use]
    pub fn take(&self) -> Option<HoverSnapshot> {
        let current = self.inner.swap(Arc::new(None));
        Arc::try_unwrap(current).unwrap_or_else(|arc| (*arc).clone())
    }
}

#[cfg(test)]
#[path = "hover_state_tests.rs"]
mod tests;
