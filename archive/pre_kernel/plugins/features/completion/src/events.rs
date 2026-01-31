//! Completion plugin events
//!
//! Events for inter-plugin communication, following the treesitter pattern.

use std::sync::Arc;

use reovim_core::event_bus::Event;

use crate::registry::SourceSupport;

/// Event for registering a completion source
///
/// External plugins (e.g., LSP, snippets) emit this event to register
/// their completion sources with the completion plugin.
///
/// # Example
///
/// ```ignore
/// // In LSP plugin's subscribe()
/// bus.emit(RegisterSource {
///     source: Arc::new(LspCompletionSource::new(client)),
/// });
/// ```
#[derive(Clone)]
pub struct RegisterSource {
    /// The completion source to register
    pub source: Arc<dyn SourceSupport>,
}

impl std::fmt::Debug for RegisterSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterSource")
            .field("source_id", &self.source.source_id())
            .finish()
    }
}

impl Event for RegisterSource {
    fn priority(&self) -> u32 {
        50 // High priority for source registration
    }
}

/// Event emitted when completion results are ready
///
/// The completion plugin emits this after the saturator has computed
/// new completion items and updated the cache.
#[derive(Debug, Clone)]
pub struct CompletionReady {
    /// Buffer ID the completions are for
    pub buffer_id: usize,
    /// Number of completion items available
    pub item_count: usize,
}

impl Event for CompletionReady {
    fn priority(&self) -> u32 {
        100 // Standard plugin priority
    }
}

/// Event to request completion dismissal
///
/// Emitted when completion should be hidden (e.g., mode change, escape key).
#[derive(Debug, Clone, Copy)]
pub struct CompletionDismissed;

impl Event for CompletionDismissed {
    fn priority(&self) -> u32 {
        100
    }
}
