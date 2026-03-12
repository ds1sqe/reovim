use {
    reovim_driver_session::SessionExtension,
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

use crate::{BindingInfo, BindingLayer, KeyCode, KeyEvent, KeySequence, PendingBindings};

const TEST_MODULE: ModuleId = ModuleId::new("test");

fn test_mode() -> reovim_kernel::api::v1::ModeId {
    reovim_kernel::api::v1::ModeId::new(TEST_MODULE, "normal")
}

fn goto_top_info() -> BindingInfo {
    BindingInfo::from_command(CommandId::new(TEST_MODULE, "goto-top"), BindingLayer::Policy)
}

fn goto_def_info() -> BindingInfo {
    BindingInfo::from_command(CommandId::new(TEST_MODULE, "goto-definition"), BindingLayer::Policy)
}

#[test]
fn test_pending_bindings_create_default() {
    let pb = PendingBindings::create();
    assert!(pb.mode_prefix.is_empty());
    assert!(pb.pending_keys.is_empty());
    assert!(pb.continuations.is_empty());
    assert!(!pb.is_active());
}

#[test]
fn test_pending_bindings_is_active() {
    let mut pb = PendingBindings::create();
    assert!(!pb.is_active());

    pb.continuations.push((KeySequence::new(), goto_top_info()));
    assert!(pb.is_active());
}

#[test]
fn test_pending_bindings_clear() {
    let mut pb = PendingBindings::create();
    pb.mode_prefix = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('d'))]);
    pb.pending_keys = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('g'))]);
    pb.mode = test_mode();
    pb.continuations.push((KeySequence::new(), goto_top_info()));

    assert!(pb.is_active());

    pb.clear();
    assert!(pb.mode_prefix.is_empty());
    assert!(pb.pending_keys.is_empty());
    assert!(pb.continuations.is_empty());
    assert!(!pb.is_active());
}

#[test]
fn test_pending_bindings_mode_prefix() {
    let mut pb = PendingBindings::create();
    let d_key = KeyEvent::new(KeyCode::Char('d'));
    pb.mode_prefix = KeySequence::from_keys(&[d_key]);
    pb.continuations.push((KeySequence::new(), goto_top_info()));

    assert!(pb.is_active());
    assert!(!pb.mode_prefix.is_empty());
    assert!(pb.pending_keys.is_empty());
}

#[test]
fn test_pending_bindings_mode_prefix_with_pending_keys() {
    let mut pb = PendingBindings::create();
    pb.mode_prefix = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('d'))]);
    pb.pending_keys = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('i'))]);
    pb.continuations.push((KeySequence::new(), goto_top_info()));

    assert!(pb.is_active());
    assert!(!pb.mode_prefix.is_empty());
    assert!(!pb.pending_keys.is_empty());
}

#[test]
fn test_pending_bindings_multiple_continuations() {
    let mut pb = PendingBindings::create();
    pb.continuations
        .push((KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('g'))]), goto_top_info()));
    pb.continuations
        .push((KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('d'))]), goto_def_info()));

    assert!(pb.is_active());
    assert_eq!(pb.continuations.len(), 2);
}
