use reovim_input_codec::{KeyCode, KeyEvent, Modifiers};

use crate::macros::*;

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

fn key_with_mod(c: char, mods: Modifiers) -> KeyEvent {
    KeyEvent::with_modifiers(KeyCode::Char(c), mods)
}

// ========================================================================
// key_to_notation tests
// ========================================================================

#[test]
fn test_key_to_notation_char() {
    assert_eq!(key_to_notation(&key('a')), "a");
    assert_eq!(key_to_notation(&key('Z')), "Z");
    assert_eq!(key_to_notation(&key('5')), "5");
    assert_eq!(key_to_notation(&key(' ')), " ");
}

#[test]
fn test_key_to_notation_special() {
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Escape)), "<Esc>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Enter)), "<Enter>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Tab)), "<Tab>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Backspace)), "<BS>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Delete)), "<Del>");
}

#[test]
fn test_key_to_notation_arrows() {
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Up)), "<Up>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Down)), "<Down>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Left)), "<Left>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Right)), "<Right>");
}

#[test]
fn test_key_to_notation_function_keys() {
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::F(1))), "<F1>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::F(12))), "<F12>");
}

#[test]
fn test_key_to_notation_modifiers() {
    assert_eq!(key_to_notation(&key_with_mod('w', Modifiers::CTRL)), "<C-w>");
    assert_eq!(key_to_notation(&key_with_mod('x', Modifiers::ALT)), "<A-x>");
    assert_eq!(key_to_notation(&key_with_mod('a', Modifiers::SHIFT)), "<S-a>");
}

#[test]
fn test_key_to_notation_combined_modifiers() {
    let mods = Modifiers::CTRL | Modifiers::ALT;
    assert_eq!(key_to_notation(&key_with_mod('d', mods)), "<C-A-d>");
}

#[test]
fn test_key_to_notation_angle_brackets() {
    assert_eq!(key_to_notation(&key('<')), "<lt>");
    assert_eq!(key_to_notation(&key('>')), "<gt>");
}

// ========================================================================
// keys_to_notation tests
// ========================================================================

#[test]
fn test_keys_to_notation_empty() {
    assert_eq!(keys_to_notation(&[]), "");
}

#[test]
fn test_keys_to_notation_simple() {
    let keys = [key('d'), key('w')];
    assert_eq!(keys_to_notation(&keys), "dw");
}

#[test]
fn test_keys_to_notation_mixed() {
    let keys = [
        key('i'),
        key('h'),
        key('e'),
        key('l'),
        key('l'),
        key('o'),
        KeyEvent::new(KeyCode::Escape),
    ];
    assert_eq!(keys_to_notation(&keys), "ihello<Esc>");
}

// ========================================================================
// notation_to_keys tests
// ========================================================================

#[test]
fn test_notation_to_keys_simple() {
    let keys = notation_to_keys("dw").unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].code, KeyCode::Char('d'));
    assert_eq!(keys[1].code, KeyCode::Char('w'));
}

#[test]
fn test_notation_to_keys_special() {
    let keys = notation_to_keys("<Esc>").unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].code, KeyCode::Escape);
}

#[test]
fn test_notation_to_keys_mixed() {
    let keys = notation_to_keys("ihello<Esc>").unwrap();
    assert_eq!(keys.len(), 7);
    assert_eq!(keys[0].code, KeyCode::Char('i'));
    assert_eq!(keys[6].code, KeyCode::Escape);
}

#[test]
fn test_notation_to_keys_ctrl() {
    let keys = notation_to_keys("<C-w>h").unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].code, KeyCode::Char('w'));
    assert!(keys[0].modifiers.contains(Modifiers::CTRL));
    assert_eq!(keys[1].code, KeyCode::Char('h'));
}

#[test]
fn test_notation_to_keys_empty() {
    assert!(notation_to_keys("").is_none());
}

// ========================================================================
// MacroContent tests
// ========================================================================

#[test]
fn test_macro_content_new() {
    let keys = vec![key('d'), key('w')];
    let macro_content = MacroContent::new(keys.clone());
    assert_eq!(macro_content.keys, keys);
}

#[test]
fn test_macro_content_empty() {
    let macro_content = MacroContent::empty();
    assert!(macro_content.is_empty());
    assert_eq!(macro_content.len(), 0);
}

#[test]
fn test_macro_content_to_notation() {
    let macro_content = MacroContent::new(vec![key('d'), key('w'), KeyEvent::new(KeyCode::Escape)]);
    assert_eq!(macro_content.to_notation(), "dw<Esc>");
}

#[test]
fn test_macro_content_from_notation() {
    let macro_content = MacroContent::from_notation("dw<Esc>").unwrap();
    assert_eq!(macro_content.len(), 3);
    assert_eq!(macro_content.keys[0].code, KeyCode::Char('d'));
    assert_eq!(macro_content.keys[2].code, KeyCode::Escape);
}

#[test]
fn test_macro_content_display() {
    let macro_content = MacroContent::new(vec![key('j'), key('j')]);
    assert_eq!(format!("{macro_content}"), "jj");
}

// ========================================================================
// Roundtrip tests (serialize -> deserialize -> same)
// ========================================================================

