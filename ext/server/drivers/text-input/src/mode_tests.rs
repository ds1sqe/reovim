use reovim_kernel::api::v1::{CommandId, ModeId, ModuleId};

use crate::{KeyCode, KeyEvent, KeySequence, Keybinding, KeybindingTarget, Modifiers};

#[test]
fn test_key_sequence_from_typed_keys_uses_contract_tokens() {
    let seq = KeySequence::from_keys(&[
        KeyEvent::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL),
        KeyEvent::new(KeyCode::Char('h')),
    ]);

    assert_eq!(seq.as_slice(), ["<C-w>".to_owned(), "h".to_owned()].as_slice());
    assert_eq!(seq.to_string(), "<C-w>h");
}

#[test]
fn test_key_sequence_parse_normalized_tokens() {
    let seq = KeySequence::parse("<Escape>gg").unwrap();
    assert_eq!(seq.as_slice(), ["<Esc>".to_owned(), "g".to_owned(), "g".to_owned()].as_slice());
}

#[test]
fn test_keybinding_new() {
    let module = ModuleId::new("test");
    let mode = ModeId::new(module.clone(), "normal");
    let cmd = CommandId::new(module, "test-cmd");

    let binding = Keybinding::new(KeySequence::new(), mode.clone(), cmd.clone(), "Test");

    assert_eq!(binding.mode, mode);
    assert_eq!(binding.command, cmd);
    assert_eq!(binding.description, "Test");
}

#[test]
fn test_keybinding_from_str() {
    let module = ModuleId::new("test");
    let mode = ModeId::new(module.clone(), "normal");
    let cmd = CommandId::new(module, "down");

    let binding = Keybinding::from_str("j", mode.clone(), cmd.clone(), "Move down").unwrap();

    assert_eq!(binding.keys.as_slice(), ["j".to_owned()].as_slice());
    assert_eq!(binding.mode, mode);
    assert_eq!(binding.command, cmd);
}

#[test]
fn test_keybinding_target_command() {
    let module = ModuleId::new("test");
    let cmd = CommandId::new(module, "test-cmd");
    let target = KeybindingTarget::Command(cmd.clone());

    assert!(target.is_command());
    assert!(!target.is_mode());
    assert_eq!(target.as_command(), Some(&cmd));
    assert_eq!(target.as_mode(), None);
}

#[test]
fn test_keybinding_target_mode() {
    let module = ModuleId::new("test");
    let mode = ModeId::new(module, "insert");
    let target = KeybindingTarget::EnterMode(mode.clone());

    assert!(!target.is_command());
    assert!(target.is_mode());
    assert_eq!(target.as_command(), None);
    assert_eq!(target.as_mode(), Some(&mode));
}

#[test]
fn test_keybinding_target_from_command() {
    let module = ModuleId::new("test");
    let cmd = CommandId::new(module, "test-cmd");
    let target: KeybindingTarget = cmd.clone().into();

    assert!(target.is_command());
    assert_eq!(target.as_command(), Some(&cmd));
}

#[test]
fn test_keybinding_target_from_mode() {
    let module = ModuleId::new("test");
    let mode = ModeId::new(module, "insert");
    let target: KeybindingTarget = mode.clone().into();

    assert!(target.is_mode());
    assert_eq!(target.as_mode(), Some(&mode));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_keybinding_target_display() {
    let module = ModuleId::new("test");
    let cmd = CommandId::new(module.clone(), "delete");
    let mode = ModeId::new(module, "insert");

    let cmd_target = KeybindingTarget::Command(cmd);
    assert_eq!(format!("{cmd_target}"), "test:delete");

    let mode_target = KeybindingTarget::EnterMode(mode);
    assert_eq!(format!("{mode_target}"), "enter:test:insert");
}

#[test]
fn test_keybinding_target_equality() {
    let module = ModuleId::new("test");
    let cmd1 = CommandId::new(module.clone(), "cmd");
    let cmd2 = CommandId::new(module.clone(), "cmd");
    let mode = ModeId::new(module, "insert");

    let target1 = KeybindingTarget::Command(cmd1);
    let target2 = KeybindingTarget::Command(cmd2);
    let target3 = KeybindingTarget::EnterMode(mode);

    assert_eq!(target1, target2);
    assert_ne!(target1, target3);
}
