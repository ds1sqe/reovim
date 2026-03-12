use reovim_kernel::api::v1::{CommandId, ModeId, ModuleId};

use crate::{KeyCode, KeyEvent, KeySequence, Keybinding, KeybindingTarget, Modifiers};

#[test]
fn test_key_sequence_new() {
    let seq = KeySequence::new();
    assert!(seq.is_empty());
    assert_eq!(seq.len(), 0);
}

#[test]
fn test_key_sequence_push() {
    let mut seq = KeySequence::new();
    seq.push(KeyEvent::new(KeyCode::Char('g')));
    seq.push(KeyEvent::new(KeyCode::Char('g')));
    assert!(!seq.is_empty());
    assert_eq!(seq.len(), 2);
}

#[test]
fn test_key_sequence_from_keys() {
    let keys = [
        KeyEvent::new(KeyCode::Char('d')),
        KeyEvent::new(KeyCode::Char('d')),
    ];
    let seq = KeySequence::from_keys(&keys);
    assert_eq!(seq.len(), 2);
    assert_eq!(seq.as_slice(), &keys);
}

#[test]
fn test_key_sequence_clear() {
    let mut seq = KeySequence::from_keys(&[
        KeyEvent::new(KeyCode::Char('a')),
        KeyEvent::new(KeyCode::Char('b')),
    ]);
    assert!(!seq.is_empty());
    seq.clear();
    assert!(seq.is_empty());
}

#[test]
fn test_key_sequence_starts_with() {
    let full = KeySequence::from_keys(&[
        KeyEvent::new(KeyCode::Char('g')),
        KeyEvent::new(KeyCode::Char('g')),
    ]);
    let prefix = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('g'))]);
    let other = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('d'))]);

    assert!(full.starts_with(&prefix));
    assert!(full.starts_with(&full));
    assert!(!full.starts_with(&other));
}

#[test]
fn test_key_sequence_parse_simple() {
    let seq = KeySequence::parse("gg").unwrap();
    assert_eq!(seq.len(), 2);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char('g'));
    assert_eq!(seq.as_slice()[1].code, KeyCode::Char('g'));
}

#[test]
fn test_key_sequence_parse_special() {
    let seq = KeySequence::parse("<Esc>").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Escape);
}

#[test]
fn test_key_sequence_parse_ctrl() {
    let seq = KeySequence::parse("<C-w>").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char('w'));
    assert!(seq.as_slice()[0].modifiers.contains(Modifiers::CTRL));
}

#[test]
fn test_key_sequence_parse_mixed() {
    let seq = KeySequence::parse("<C-w>h").unwrap();
    assert_eq!(seq.len(), 2);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char('w'));
    assert!(seq.as_slice()[0].modifiers.contains(Modifiers::CTRL));
    assert_eq!(seq.as_slice()[1].code, KeyCode::Char('h'));
}

#[test]
fn test_key_sequence_parse_empty() {
    assert!(KeySequence::parse("").is_none());
}

#[test]
fn test_key_sequence_parse_unicode_emoji() {
    let seq = KeySequence::parse("\u{1f389}").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char('\u{1f389}'));

    let seq = KeySequence::parse("i\u{1f389}<Esc>").unwrap();
    assert_eq!(seq.len(), 3);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char('i'));
    assert_eq!(seq.as_slice()[1].code, KeyCode::Char('\u{1f389}'));
    assert_eq!(seq.as_slice()[2].code, KeyCode::Escape);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_display() {
    let seq = KeySequence::from_keys(&[
        KeyEvent::new(KeyCode::Char('g')),
        KeyEvent::new(KeyCode::Char('g')),
    ]);
    assert_eq!(format!("{seq}"), "gg");

    let ctrl_w = KeySequence::from_keys(&[KeyEvent::with_modifiers(
        KeyCode::Char('w'),
        Modifiers::CTRL,
    )]);
    assert_eq!(format!("{ctrl_w}"), "<C-w>");
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

    assert_eq!(binding.keys.len(), 1);
    assert_eq!(binding.mode, mode);
    assert_eq!(binding.command, cmd);
}

// ========================================================================
// KeybindingTarget tests
// ========================================================================

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

// ========================================================================
// Additional KeySequence::parse tests for uncovered paths
// ========================================================================

#[test]
fn test_key_sequence_parse_alt_modifier() {
    let seq = KeySequence::parse("<A-x>").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char('x'));
    assert!(seq.as_slice()[0].modifiers.contains(Modifiers::ALT));
}

