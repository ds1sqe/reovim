//! Window mode key resolver.
//!
//! Window mode (`<C-w>` prefix) handles window navigation and manipulation.
//! Each key in window mode executes a window command and automatically
//! returns to normal mode.
//!
//! # Architecture (Epic #438)
//!
//! Window mode is a simple modal resolver:
//! - Enter via `<C-w>` from normal mode
//! - Each key executes a window command (split, focus, resize, close)
//! - Automatically return to normal mode after command execution
//!
//! # Keybindings
//!
//! - `h/j/k/l` or arrows - Focus navigation
//! - `w/W/p` - Focus cycling
//! - `s/v/n` - Split operations
//! - `c/q/o` - Close operations
//! - `+/-/>/</=` - Resize operations
//! - `<Esc>` - Cancel and return to normal mode

use std::collections::HashMap;

use {
    reovim_driver_input::{
        KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, ModeTransition,
        PopResult, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::modes::VimMode;

/// Vim window mode key resolver.
///
/// Window mode is entered via `<C-w>` from normal mode. Each key executes
/// a window command and the mode automatically returns to normal.
///
/// This resolver is simple compared to operator resolvers:
/// - No count accumulation (window commands don't use counts in window mode)
/// - No multi-key sequences beyond the initial key
/// - Automatic return to normal mode after each command
///
/// # Example
///
/// ```ignore
/// // Enter window mode (from normal mode via <C-w>)
/// // Press 'h' - executes focus-left, returns to normal
/// // Press 's' - executes split-horizontal, returns to normal
/// ```
pub struct VimWindowResolver {
    /// Mode ID for window mode.
    mode_id: ModeId,
}

impl VimWindowResolver {
    /// Create a new window mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: VimMode::WINDOW_ID,
        }
    }
}

impl Default for VimWindowResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimWindowResolver {
    /// Window mode key resolution.
    ///
    /// Each key in window mode is looked up in the keymap. If a binding is found,
    /// the command is executed and we transition back to normal mode. If not found,
    /// the key is ignored (`NotHandled`).
    ///
    /// Unlike operator modes, window mode doesn't accumulate counts or wait for
    /// motions - each key is a complete command.
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Query keymap for this key in window mode
        let keys = KeySequence::from_keys(&[*key]);
        let lookup_state = input.keymap.query(input.mode, &keys);

        match lookup_state {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd, .. } => {
                // Found a binding - execute command and return to normal mode
                // Use ModeTransition::Pop with PopResult::ExecuteCommand to both
                // execute the command and return to normal mode (parent mode).

                // Check if this is the cancel command (Escape)
                if cmd == crate::ids::CANCEL_TO_NORMAL {
                    // Just pop back to normal without executing anything extra
                    ResolveResult::ModeTransition(ModeTransition::Pop {
                        result: Some(PopResult::Cancelled),
                    })
                } else {
                    // Window command - execute and pop back to normal mode
                    ResolveResult::ModeTransition(ModeTransition::Pop {
                        result: Some(PopResult::ExecuteCommand {
                            command: cmd,
                            args: HashMap::new(),
                        }),
                    })
                }
            }
            KeyLookupState::PrefixOnly => {
                // Waiting for more keys (window mode currently has no multi-key sequences)
                ResolveResult::Pending
            }
            KeyLookupState::NotFound => {
                // No binding - key not handled
                // Vim ignores unbound keys in window mode without exiting
                ResolveResult::NotHandled
            }
        }
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        // Window mode doesn't inherit from normal mode
        // (keys have different meanings)
        None
    }

    fn reset(&mut self) {
        // No state to reset in window mode
    }
}

#[cfg(test)]
#[allow(clippy::doc_markdown)]
mod tests {
    use {
        reovim_driver_input::{KeyCode, KeyLookupState, KeySequence, KeymapQuery},
        reovim_kernel::api::v1::ModeId,
    };

    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn test_state() -> ModeState {
        ModeState::new(VimMode::WINDOW_ID)
    }

    /// Mock keymap that returns `NotFound` for all queries.
    struct NotFoundKeymap;

