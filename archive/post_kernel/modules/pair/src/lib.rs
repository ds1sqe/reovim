//! Pair module: auto-close brackets, rainbow highlighting, matched pair indicator.
//!
//! This module provides bracket assistance features:
//!
//! - **Auto-close brackets**: Typing `(` inserts `()` with cursor between
//! - **Rainbow bracket highlighting**: Nested brackets colored by depth
//! - **Matched pair indicator**: Highlight matching bracket under cursor
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │  PAIR MODULE (this crate)                                   POLICY      │
//! │  PairModule, SharedPairState, rainbow colors, auto-pair behavior        │
//! │  → Decides HOW brackets behave                                          │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  DISPLAY DRIVER                                             MECHANISM   │
//! │  DecorationProvider, DecorationStore, Style, Color                      │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  KERNEL                                                     MECHANISM   │
//! │  Module trait, ServiceRegistry, EventBus, BufferManager                 │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Event Subscriptions
//!
//! The module subscribes to the following kernel events:
//!
//! - `CursorMoved` (priority 100): Update cursor position for matched pair tracking
//! - `BufferModified` (priority 90): Auto-pair insertion for opening brackets
//! - `BufferModified` (priority 100): Invalidate bracket cache when content changes
//! - `BufferClosed` (priority 100): Cleanup buffer-specific state
//!
//! # Usage
//!
//! The module is automatically loaded when included in the defaults bundle.
//! No configuration is required for basic functionality.
//!
//! ```ignore
//! use reovim_module_pair::{PairModule, PAIR_MODULE};
//!
//! let module = PairModule::new();
//! assert_eq!(module.id(), PAIR_MODULE);
//! ```

pub mod auto_pair;
pub mod decorations;
pub mod rainbow;
pub mod state;
pub mod styles;

// Re-exports
pub use {
    auto_pair::AutoPairResult,
    decorations::{
        PAIR_DECORATION_GROUP, RAINBOW_COLORS, UNMATCHED_COLOR, color_for_depth,
        generate_bracket_decorations, generate_decorations_for_buffer, style_for_bracket,
        style_for_matched_pair,
    },
    state::{BracketInfo, MatchedPair, SharedPairState},
};

/// Decoration source key for rainbow bracket highlighting.
///
/// This follows the `<module>.<feature>` naming convention.
/// Used to register in `BufferDecorationSourceRegistry`.
pub const DECORATION_KEY_RAINBOW: &str = "pair.rainbow";

use std::sync::Arc;

use {
    reovim_arch::sync::RwLock,
    reovim_driver_buffer::{BufferManagerKey, BufferManagerRegistry},
    reovim_kernel::api::v1::{
        Buffer, BufferId, EventResult, Module, ModuleContext, ModuleError, ModuleId, Position,
        ProbeResult, Subscription, Version,
        events::kernel::{BufferClosed, BufferModified, CursorMoved, Modification, priority},
    },
};

use {
    crate::auto_pair::{
        get_closing_bracket, is_closing_bracket, is_opening_bracket, mark_auto_insert,
        should_auto_pair,
    },
    tracing::{debug, info, trace},
};

/// Convert event `buffer_id` (u64) to kernel `BufferId`.
///
/// # Note
///
/// Buffer IDs are monotonically increasing from 0 within a session,
/// so they will never realistically exceed `usize::MAX` even on 32-bit systems.
#[allow(clippy::cast_possible_truncation)]
const fn buffer_id_from_event(event_buffer_id: u64) -> BufferId {
    BufferId::from_raw(event_buffer_id as usize)
}

/// Handle auto-pair logic for a single character insertion.
///
/// This function handles:
/// - **Skip-over**: If a closing bracket is typed and the next char is the same,
///   delete the just-typed char and leave cursor at the original bracket.
/// - **Auto-pair**: If an opening bracket is typed, insert the closing bracket
///   and position cursor between them.
fn handle_auto_pair(
    typed_char: char,
    start: (u32, u32),
    text: &str,
    buffer_lock: &Arc<RwLock<Buffer>>,
) {
    // Check for closing bracket skip-over (#440)
    // If user typed a closing bracket and the next char is the same,
    // delete the just-typed char and move cursor forward
    if is_closing_bracket(typed_char) {
        let mut buffer = buffer_lock.write();
        let insert_pos = Position::new(start.0 as usize, start.1 as usize);

        // Check char at position AFTER the insert (insert_pos.column + 1)
        if let Some(line_content) = buffer.line(insert_pos.line)
            && let Some(next_char) = line_content.chars().nth(insert_pos.column + 1)
            && next_char == typed_char
        {
            // Skip over: delete the just-inserted char, move cursor AFTER the bracket
            buffer.delete_range(insert_pos, Position::new(insert_pos.line, insert_pos.column + 1));
            // Cursor should be AFTER the closing bracket (position + 1)
            let after_bracket = Position::new(insert_pos.line, insert_pos.column + 1);
            buffer.set_position(after_bracket);
            trace!(
                "Pair: skipped over '{}' at ({}, {}), cursor now at col {}",
                typed_char, insert_pos.line, insert_pos.column, after_bracket.column
            );
            return;
        }
        drop(buffer);
    }

    // Check for opening bracket auto-pair
    match should_auto_pair(text) {
        AutoPairResult::Insert { close, symmetric } => {
            // Calculate position after the inserted character
            let insert_pos = Position::new(start.0 as usize, start.1 as usize + 1);

            // Insert closing bracket, then restore cursor to between brackets
            {
                let mut buffer = buffer_lock.write();
                buffer.insert_at(insert_pos, &close.to_string());
                buffer.set_position(insert_pos); // Restore cursor between brackets
            }

            // Track symmetric pairs to prevent infinite recursion
            if symmetric {
                mark_auto_insert(close);
            }

            trace!(
                "Pair: auto-inserted '{}' after '{}' at ({}, {})",
                close, text, insert_pos.line, insert_pos.column
            );
        }
        AutoPairResult::Skip | AutoPairResult::SkipOver => {}
    }
}

