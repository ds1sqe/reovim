use super::*;

#[test]
fn test_default_no_active_snippet() {
    let state = SnippetSessionState::default();
    assert!(state.active.is_none());
}

#[test]
fn test_create_returns_default() {
    let state = SnippetSessionState::create();
    assert!(state.active.is_none());
}

#[test]
fn test_set_active_snippet() {
    use {
        crate::{parser, variables::VariableContext},
        reovim_domain_text::Position,
    };

    let mut state = SnippetSessionState::default();
    let body = parser::parse("$1").unwrap();
    let (_, snippet) = ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    state.active = Some(snippet);
    assert!(state.active.is_some());
}

#[test]
fn test_clear_active_snippet() {
    use {
        crate::{parser, variables::VariableContext},
        reovim_domain_text::Position,
    };

    let mut state = SnippetSessionState::default();
    let body = parser::parse("$1").unwrap();
    let (_, snippet) = ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    state.active = Some(snippet);
    state.active = None;
    assert!(state.active.is_none());
}
