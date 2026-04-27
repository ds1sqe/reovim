use {super::*, crossterm::event::KeyEventKind};

fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    }
}

#[test]
fn test_simple_char() {
    let key = make_key(KeyCode::Char('a'), KeyModifiers::NONE);
    assert_eq!(InputHandler::key_to_notation(&key), Some("a".to_string()));
}

#[test]
fn test_uppercase_char() {
    let key = make_key(KeyCode::Char('A'), KeyModifiers::SHIFT);
    assert_eq!(InputHandler::key_to_notation(&key), Some("A".to_string()));
}

#[test]
fn test_ctrl_char() {
    let key = make_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<C-c>".to_string()));
}

#[test]
fn test_alt_char() {
    let key = make_key(KeyCode::Char('x'), KeyModifiers::ALT);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<M-x>".to_string()));
}

#[test]
fn test_escape() {
    let key = make_key(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<Esc>".to_string()));
}

#[test]
fn test_enter() {
    let key = make_key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<CR>".to_string()));
}

#[test]
fn test_arrow_keys() {
    let key = make_key(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<Up>".to_string()));

    let key = make_key(KeyCode::Down, KeyModifiers::CONTROL);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<C-Down>".to_string()));
}

#[test]
fn test_function_keys() {
    let key = make_key(KeyCode::F(1), KeyModifiers::NONE);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<F1>".to_string()));

    let key = make_key(KeyCode::F(12), KeyModifiers::SHIFT);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<S-F12>".to_string()));
}

#[test]
fn test_space() {
    let key = make_key(KeyCode::Char(' '), KeyModifiers::NONE);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<Space>".to_string()));
}

#[test]
fn test_tab() {
    let key = make_key(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<Tab>".to_string()));
}

#[test]
fn test_shift_tab() {
    let key = make_key(KeyCode::Tab, KeyModifiers::SHIFT);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<S-Tab>".to_string()));
}

#[test]
fn test_special_keys() {
    assert_eq!(
        InputHandler::key_to_notation(&make_key(KeyCode::Backspace, KeyModifiers::NONE)),
        Some("<BS>".to_string())
    );
    assert_eq!(
        InputHandler::key_to_notation(&make_key(KeyCode::Delete, KeyModifiers::NONE)),
        Some("<Del>".to_string())
    );
    assert_eq!(
        InputHandler::key_to_notation(&make_key(KeyCode::Insert, KeyModifiers::NONE)),
        Some("<Insert>".to_string())
    );
    assert_eq!(
        InputHandler::key_to_notation(&make_key(KeyCode::Home, KeyModifiers::NONE)),
        Some("<Home>".to_string())
    );
    assert_eq!(
        InputHandler::key_to_notation(&make_key(KeyCode::End, KeyModifiers::NONE)),
        Some("<End>".to_string())
    );
    assert_eq!(
        InputHandler::key_to_notation(&make_key(KeyCode::PageUp, KeyModifiers::NONE)),
        Some("<PageUp>".to_string())
    );
    assert_eq!(
        InputHandler::key_to_notation(&make_key(KeyCode::PageDown, KeyModifiers::NONE)),
        Some("<PageDown>".to_string())
    );
}

#[test]
fn test_left_right_arrows() {
    assert_eq!(
        InputHandler::key_to_notation(&make_key(KeyCode::Left, KeyModifiers::NONE)),
        Some("<Left>".to_string())
    );
    assert_eq!(
        InputHandler::key_to_notation(&make_key(KeyCode::Right, KeyModifiers::NONE)),
        Some("<Right>".to_string())
    );
}

#[test]
fn test_ctrl_alt_char() {
    let key = make_key(KeyCode::Char('x'), KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<C-M-x>".to_string()));
}

#[test]
fn test_alt_shift_lowercase() {
    let key = make_key(KeyCode::Char('a'), KeyModifiers::ALT | KeyModifiers::SHIFT);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<M-A>".to_string()));
}

#[test]
fn test_less_than() {
    let key = make_key(KeyCode::Char('<'), KeyModifiers::NONE);
    assert_eq!(InputHandler::key_to_notation(&key), Some("<lt>".to_string()));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_unknown_key_code() {
    let key = make_key(KeyCode::Null, KeyModifiers::NONE);
    assert_eq!(InputHandler::key_to_notation(&key), None);
}

#[test]
fn test_event_to_keys_non_key_event() {
    let event = Event::Resize(80, 24);
    assert_eq!(InputHandler::event_to_keys(&event), None);
}

#[test]
fn test_event_to_keys_key_event() {
    let event = Event::Key(make_key(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(InputHandler::event_to_keys(&event), Some("a".to_string()));
}