/// Handle backspace pair deletion (#440).
///
/// When an opening bracket is deleted (via backspace), check if the next character
/// is the corresponding closing bracket. If so, delete it too.
///
/// Example: `(|)` + backspace → empty buffer (both brackets deleted)
fn handle_backspace_pair(start: (u32, u32), deleted_text: &str, buffer_lock: &Arc<RwLock<Buffer>>) {
    // Only handle single-character deletions
    let mut chars = deleted_text.chars();
    let Some(deleted_char) = chars.next() else {
        return;
    };
    if chars.next().is_some() {
        return; // Multi-char delete, skip
    }

    // Only handle opening brackets
    if !is_opening_bracket(deleted_char) {
        return;
    }

    // Get the expected closing bracket
    let Some(expected_close) = get_closing_bracket(deleted_char) else {
        return;
    };

    let mut buffer = buffer_lock.write();
    let delete_pos = Position::new(start.0 as usize, start.1 as usize);

    // Check if the character at the delete position is the closing bracket
    if let Some(line_content) = buffer.line(delete_pos.line)
        && let Some(char_at_pos) = line_content.chars().nth(delete_pos.column)
        && char_at_pos == expected_close
    {
        // Delete the closing bracket
        buffer.delete_range(delete_pos, Position::new(delete_pos.line, delete_pos.column + 1));
        drop(buffer);
        trace!(
            "Pair: deleted matching '{}' after backspace of '{}' at ({}, {})",
            expected_close, deleted_char, delete_pos.line, delete_pos.column
        );
    }
}

/// The module identifier for the pair module.
pub const PAIR_MODULE: ModuleId = ModuleId::new("pair");

/// Pair module instance.
///
/// Provides bracket assistance: auto-close, rainbow highlighting, and
/// matched pair indication.
///
/// # Subscription Lifetime
///
/// Event subscriptions are stored in the `subscriptions` field to prevent
/// early drop. They are automatically unsubscribed when the module exits.
pub struct PairModule {
    /// Shared bracket state registered in `ServiceRegistry`.
    state: Option<Arc<SharedPairState>>,
    /// Active event subscriptions (RAII: dropped on exit).
    subscriptions: Vec<Subscription>,
}

impl PairModule {
    /// Create a new pair module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: None,
            subscriptions: Vec::new(),
        }
    }
}

impl Default for PairModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PairModule {
    fn id(&self) -> ModuleId {
        PAIR_MODULE
    }