#[test]
fn test_roundtrip_simple() {
    let original = vec![key('d'), key('w')];
    let notation = keys_to_notation(&original);
    let parsed = notation_to_keys(&notation).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn test_roundtrip_with_special() {
    let original = vec![key('i'), key('h'), key('i'), KeyEvent::new(KeyCode::Escape)];
    let notation = keys_to_notation(&original);
    let parsed = notation_to_keys(&notation).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn test_roundtrip_with_ctrl() {
    let original = vec![key_with_mod('w', Modifiers::CTRL), key('h')];
    let notation = keys_to_notation(&original);
    let parsed = notation_to_keys(&notation).unwrap();

    assert_eq!(original.len(), parsed.len());
    assert_eq!(original[0].code, parsed[0].code);
    assert!(parsed[0].modifiers.contains(Modifiers::CTRL));
    assert_eq!(original[1].code, parsed[1].code);
}

// ========================================================================
// Additional key_to_notation edge cases
// ========================================================================

#[test]
fn test_key_to_notation_home_end() {
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Home)), "<Home>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::End)), "<End>");
}

#[test]
fn test_key_to_notation_page_up_down() {
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::PageUp)), "<PageUp>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::PageDown)), "<PageDown>");
}

#[test]
fn test_key_to_notation_backtab() {
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::BackTab)), "<S-Tab>");
}

#[test]
fn test_key_to_notation_f_keys_range() {
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::F(1))), "<F1>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::F(5))), "<F5>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::F(10))), "<F10>");
    assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::F(12))), "<F12>");
}

#[test]
fn test_key_to_notation_special_with_modifiers() {
    // Escape with ctrl
    let key_esc_ctrl = KeyEvent::with_modifiers(KeyCode::Escape, Modifiers::CTRL);
    assert_eq!(key_to_notation(&key_esc_ctrl), "<C-Esc>");

    // Enter with alt
    let key_enter_alt = KeyEvent::with_modifiers(KeyCode::Enter, Modifiers::ALT);
    assert_eq!(key_to_notation(&key_enter_alt), "<A-Enter>");

    // Tab with shift
    let key_tab_shift = KeyEvent::with_modifiers(KeyCode::Tab, Modifiers::SHIFT);
    assert_eq!(key_to_notation(&key_tab_shift), "<S-Tab>");

    // Backspace with ctrl
    let key_bs_ctrl = KeyEvent::with_modifiers(KeyCode::Backspace, Modifiers::CTRL);
    assert_eq!(key_to_notation(&key_bs_ctrl), "<C-BS>");

    // Delete with alt
    let key_del_alt = KeyEvent::with_modifiers(KeyCode::Delete, Modifiers::ALT);
    assert_eq!(key_to_notation(&key_del_alt), "<A-Del>");
}

#[test]
fn test_key_to_notation_arrows_with_modifiers() {
    let key_up_ctrl = KeyEvent::with_modifiers(KeyCode::Up, Modifiers::CTRL);
    assert_eq!(key_to_notation(&key_up_ctrl), "<C-Up>");

    let key_down_shift = KeyEvent::with_modifiers(KeyCode::Down, Modifiers::SHIFT);
    assert_eq!(key_to_notation(&key_down_shift), "<S-Down>");
}

#[test]
fn test_key_to_notation_all_three_modifiers() {
    let mods = Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT;
    assert_eq!(key_to_notation(&key_with_mod('a', mods)), "<C-A-S-a>");
}

#[test]
fn test_key_to_notation_f_key_with_modifier() {
    let key_f1_ctrl = KeyEvent::with_modifiers(KeyCode::F(1), Modifiers::CTRL);
    assert_eq!(key_to_notation(&key_f1_ctrl), "<C-F1>");
}

// ========================================================================
// Additional MacroContent tests
// ========================================================================

#[test]
fn test_macro_content_default() {
    let mc = MacroContent::default();
    assert!(mc.is_empty());
    assert_eq!(mc.len(), 0);
}

#[test]
fn test_macro_content_len_nonzero() {
    let mc = MacroContent::new(vec![key('a'), key('b'), key('c')]);
    assert_eq!(mc.len(), 3);
    assert!(!mc.is_empty());
}

#[test]
fn test_macro_content_from_notation_invalid() {
    // Empty string
    let result = MacroContent::from_notation("");
    assert!(result.is_none());
}

#[test]
fn test_macro_content_debug() {
    let mc = MacroContent::new(vec![key('d')]);
    let debug = format!("{mc:?}");
    assert!(debug.contains("MacroContent"));
}

#[test]
fn test_macro_content_clone() {
    let mc = MacroContent::new(vec![key('x'), key('y')]);
    let cloned = mc.clone();
    assert_eq!(cloned.len(), mc.len());
    assert_eq!(cloned.keys, mc.keys);
}

// ========================================================================
// Additional keys_to_notation tests
// ========================================================================

#[test]
fn test_keys_to_notation_single_char() {
    assert_eq!(keys_to_notation(&[key('a')]), "a");
}

#[test]
fn test_keys_to_notation_single_special() {
    assert_eq!(keys_to_notation(&[KeyEvent::new(KeyCode::Escape)]), "<Esc>");
}

#[test]
fn test_keys_to_notation_complex_sequence() {
    let keys = [
        key_with_mod('w', Modifiers::CTRL),
        key('h'),
        KeyEvent::new(KeyCode::Escape),
    ];
    assert_eq!(keys_to_notation(&keys), "<C-w>h<Esc>");
}

// ========================================================================
// Roundtrip edge cases
// ========================================================================

#[test]
fn test_roundtrip_macro_content() {
    let original = MacroContent::new(vec![key('d'), key('w'), KeyEvent::new(KeyCode::Escape)]);
    let notation = original.to_notation();
    let restored = MacroContent::from_notation(&notation).unwrap();
    assert_eq!(original.keys, restored.keys);
}

#[test]
fn test_roundtrip_single_char() {
    let original = vec![key('x')];
    let notation = keys_to_notation(&original);
    let parsed = notation_to_keys(&notation).unwrap();
    assert_eq!(original, parsed);
}
