//! Jump-input mode key resolver (#524).
//!
//! `JumpResolver` handles the `range-finder:jump-input` mode. Characters
//! are routed directly to `JumpSessionState` via `client_extensions`;
//! Escape cancels and pops back to normal mode.

use std::collections::HashMap;

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, ModeKeyResolver, ModeState, ModeTransition, PopResult, ResolveInput,
        ResolveResult,
    },
    reovim_driver_session::{ExtensionMap, TextInputSink},
    reovim_kernel::api::v1::ModeId,
};

use super::{ids, state::JumpSessionState};

/// Key resolver for jump-input mode.
///
/// Uses `resolve_with_extensions` (not `resolve_with_keymap`) because jump
/// label input is raw character processing routed directly to
/// `JumpSessionState` via `client_extensions`, not keymap-driven command
/// dispatch.
pub struct JumpResolver {
    /// Owned mode ID (satisfies lifetime requirement).
    mode_id: ModeId,
    /// Parent mode for unhandled key inheritance.
    parent_mode_id: ModeId,
}

impl JumpResolver {
    /// Create a jump resolver with the given parent mode for inheritance.
    ///
    /// The parent mode is resolved at init time via `ModeInfoStore::find_by_name()`
    /// rather than hardcoding foreign module constants.
    #[must_use]
    pub const fn with_parent(parent_mode: ModeId) -> Self {
        Self {
            mode_id: ids::JUMP_INPUT_MODE,
            parent_mode_id: parent_mode,
        }
    }
}

impl ModeKeyResolver for JumpResolver {
    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        Some(&self.parent_mode_id)
    }

    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        _input: &ResolveInput<'_>,
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        match key.code {
            KeyCode::Escape => {
                if let Some(jump) = client_extensions.get_mut::<JumpSessionState>() {
                    jump.cancel();
                }
                ResolveResult::ModeTransition(ModeTransition::Pop {
                    result: Some(PopResult::Cancelled),
                })
            }
            KeyCode::Char(c) => {
                let jump = client_extensions.get_or_insert::<JumpSessionState>();
                jump.insert_char(c);
                if jump.is_active() {
                    // State machine still running, consume key.
                    ResolveResult::Completed
                } else if jump.has_target() {
                    // Target resolved - pop mode and execute jump.
                    ResolveResult::ModeTransition(ModeTransition::Pop {
                        result: Some(PopResult::ExecuteCommand {
                            command: ids::JUMP_EXECUTE,
                            args: HashMap::new(),
                        }),
                    })
                } else {
                    // No matches - cancel and pop.
                    ResolveResult::ModeTransition(ModeTransition::Pop {
                        result: Some(PopResult::Cancelled),
                    })
                }
            }
            _ => ResolveResult::NotHandled,
        }
    }
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
