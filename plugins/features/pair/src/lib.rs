//! Pair plugin for reovim - bracket matching, highlighting, and auto-pair insertion
//!
//! This plugin provides:
//! - Rainbow bracket coloring based on nesting depth (6-color cycle)
//! - Matched pair highlighting when cursor is inside brackets
//! - Bold + underline highlighting when cursor is directly ON a bracket
//! - Unmatched bracket warning (red underline)
//! - Auto-insertion of closing brackets with cursor positioning
//!
//! # Supported Brackets
//!
//! - Round brackets: `(` `)`
//! - Square brackets: `[` `]`
//! - Curly brackets: `{` `}`
//! - Backticks: `` ` `` (for markdown code, template literals)
//! - Single quotes: `'`
//! - Double quotes: `"`
//!
//! Note: Angle brackets `<>` are intentionally excluded as they conflict with
//! operators (`->`, `=>`, `<=`, `>=`, `<<`, `>>`) in most languages.
//!
//! Symmetric pairs (backticks, quotes) use a tracking mechanism to prevent
//! infinite recursion when auto-inserting the closing character.
//!
//! # Highlighting Behavior
//!
//! | Cursor Position | Style |
//! |-----------------|-------|
//! | On bracket `(` or `)` | Rainbow color + bold + underline |
//! | Inside `(...)` | Rainbow color only |
//! | Unmatched bracket | Red + underline |
//!
//! # State Management
//!
//! The plugin stores `PairState` in `PluginStateRegistry`, tracking:
//! - Cached bracket depth information per buffer
//! - Current matched pair (if cursor is inside or on a bracket)

pub mod rainbow;
pub mod stage;
pub mod state;

use std::{
    any::TypeId,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

use reovim_core::{
    event_bus::{
        BufferClosed, BufferModification, BufferModified, CursorMoved, EventBus, EventResult,
        RequestInsertText,
    },
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

use {stage::PairRenderStage, state::SharedPairState};

/// Tracks the last auto-inserted character to prevent infinite recursion with symmetric pairs.
/// Uses a simple encoding: 0 = none, 1 = backtick, 2 = single quote, 3 = double quote
static LAST_AUTO_INSERT: AtomicU8 = AtomicU8::new(0);

fn char_to_code(c: &str) -> u8 {
    match c {
        "`" => 1,
        "'" => 2,
        "\"" => 3,
        _ => 0,
    }
}

fn is_last_auto_insert(c: &str) -> bool {
    let code = char_to_code(c);
    code != 0 && LAST_AUTO_INSERT.swap(0, Ordering::SeqCst) == code
}

/// Pair plugin for bracket matching, rainbow coloring, and auto-pair insertion
pub struct PairPlugin {
    state: Arc<SharedPairState>,
}

impl Default for PairPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PairPlugin {
    /// Create a new pair plugin
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(SharedPairState::new()),
        }
    }
}

impl Plugin for PairPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:pair")
    }

    fn name(&self) -> &'static str {
        "Pair"
    }

    fn description(&self) -> &'static str {
        "Rainbow brackets, matched pair highlighting, and auto-pair insertion"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register render stage for bracket highlighting
        let stage = Arc::new(PairRenderStage::new(Arc::clone(&self.state)));
        ctx.register_render_stage(stage);

        tracing::debug!("PairPlugin: registered render stage");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register shared pair state
        registry.register(Arc::clone(&self.state));

        tracing::debug!("PairPlugin: initialized SharedPairState");
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Handle cursor movement - update matched pair
        let state_clone = Arc::clone(&state);
        bus.subscribe::<CursorMoved, _>(100, move |event, ctx| {
            state_clone.with::<Arc<SharedPairState>, _, _>(|pair_state| {
                pair_state.update_cursor(event.buffer_id, event.to);
            });
            // Request re-render to show matched pair
            ctx.request_render();
            EventResult::Handled
        });

        // Handle buffer content changes - invalidate depth cache
        let state_clone = Arc::clone(&state);
        bus.subscribe::<BufferModified, _>(100, move |event, _ctx| {
            state_clone.with::<Arc<SharedPairState>, _, _>(|pair_state| {
                pair_state.invalidate_buffer(event.buffer_id);
            });
            tracing::trace!(buffer_id = event.buffer_id, "PairPlugin: invalidated bracket cache");
            EventResult::Handled
        });

        // Auto-pair insertion: when an opening bracket is typed, insert the closing one
        bus.subscribe::<BufferModified, _>(90, move |event, ctx| {
            if let BufferModification::Insert { text, .. } = &event.modification {
                // For symmetric pairs (`, ', "), check if this is our own auto-inserted character
                // to prevent infinite recursion
                if is_last_auto_insert(text) {
                    tracing::trace!(
                        buffer_id = event.buffer_id,
                        text = text,
                        "PairPlugin: skipping auto-insert for our own symmetric pair"
                    );
                    return EventResult::Handled;
                }

                // Only handle single-character insertions of opening brackets
                let (close, is_symmetric) = match text.as_str() {
                    "(" => (Some(")"), false),
                    "[" => (Some("]"), false),
                    "{" => (Some("}"), false),
                    "`" => (Some("`"), true),
                    "'" => (Some("'"), true),
                    "\"" => (Some("\""), true),
                    // Note: < is intentionally excluded as it's ambiguous (less-than vs angle bracket)
                    _ => (None, false),
                };

                if let Some(closing) = close {
                    tracing::trace!(
                        buffer_id = event.buffer_id,
                        open = text,
                        close = closing,
                        is_symmetric = is_symmetric,
                        "PairPlugin: auto-inserting closing bracket"
                    );

                    // For symmetric pairs, track that we're inserting to prevent recursion
                    if is_symmetric {
                        LAST_AUTO_INSERT.store(char_to_code(closing), Ordering::SeqCst);
                    }

                    // Insert the closing bracket and move cursor back
                    ctx.emit(RequestInsertText {
                        text: closing.to_string(),
                        move_cursor_left: true,
                        delete_prefix_len: 0,
                    });
                }
            }
            EventResult::Handled
        });

        // Handle buffer close - clean up state
        let state_clone = Arc::clone(&state);
        bus.subscribe::<BufferClosed, _>(100, move |event, _ctx| {
            state_clone.with::<Arc<SharedPairState>, _, _>(|pair_state| {
                pair_state.remove_buffer(event.buffer_id);
            });
            tracing::trace!(
                buffer_id = event.buffer_id,
                "PairPlugin: cleaned up pair state for closed buffer"
            );
            EventResult::Handled
        });

        tracing::debug!("PairPlugin: subscribed to cursor and buffer events");
    }
}

// Re-export types for external use
pub use state::{BracketInfo, PairState};