#[test]
fn test_key_sequence_parse_meta_alias_for_alt() {
    // M- is alias for Alt
    let seq = KeySequence::parse("<M-x>").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char('x'));
    assert!(seq.as_slice()[0].modifiers.contains(Modifiers::ALT));
}

#[test]
fn test_key_sequence_parse_shift_modifier() {
    let seq = KeySequence::parse("<S-Tab>").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Tab);
    assert!(seq.as_slice()[0].modifiers.contains(Modifiers::SHIFT));
}

#[test]
fn test_key_sequence_parse_combined_modifiers() {
    let seq = KeySequence::parse("<C-A-S-x>").unwrap();
    assert_eq!(seq.len(), 1);
    assert!(seq.as_slice()[0].modifiers.contains(Modifiers::CTRL));
    assert!(seq.as_slice()[0].modifiers.contains(Modifiers::ALT));
    assert!(seq.as_slice()[0].modifiers.contains(Modifiers::SHIFT));
}

#[test]
fn test_key_sequence_parse_enter_aliases() {
    for alias in ["<Enter>", "<CR>", "<Return>"] {
        let seq = KeySequence::parse(alias).unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Enter, "alias {alias} did not parse to Enter");
    }
}

#[test]
fn test_key_sequence_parse_escape_aliases() {
    for alias in ["<Esc>", "<Escape>"] {
        let seq = KeySequence::parse(alias).unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(
            seq.as_slice()[0].code,
            KeyCode::Escape,
            "alias {alias} did not parse to Escape"
        );
    }
}

#[test]
fn test_key_sequence_parse_space() {
    let seq = KeySequence::parse("<Space>").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char(' '));
}

#[test]
fn test_key_sequence_parse_backspace_aliases() {
    for alias in ["<BS>", "<Backspace>"] {
        let seq = KeySequence::parse(alias).unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(
            seq.as_slice()[0].code,
            KeyCode::Backspace,
            "alias {alias} did not parse to Backspace"
        );
    }
}

#[test]
fn test_key_sequence_parse_delete_aliases() {
    for alias in ["<Del>", "<Delete>"] {
        let seq = KeySequence::parse(alias).unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(
            seq.as_slice()[0].code,
            KeyCode::Delete,
            "alias {alias} did not parse to Delete"
        );
    }
}

#[test]
fn test_key_sequence_parse_arrow_keys() {
    let cases = [
        ("<Up>", KeyCode::Up),
        ("<Down>", KeyCode::Down),
        ("<Left>", KeyCode::Left),
        ("<Right>", KeyCode::Right),
    ];
    for (notation, expected) in cases {
        let seq = KeySequence::parse(notation).unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, expected, "notation {notation} did not parse correctly");
    }
}

#[test]
fn test_key_sequence_parse_navigation_keys() {
    let cases = [
        ("<Home>", KeyCode::Home),
        ("<End>", KeyCode::End),
        ("<PageUp>", KeyCode::PageUp),
        ("<PageDown>", KeyCode::PageDown),
    ];
    for (notation, expected) in cases {
        let seq = KeySequence::parse(notation).unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, expected, "notation {notation} did not parse correctly");
    }
}

#[test]
fn test_key_sequence_parse_function_keys() {
    for n in 1..=12u8 {
        let notation = format!("<F{n}>");
        let seq = KeySequence::parse(&notation).unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::F(n), "F{n} did not parse correctly");
    }
}

#[test]
fn test_key_sequence_parse_tab() {
    let seq = KeySequence::parse("<Tab>").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Tab);
}

#[test]
fn test_key_sequence_parse_lt_gt() {
    let seq = KeySequence::parse("<lt>").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char('<'));

    let seq = KeySequence::parse("<gt>").unwrap();
    assert_eq!(seq.len(), 1);
    assert_eq!(seq.as_slice()[0].code, KeyCode::Char('>'));
}

#[test]
fn test_key_sequence_parse_invalid_special() {
    // Unknown key name should return None
    assert!(KeySequence::parse("<Unknown>").is_none());
    assert!(KeySequence::parse("<InvalidKey>").is_none());
}

#[test]
fn test_key_sequence_parse_case_insensitive() {
    // parse_special converts to lowercase
    let seq = KeySequence::parse("<ESC>").unwrap();
    assert_eq!(seq.as_slice()[0].code, KeyCode::Escape);

    let seq = KeySequence::parse("<ENTER>").unwrap();
    assert_eq!(seq.as_slice()[0].code, KeyCode::Enter);
}

