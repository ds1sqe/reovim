use {
    reovim_driver_text_input::{KeyCode, KeymapQuery},
    reovim_kernel::api::v1::CommandId,
};

use super::*;

// =========================================================================
// Mock keymaps
// =========================================================================

/// Mock keymap that always returns `NotFound`.
struct NotFoundKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for NotFoundKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::NotFound
    }
}

/// Mock keymap that always returns `ExactOnly` with a fixed command.
struct ExactOnlyKeymap(CommandId);

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for ExactOnlyKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::ExactOnly(self.0.clone())
    }
}

/// Mock keymap that always returns `ExactWithLonger`.
struct ExactWithLongerKeymap(CommandId);

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for ExactWithLongerKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::ExactWithLonger {
            exact: self.0.clone(),
        }
    }
}

/// Mock keymap that always returns `PrefixOnly`.
struct PrefixOnlyKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for PrefixOnlyKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::PrefixOnly
    }
}

fn resolve_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = ids::NAVIGATING_MODE;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

fn tab_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Tab)
}

/// Test parent mode for resolver construction.
fn test_parent_mode() -> ModeId {
    ModeId::new(reovim_kernel::api::v1::ModuleId::new("test"), "parent")
}

fn make_resolver() -> SnippetResolver {
    SnippetResolver::with_parent(test_parent_mode())
}

// =========================================================================
// Construction and trait
// =========================================================================

#[test]
fn test_mode_id() {
    let resolver = make_resolver();
    assert_eq!(resolver.mode_id(), &ids::NAVIGATING_MODE);
}

#[test]
fn test_inherits_from_parent() {
    let resolver = make_resolver();
    assert_eq!(resolver.inherits_from(), Some(&test_parent_mode()));
}

#[test]
fn test_with_parent_stores_parent() {
    let parent = ModeId::new(reovim_kernel::api::v1::ModuleId::new("test"), "insert");
    let resolver = SnippetResolver::with_parent(parent.clone());
    assert_eq!(resolver.inherits_from(), Some(&parent));
}

#[test]
fn test_inherits_from_is_some() {
    let resolver = make_resolver();
    assert!(resolver.inherits_from().is_some());
}

// =========================================================================
// resolve_with_keymap
// =========================================================================

#[test]
fn test_resolve_not_found() {
    let resolver = make_resolver();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut state = ModeState::new(ids::NAVIGATING_MODE);

    let result = resolver.resolve_with_keymap(&tab_key(), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_resolve_exact_only() {
    let resolver = make_resolver();
    let cmd = ids::JUMP_NEXT;
    let keymap = ExactOnlyKeymap(cmd.clone());
    let input = resolve_input(&keymap);
    let mut state = ModeState::new(ids::NAVIGATING_MODE);

    let result = resolver.resolve_with_keymap(&tab_key(), &mut state, &input);
    match result {
        ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
        other => panic!("expected Execute, got {other:?}"),
    }
}

#[test]
fn test_resolve_exact_with_longer() {
    let resolver = make_resolver();
    let cmd = ids::JUMP_NEXT;
    let keymap = ExactWithLongerKeymap(cmd.clone());
    let input = resolve_input(&keymap);
    let mut state = ModeState::new(ids::NAVIGATING_MODE);

    let result = resolver.resolve_with_keymap(&tab_key(), &mut state, &input);
    match result {
        ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
        other => panic!("expected Execute, got {other:?}"),
    }
}

#[test]
fn test_resolve_prefix_only() {
    let resolver = make_resolver();
    let keymap = PrefixOnlyKeymap;
    let input = resolve_input(&keymap);
    let mut state = ModeState::new(ids::NAVIGATING_MODE);

    let result = resolver.resolve_with_keymap(&tab_key(), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
}