    fn name(&self) -> &'static str {
        "Pair Module"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 2)
    }

    #[allow(clippy::too_many_lines)]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        info!("Initializing pair module");

        // Create and register shared state
        let state = Arc::new(SharedPairState::new());
        ctx.services.register(state.clone());
        self.state = Some(state.clone());

        debug!("Pair module: SharedPairState registered in ServiceRegistry");

        // Register style group defaults in StyleGroupRegistry
        // This allows themes to optionally override rainbow bracket colors
        {
            use reovim_driver_display::StyleGroupRegistry;

            let style_registry = ctx.services.get::<StyleGroupRegistry>().unwrap_or_else(|| {
                let new_registry = StyleGroupRegistry::new();
                ctx.services.register(Arc::new(new_registry));
                ctx.services.get::<StyleGroupRegistry>().unwrap()
            });

            style_registry.register_batch(&styles::default_registrations());
            debug!(
                "Pair module: registered {} style groups in StyleGroupRegistry",
                styles::groups::ALL_RAINBOW_GROUPS.len()
            );
        }

        // Register as BufferDecorationSource in BufferDecorationSourceRegistry (#440)
        // This allows the runner to query decorations generically without
        // knowing about this module specifically (mechanism/policy separation)
        {
            use reovim_driver_display::{BufferDecorationSourceRegistry, DecorationSourceKey};

            let registry = ctx
                .services
                .get::<BufferDecorationSourceRegistry>()
                .unwrap_or_else(|| {
                    let new_registry = BufferDecorationSourceRegistry::new();
                    ctx.services.register(Arc::new(new_registry));
                    ctx.services
                        .get::<BufferDecorationSourceRegistry>()
                        .unwrap()
                });

            let key = DecorationSourceKey::new(DECORATION_KEY_RAINBOW);
            registry.register(key, state.clone());
            debug!(
                "Pair module: registered in BufferDecorationSourceRegistry with key '{}'",
                DECORATION_KEY_RAINBOW
            );
        }

        // Get event bus for subscriptions
        let bus = Arc::clone(&ctx.kernel.event_bus);

        // Subscribe to cursor movement events
        // Priority: PLUGIN (100) - standard module priority
        let state_cursor = state.clone();
        let sub_cursor = bus.subscribe::<CursorMoved, _>(priority::PLUGIN, move |event| {
            let buffer_id = buffer_id_from_event(event.buffer_id);
            let cursor = (event.to.0 as usize, event.to.1 as usize);

            trace!(
                "Pair: cursor moved in buffer {} to ({}, {})",
                event.buffer_id, cursor.0, cursor.1
            );

            state_cursor.update_cursor(buffer_id, cursor);
            EventResult::Handled
        });
        self.subscriptions.push(sub_cursor);

        // Subscribe to buffer modifications for auto-pair insertion
        // Priority: 90 - process before cache invalidation at 100
        //
        // NOTE (#440): We capture `services` instead of `ctx.kernel.buffers` because
        // the buffer manager is registered AFTER this module's init() by another module.
        // At init time, ctx.kernel.buffers is a stub. We must query ServiceRegistry
        // at handler invocation time to get the real buffer manager.
        let services_for_auto_pair = Arc::clone(&ctx.services);
        let sub_auto_pair = bus.subscribe::<BufferModified, _>(90, move |event| {
            let buffer_id = buffer_id_from_event(event.buffer_id);

            // Query buffer manager from ServiceRegistry (late binding)
            let buffer_lock = services_for_auto_pair
                .get::<BufferManagerRegistry>()
                .and_then(|registry| registry.get(&BufferManagerKey::Simple))
                .and_then(|mgr| mgr.get(buffer_id));

            let Some(buffer_lock) = buffer_lock else {
                trace!("Pair: buffer {} not found, skipping", event.buffer_id);
                return EventResult::Handled;
            };

            match &event.modification {
                // Handle Insert: auto-pair or skip-over
                Modification::Insert { start, text } => {
                    // Only handle single-character insertions
                    let mut chars = text.chars();
                    let Some(typed_char) = chars.next() else {
                        return EventResult::Handled;
                    };
                    if chars.next().is_some() {
                        return EventResult::Handled; // Multi-char insert, skip
                    }

                    handle_auto_pair(typed_char, *start, text, &buffer_lock);
                }

                // Handle Delete: backspace pair deletion (#440)
                Modification::Delete { start, text, .. } => {
                    handle_backspace_pair(*start, text, &buffer_lock);
                }

                // Replace, FullReplace: no special handling needed
                Modification::Replace { .. } | Modification::FullReplace => {}
            }

            EventResult::Handled
        });
        self.subscriptions.push(sub_auto_pair);

        // Subscribe to buffer modification events
        // Priority: PLUGIN (100) - invalidate cache when content changes
        let state_modified = state.clone();
        let sub_modified = bus.subscribe::<BufferModified, _>(priority::PLUGIN, move |event| {
            let buffer_id = buffer_id_from_event(event.buffer_id);

            trace!("Pair: buffer {} modified, invalidating bracket cache", event.buffer_id);

            state_modified.invalidate_buffer(buffer_id);
            EventResult::Handled
        });
        self.subscriptions.push(sub_modified);

        // Subscribe to buffer close events
        // Priority: PLUGIN (100) - cleanup buffer state
        let state_closed = state;
        let sub_closed = bus.subscribe::<BufferClosed, _>(priority::PLUGIN, move |event| {
            let buffer_id = buffer_id_from_event(event.buffer_id);

            trace!("Pair: buffer {} closed, removing state", event.buffer_id);

            state_closed.remove_buffer(buffer_id);
            EventResult::Handled
        });
        self.subscriptions.push(sub_closed);

        info!("Pair module initialized with {} event subscriptions", self.subscriptions.len());
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        info!("Shutting down pair module");

        // Clear subscriptions (auto-unsubscribe via RAII)
        let count = self.subscriptions.len();
        self.subscriptions.clear();
        debug!("Pair module: cleared {} subscriptions", count);

        self.state = None;
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PairModule);

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use reovim_kernel::api::v1::Module;

    use super::*;

    #[test]
    fn test_pair_module_id() {
        assert_eq!(PAIR_MODULE.as_str(), "pair");
    }

    #[test]
    fn test_pair_module_identity() {
        let module = PairModule::new();
        assert_eq!(module.id(), PAIR_MODULE);
        assert_eq!(module.name(), "Pair Module");
        assert_eq!(module.version(), Version::new(0, 9, 2));
    }

    #[test]
    fn test_pair_module_default() {
        let module = PairModule::default();
        assert!(module.state.is_none());
        assert!(module.subscriptions.is_empty());
    }

    #[test]
    fn test_pair_module_subscription_count() {
        // Verify module struct can hold subscriptions
        let module = PairModule::new();
        assert_eq!(module.subscriptions.len(), 0);
    }
}
