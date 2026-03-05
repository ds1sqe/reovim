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
mod tests {
    use {
        reovim_driver_input::{KeyLookupState, KeySequence, KeymapQuery},
        reovim_kernel::api::v1::ModuleId,
    };

    use super::*;

    /// Test parent mode for resolver construction.
    fn test_parent_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "parent")
    }

    fn make_resolver() -> JumpResolver {
        JumpResolver::with_parent(test_parent_mode())
    }

    fn make_char_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn make_escape_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Escape)
    }

    /// Mock keymap that always returns `NotFound`.
    struct NotFoundKeymap;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for NotFoundKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::NotFound
        }
    }

    fn make_resolve_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = ids::JUMP_INPUT_MODE;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    fn resolve(
        resolver: &JumpResolver,
        key: &KeyEvent,
        client_ext: &mut ExtensionMap,
    ) -> ResolveResult {
        let keymap = NotFoundKeymap;
        let input = make_resolve_input(&keymap);
        let mut state = ModeState::new(ids::JUMP_INPUT_MODE);
        let mut shared = ExtensionMap::new();
        resolver.resolve_with_extensions(key, &mut state, &input, &mut shared, client_ext)
    }

    #[test]
    fn test_mode_id() {
        let resolver = make_resolver();
        assert_eq!(resolver.mode_id(), &ids::JUMP_INPUT_MODE);
    }

    #[test]
    fn test_inherits_from_parent() {
        let resolver = make_resolver();
        assert_eq!(resolver.inherits_from(), Some(&test_parent_mode()));
    }

    #[test]
    fn test_with_parent_stores_parent() {
        let parent = ModeId::new(ModuleId::new("vim"), "normal");
        let resolver = JumpResolver::with_parent(parent.clone());
        assert_eq!(resolver.inherits_from(), Some(&parent));
    }

    #[test]
    fn test_escape_cancels_and_pops() {
        let resolver = make_resolver();
        let mut ext = ExtensionMap::new();

        // Start a jump session.
        let jump = ext.get_or_insert::<JumpSessionState>();
        jump.start(vec!["hello world".into()], 0, 100, crate::jump::search::Direction::Both);
        assert!(jump.is_active());

        let result = resolve(&resolver, &make_escape_key(), &mut ext);
        assert!(matches!(
            result,
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::Cancelled)
            })
        ));

        // State should be cancelled.
        let jump = ext.get_or_insert::<JumpSessionState>();
        assert!(!jump.is_active());
    }

    #[test]
    fn test_escape_without_state() {
        let resolver = make_resolver();
        let mut ext = ExtensionMap::new();

        let result = resolve(&resolver, &make_escape_key(), &mut ext);
        assert!(matches!(
            result,
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::Cancelled)
            })
        ));
    }

    #[test]
    fn test_char_first_char_completed() {
        let resolver = make_resolver();
        let mut ext = ExtensionMap::new();

        let jump = ext.get_or_insert::<JumpSessionState>();
        jump.start(vec!["hello world".into()], 0, 100, crate::jump::search::Direction::Both);

        // First char: state machine waiting for second char.
        let result = resolve(&resolver, &make_char_key('h'), &mut ext);
        assert!(matches!(result, ResolveResult::Completed));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_char_auto_jump_pops_with_execute() {
        let resolver = make_resolver();
        let mut ext = ExtensionMap::new();

        let jump = ext.get_or_insert::<JumpSessionState>();
        jump.start(vec!["hello world".into()], 0, 100, crate::jump::search::Direction::Both);

        // "wo" has exactly one match -> auto-jump.
        resolve(&resolver, &make_char_key('w'), &mut ext);
        let result = resolve(&resolver, &make_char_key('o'), &mut ext);
        let ResolveResult::ModeTransition(ModeTransition::Pop {
            result: Some(PopResult::ExecuteCommand { command, args }),
        }) = result
        else {
            panic!("expected Pop with ExecuteCommand");
        };
        assert_eq!(command, ids::JUMP_EXECUTE);
        assert!(args.is_empty());
    }

    #[test]
    fn test_char_no_matches_pops_cancelled() {
        let resolver = make_resolver();
        let mut ext = ExtensionMap::new();

        let jump = ext.get_or_insert::<JumpSessionState>();
        jump.start(vec!["hello world".into()], 0, 0, crate::jump::search::Direction::Both);

        // "zz" has zero matches.
        resolve(&resolver, &make_char_key('z'), &mut ext);
        let result = resolve(&resolver, &make_char_key('z'), &mut ext);
        assert!(matches!(
            result,
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::Cancelled)
            })
        ));
    }

    #[test]
    fn test_char_multiple_matches_shows_labels() {
        let resolver = make_resolver();
        let mut ext = ExtensionMap::new();

        let jump = ext.get_or_insert::<JumpSessionState>();
        jump.start(vec!["he he he".into()], 0, 100, crate::jump::search::Direction::Both);

        // "he" has 3 matches -> showing labels, still active.
        resolve(&resolver, &make_char_key('h'), &mut ext);
        let result = resolve(&resolver, &make_char_key('e'), &mut ext);
        assert!(matches!(result, ResolveResult::Completed));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_char_label_selection_completes() {
        let resolver = make_resolver();
        let mut ext = ExtensionMap::new();

        let jump = ext.get_or_insert::<JumpSessionState>();
        jump.start(vec!["he he he".into()], 0, 100, crate::jump::search::Direction::Both);

        // "he" -> 3 matches with single-char labels.
        resolve(&resolver, &make_char_key('h'), &mut ext);
        resolve(&resolver, &make_char_key('e'), &mut ext);

        // Get first label.
        let first_label = {
            let jump = ext.get_or_insert::<JumpSessionState>();
            let matches = jump.get_matches().unwrap();
            matches[0].label.chars().next().unwrap()
        };

        // Select the label.
        let result = resolve(&resolver, &make_char_key(first_label), &mut ext);
        let ResolveResult::ModeTransition(ModeTransition::Pop {
            result: Some(PopResult::ExecuteCommand { command, .. }),
        }) = result
        else {
            panic!("expected Pop with ExecuteCommand");
        };
        assert_eq!(command, ids::JUMP_EXECUTE);
    }

    #[test]
    fn test_non_char_key_not_handled() {
        let resolver = make_resolver();
        let mut ext = ExtensionMap::new();

        let result = resolve(&resolver, &KeyEvent::new(KeyCode::Tab), &mut ext);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_backspace_not_handled() {
        let resolver = make_resolver();
        let mut ext = ExtensionMap::new();

        let result = resolve(&resolver, &KeyEvent::new(KeyCode::Backspace), &mut ext);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_text_input_sink_wired() {
        use reovim_driver_session::TextInputSink;

        // Verify JumpSessionState's TextInputSink works through insert_char.
        let mut state = JumpSessionState::default();
        state.start(vec!["hello world".into()], 0, 100, crate::jump::search::Direction::Both);
        state.insert_char('w');
        state.insert_char('o');
        assert!(state.has_target());
    }
}