    impl KeymapQuery for NotFoundKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::NotFound
        }
    }

    fn resolve_input(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::WINDOW_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    #[test]
    fn test_new_resolver() {
        let resolver = VimWindowResolver::new();
        assert_eq!(resolver.mode_id(), &VimMode::WINDOW_ID);
    }

    #[test]
    fn test_mode_id() {
        let resolver = VimWindowResolver::new();
        assert_eq!(resolver.mode_id().name(), "window");
    }

    #[test]
    fn test_inherits_from_none() {
        let resolver = VimWindowResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn test_unbound_key_not_handled() {
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // Unbound key should return NotHandled
        let result = resolver.resolve_with_keymap(&key('x'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_escape_not_handled_without_binding() {
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // Escape with no keymap binding returns NotHandled
        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_default_impl() {
        let resolver = VimWindowResolver::default();
        assert_eq!(resolver.mode_id(), &VimMode::WINDOW_ID);
    }

    // ========================================================================
    // Keymap-based resolution tests
    // ========================================================================

    /// Mock keymap returning ExactOnly for a given command.
    struct ExactOnlyKeymap {
        cmd: &'static str,
    }

    impl KeymapQuery for ExactOnlyKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::ExactOnly(reovim_kernel::api::v1::CommandId::new(
                reovim_kernel::api::v1::ModuleId::new("test"),
                self.cmd,
            ))
        }
    }

    /// Mock keymap returning PrefixOnly.
    struct PrefixOnlyKeymap;

    impl KeymapQuery for PrefixOnlyKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::PrefixOnly
        }
    }

    /// Mock keymap returning ExactWithLonger.
    struct ExactWithLongerKeymap {
        cmd: &'static str,
    }

    impl KeymapQuery for ExactWithLongerKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::ExactWithLonger {
                exact: reovim_kernel::api::v1::CommandId::new(
                    reovim_kernel::api::v1::ModuleId::new("test"),
                    self.cmd,
                ),
            }
        }
    }

    fn resolve_input_with<T: KeymapQuery>(keymap: &T) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::WINDOW_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    #[test]
    fn test_exact_only_command_pops_with_execute() {
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = ExactOnlyKeymap { cmd: "focus-left" };
        let input = resolve_input_with(&keymap);

        let result = resolver.resolve_with_keymap(&key('h'), &mut state, &input);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::ExecuteCommand { command, .. }),
            }) => {
                assert_eq!(command.name(), "focus-left");
            }
            _ => panic!("Expected Pop with ExecuteCommand, got {result:?}"),
        }
    }

    #[test]
    fn test_exact_with_longer_pops_with_execute() {
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = ExactWithLongerKeymap {
            cmd: "split-horizontal",
        };
        let input = resolve_input_with(&keymap);

        let result = resolver.resolve_with_keymap(&key('s'), &mut state, &input);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::ExecuteCommand { command, .. }),
            }) => {
                assert_eq!(command.name(), "split-horizontal");
            }
            _ => panic!("Expected Pop with ExecuteCommand, got {result:?}"),
        }
    }

    #[test]
    fn test_prefix_only_returns_pending() {
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = PrefixOnlyKeymap;
        let input = resolve_input_with(&keymap);

        let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
    }

    #[test]
    fn test_escape_binding_pops_cancelled() {
        // When keymap returns the CANCEL_TO_NORMAL command for Escape,
        // the resolver should Pop with Cancelled (not ExecuteCommand).
        struct CancelKeymap;
        impl KeymapQuery for CancelKeymap {
            fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
                KeyLookupState::ExactOnly(crate::ids::CANCEL_TO_NORMAL)
            }
        }

        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = CancelKeymap;
        let input = resolve_input_with(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::Cancelled),
            }) => {}
            _ => panic!("Expected Pop with Cancelled, got {result:?}"),
        }
    }

    #[test]
    fn test_reset_is_noop() {
        // Window mode has no state to reset, verify reset is safe to call
        let mut resolver = VimWindowResolver::new();
        resolver.reset();
        // Resolver should still work
        assert_eq!(resolver.mode_id(), &VimMode::WINDOW_ID);
    }

    #[test]
    fn test_multiple_keys_independent() {
        // Each key resolution is independent in window mode
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = ExactOnlyKeymap { cmd: "focus-down" };
        let input = resolve_input_with(&keymap);

        // First key
        let result = resolver.resolve_with_keymap(&key('j'), &mut state, &input);
        assert!(matches!(result, ResolveResult::ModeTransition(ModeTransition::Pop { .. })));

        // Second key - should also work (no accumulated state)
        let result = resolver.resolve_with_keymap(&key('k'), &mut state, &input);
        assert!(matches!(result, ResolveResult::ModeTransition(ModeTransition::Pop { .. })));
    }

    #[test]
    fn test_not_found_returns_not_handled() {
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_cancel_with_exact_with_longer() {
        // Test that CANCEL_TO_NORMAL also works via ExactWithLonger
        struct CancelWithLongerKeymap;
        impl KeymapQuery for CancelWithLongerKeymap {
            fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
                KeyLookupState::ExactWithLonger {
                    exact: crate::ids::CANCEL_TO_NORMAL,
                }
            }
        }

        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = CancelWithLongerKeymap;
        let input = resolve_input_with(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::Cancelled),
            }) => {}
            _ => panic!("Expected Pop with Cancelled, got {result:?}"),
        }
    }

    #[test]
    fn test_execute_command_args_empty() {
        // Verify that the ExecuteCommand pop result has empty args
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = ExactOnlyKeymap {
            cmd: "close-window",
        };
        let input = resolve_input_with(&keymap);

        let result = resolver.resolve_with_keymap(&key('c'), &mut state, &input);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::ExecuteCommand { args, .. }),
            }) => {
                assert!(args.is_empty());
            }
            _ => panic!("Expected Pop with ExecuteCommand, got {result:?}"),
        }
    }

    #[test]
    fn test_exact_with_longer_command_name() {
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = ExactWithLongerKeymap {
            cmd: "split-vertical",
        };
        let input = resolve_input_with(&keymap);

        let result = resolver.resolve_with_keymap(&key('v'), &mut state, &input);
        match result {
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::ExecuteCommand { command, .. }),
            }) => {
                assert_eq!(command.name(), "split-vertical");
            }
            _ => panic!("Expected Pop with ExecuteCommand, got {result:?}"),
        }
    }

    #[test]
    fn test_different_key_chars_same_result() {
        // Window mode treats each key independently through keymap
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = ExactOnlyKeymap { cmd: "focus-right" };
        let input = resolve_input_with(&keymap);

        let result = resolver.resolve_with_keymap(&key('l'), &mut state, &input);
        assert!(matches!(result, ResolveResult::ModeTransition(ModeTransition::Pop { .. })));
    }

    #[test]
    fn test_mode_id_module() {
        let resolver = VimWindowResolver::new();
        assert_eq!(resolver.mode_id().module().as_str(), "vim");
    }

    #[test]
    fn test_not_found_different_keys() {
        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // All keys return NotHandled with NotFound keymap
        for c in ['a', 'b', 'c', '1', '2', '3'] {
            let result = resolver.resolve_with_keymap(&key(c), &mut state, &input);
            assert!(matches!(result, ResolveResult::NotHandled), "key '{c}' should be not handled");
        }
    }

    #[test]
    fn test_const_new() {
        // Verify const fn construction works
        const RESOLVER: VimWindowResolver = VimWindowResolver::new();
        assert_eq!(RESOLVER.mode_id().name(), "window");
    }

    #[test]
    fn test_reset_then_resolve() {
        // After reset, resolver should still work normally
        let mut resolver = VimWindowResolver::new();
        resolver.reset();

        let mut state = test_state();
        let keymap = ExactOnlyKeymap { cmd: "focus-up" };
        let input = resolve_input_with(&keymap);

        let result = resolver.resolve_with_keymap(&key('k'), &mut state, &input);
        assert!(matches!(result, ResolveResult::ModeTransition(ModeTransition::Pop { .. })));
    }

    #[test]
    fn test_cancel_command_exact_only() {
        // Test cancel via ExactOnly
        struct CancelExactKeymap;
        impl KeymapQuery for CancelExactKeymap {
            fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
                KeyLookupState::ExactOnly(crate::ids::CANCEL_TO_NORMAL)
            }
        }

        let resolver = VimWindowResolver::new();
        let mut state = test_state();
        let keymap = CancelExactKeymap;
        let input = resolve_input_with(&keymap);

        let result = resolver.resolve_with_keymap(&key('q'), &mut state, &input);
        assert!(matches!(
            result,
            ResolveResult::ModeTransition(ModeTransition::Pop {
                result: Some(PopResult::Cancelled),
            })
        ));
    }
}