// ========================================================================
// Additional KeySequence::Display tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_display_special_keys() {
    let test_cases = [
        (KeyCode::Escape, "<Esc>"),
        (KeyCode::Enter, "<Enter>"),
        (KeyCode::Tab, "<Tab>"),
        (KeyCode::Backspace, "<BS>"),
        (KeyCode::Delete, "<Del>"),
        (KeyCode::Up, "<Up>"),
        (KeyCode::Down, "<Down>"),
        (KeyCode::Left, "<Left>"),
        (KeyCode::Right, "<Right>"),
        (KeyCode::Home, "<Home>"),
        (KeyCode::End, "<End>"),
        (KeyCode::PageUp, "<PageUp>"),
        (KeyCode::PageDown, "<PageDown>"),
    ];

    for (code, expected) in test_cases {
        let seq = KeySequence::from_keys(&[KeyEvent::new(code)]);
        assert_eq!(format!("{seq}"), expected, "Display for {code:?} was wrong");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_display_function_key() {
    let seq = KeySequence::from_keys(&[KeyEvent::new(KeyCode::F(5))]);
    assert_eq!(format!("{seq}"), "<F5>");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_display_alt_modifier() {
    let seq =
        KeySequence::from_keys(&[KeyEvent::with_modifiers(KeyCode::Char('x'), Modifiers::ALT)]);
    assert_eq!(format!("{seq}"), "<A-x>");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_display_shift_modifier() {
    let seq = KeySequence::from_keys(&[KeyEvent::with_modifiers(
        KeyCode::Char('a'),
        Modifiers::SHIFT,
    )]);
    assert_eq!(format!("{seq}"), "<S-a>");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_display_combined_modifiers() {
    let seq = KeySequence::from_keys(&[KeyEvent::with_modifiers(
        KeyCode::Char('x'),
        Modifiers::CTRL | Modifiers::ALT,
    )]);
    let display = format!("{seq}");
    assert!(display.contains("C-"));
    assert!(display.contains("A-"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_display_unknown_key() {
    // Test fallback display for keys not in the match
    let seq = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Insert)]);
    assert_eq!(format!("{seq}"), "<?>");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_display_mixed() {
    let seq = KeySequence::from_keys(&[
        KeyEvent::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL),
        KeyEvent::new(KeyCode::Char('h')),
    ]);
    assert_eq!(format!("{seq}"), "<C-w>h");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_display_empty() {
    let seq = KeySequence::new();
    assert_eq!(format!("{seq}"), "");
}

// ========================================================================
// KeySequence default and equality tests
// ========================================================================

#[test]
fn test_key_sequence_default_is_empty() {
    let seq = KeySequence::default();
    assert!(seq.is_empty());
}

#[test]
fn test_key_sequence_hash() {
    use std::collections::HashSet;
    let seq1 = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('j'))]);
    let seq2 = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('j'))]);
    let seq3 = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('k'))]);

    let mut set = HashSet::new();
    set.insert(seq1);
    set.insert(seq2);
    assert_eq!(set.len(), 1); // seq1 and seq2 are equal
    set.insert(seq3);
    assert_eq!(set.len(), 2);
}

// ========================================================================
// Keybinding::from_str additional test
// ========================================================================

#[test]
fn test_keybinding_from_str_invalid_returns_none() {
    let module = ModuleId::new("test");
    let mode = ModeId::new(module.clone(), "normal");
    let cmd = CommandId::new(module, "cmd");

    // Empty string
    let binding = Keybinding::from_str("", mode, cmd, "empty");
    assert!(binding.is_none());
}

#[test]
fn test_keybinding_from_str_special_key() {
    let module = ModuleId::new("test");
    let mode = ModeId::new(module.clone(), "normal");
    let cmd = CommandId::new(module, "escape");

    let binding = Keybinding::from_str("<Esc>", mode, cmd, "Escape").unwrap();
    assert_eq!(binding.keys.len(), 1);
    assert_eq!(binding.keys.as_slice()[0].code, KeyCode::Escape);
}

#[test]
fn test_keybinding_from_str_multi_key() {
    let module = ModuleId::new("test");
    let mode = ModeId::new(module.clone(), "normal");
    let cmd = CommandId::new(module, "goto-top");

    let binding = Keybinding::from_str("gg", mode, cmd, "Go to top").unwrap();
    assert_eq!(binding.keys.len(), 2);
}
