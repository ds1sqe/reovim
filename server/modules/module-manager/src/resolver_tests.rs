use reovim_driver_input::{KeyCode, KeymapQuery};

use super::*;

fn test_state() -> ModeState {
    ModeState::new(ManagerMode::MANAGER_ID)
}

/// Mock keymap that always returns `NotFound`.
struct NotFoundKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for NotFoundKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::NotFound
    }
}

fn resolve_input(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = ManagerMode::MANAGER_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

#[test]
fn resolver_new() {
    let resolver = ManagerResolver::new();
    assert_eq!(resolver.mode_id(), &ManagerMode::MANAGER_ID);
}

#[test]
fn resolver_default() {
    let resolver = ManagerResolver::default();
    assert_eq!(resolver.mode_id(), &ManagerMode::MANAGER_ID);
}

#[test]
fn unbound_key_not_handled() {
    let resolver = ManagerResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let k = KeyEvent::new(KeyCode::Char('z'));
    let result = resolver.resolve_with_keymap(&k, &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn escape_not_handled_without_binding() {
    let resolver = ManagerResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let k = KeyEvent::new(KeyCode::Escape);
    let result = resolver.resolve_with_keymap(&k, &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn no_inheritance() {
    let resolver = ManagerResolver::new();
    assert!(resolver.inherits_from().is_none());
}

#[test]
fn reset_is_noop() {
    let mut resolver = ManagerResolver::new();
    resolver.reset();
    assert_eq!(resolver.mode_id(), &ManagerMode::MANAGER_ID);
}

/// Mock keymap that returns `ExactOnly` for 'j'.
struct JKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for JKeymap {
    fn query(&self, _mode: &ModeId, keys: &KeySequence) -> KeyLookupState {
        if keys.len() == 1 && matches!(keys.as_slice()[0].code, KeyCode::Char('j')) {
            KeyLookupState::ExactOnly(crate::ids::NEXT)
        } else {
            KeyLookupState::NotFound
        }
    }
}

#[test]
fn bound_key_executes() {
    let resolver = ManagerResolver::new();
    let mut state = test_state();
    let keymap = JKeymap;
    let input = resolve_input(&keymap);

    let k = KeyEvent::new(KeyCode::Char('j'));
    let result = resolver.resolve_with_keymap(&k, &mut state, &input);
    assert!(matches!(result, ResolveResult::Execute(..)));
}

/// Mock keymap that returns `PrefixOnly`.
struct PrefixKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for PrefixKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::PrefixOnly
    }
}

#[test]
fn prefix_returns_pending() {
    let resolver = ManagerResolver::new();
    let mut state = test_state();
    let keymap = PrefixKeymap;
    let input = resolve_input(&keymap);

    let k = KeyEvent::new(KeyCode::Char('g'));
    let result = resolver.resolve_with_keymap(&k, &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
}

/// Mock keymap that returns `ExactWithLonger`.
struct ExactWithLongerKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for ExactWithLongerKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::ExactWithLonger {
            exact: crate::ids::CLOSE,
        }
    }
}

#[test]
fn exact_with_longer_executes() {
    let resolver = ManagerResolver::new();
    let mut state = test_state();
    let keymap = ExactWithLongerKeymap;
    let input = resolve_input(&keymap);

    let k = KeyEvent::new(KeyCode::Char('q'));
    let result = resolver.resolve_with_keymap(&k, &mut state, &input);
    assert!(matches!(result, ResolveResult::Execute(..)));
}
