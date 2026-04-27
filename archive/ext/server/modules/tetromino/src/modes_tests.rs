use super::*;

#[test]
fn mode_module() {
    assert_eq!(TetrominoMode::module(), ids::MODULE);
}

#[test]
fn play_discriminant() {
    assert_eq!(TetrominoMode::Play.discriminant(), 0);
}

#[test]
fn paused_discriminant() {
    assert_eq!(TetrominoMode::Paused.discriminant(), 1);
}

#[test]
fn menu_discriminant() {
    assert_eq!(TetrominoMode::Menu.discriminant(), 2);
}

#[test]
fn lobby_discriminant() {
    assert_eq!(TetrominoMode::Lobby.discriminant(), 3);
}

#[test]
fn room_discriminant() {
    assert_eq!(TetrominoMode::Room.discriminant(), 4);
}

#[test]
fn play_display_name() {
    assert_eq!(TetrominoMode::Play.display_name(), "PLAY");
}

#[test]
fn paused_display_name() {
    assert_eq!(TetrominoMode::Paused.display_name(), "PAUSED");
}

#[test]
fn menu_display_name() {
    assert_eq!(TetrominoMode::Menu.display_name(), "MENU");
}

#[test]
fn lobby_display_name() {
    assert_eq!(TetrominoMode::Lobby.display_name(), "LOBBY");
}

#[test]
fn room_display_name() {
    assert_eq!(TetrominoMode::Room.display_name(), "ROOM");
}

#[test]
fn play_cursor_hidden() {
    assert_eq!(TetrominoMode::Play.cursor_style(), CursorStyle::Hidden);
}

#[test]
fn paused_cursor_hidden() {
    assert_eq!(TetrominoMode::Paused.cursor_style(), CursorStyle::Hidden);
}

#[test]
fn menu_cursor_hidden() {
    assert_eq!(TetrominoMode::Menu.cursor_style(), CursorStyle::Hidden);
}

#[test]
fn lobby_cursor_hidden() {
    assert_eq!(TetrominoMode::Lobby.cursor_style(), CursorStyle::Hidden);
}

#[test]
fn room_cursor_hidden() {
    assert_eq!(TetrominoMode::Room.cursor_style(), CursorStyle::Hidden);
}

#[test]
fn play_no_char_input() {
    assert!(!TetrominoMode::Play.accepts_char_input());
}

#[test]
fn paused_no_char_input() {
    assert!(!TetrominoMode::Paused.accepts_char_input());
}

#[test]
fn menu_no_char_input() {
    assert!(!TetrominoMode::Menu.accepts_char_input());
}

#[test]
fn lobby_no_char_input() {
    assert!(!TetrominoMode::Lobby.accepts_char_input());
}

#[test]
fn room_no_char_input() {
    assert!(!TetrominoMode::Room.accepts_char_input());
}

#[test]
fn play_no_selection() {
    assert!(!TetrominoMode::Play.has_selection());
}

#[test]
fn paused_no_selection() {
    assert!(!TetrominoMode::Paused.has_selection());
}

#[test]
fn menu_no_selection() {
    assert!(!TetrominoMode::Menu.has_selection());
}

#[test]
fn lobby_no_selection() {
    assert!(!TetrominoMode::Lobby.has_selection());
}

#[test]
fn room_no_selection() {
    assert!(!TetrominoMode::Room.has_selection());
}

#[test]
fn play_no_inheritance() {
    assert!(TetrominoMode::Play.inherits_from().is_none());
}

#[test]
fn paused_no_inheritance() {
    assert!(TetrominoMode::Paused.inherits_from().is_none());
}

#[test]
fn menu_no_inheritance() {
    assert!(TetrominoMode::Menu.inherits_from().is_none());
}

#[test]
fn lobby_no_inheritance() {
    assert!(TetrominoMode::Lobby.inherits_from().is_none());
}

#[test]
fn room_no_inheritance() {
    assert!(TetrominoMode::Room.inherits_from().is_none());
}

#[test]
fn play_not_entry() {
    assert!(!TetrominoMode::Play.is_entry());
}

#[test]
fn paused_not_entry() {
    assert!(!TetrominoMode::Paused.is_entry());
}

#[test]
fn menu_not_entry() {
    assert!(!TetrominoMode::Menu.is_entry());
}

#[test]
fn lobby_not_entry() {
    assert!(!TetrominoMode::Lobby.is_entry());
}

#[test]
fn room_not_entry() {
    assert!(!TetrominoMode::Room.is_entry());
}

#[test]
fn play_mode_id_matches_constant() {
    assert_eq!(TetrominoMode::Play.id(), TetrominoMode::PLAY_ID);
}

#[test]
fn paused_mode_id_matches_constant() {
    assert_eq!(TetrominoMode::Paused.id(), TetrominoMode::PAUSED_ID);
}

#[test]
fn menu_mode_id_matches_constant() {
    assert_eq!(TetrominoMode::Menu.id(), TetrominoMode::MENU_ID);
}

#[test]
fn lobby_mode_id_matches_constant() {
    assert_eq!(TetrominoMode::Lobby.id(), TetrominoMode::LOBBY_ID);
}

#[test]
fn room_mode_id_matches_constant() {
    assert_eq!(TetrominoMode::Room.id(), TetrominoMode::ROOM_ID);
}

#[test]
fn result_discriminant() {
    assert_eq!(TetrominoMode::Result.discriminant(), 5);
}

#[test]
fn result_display_name() {
    assert_eq!(TetrominoMode::Result.display_name(), "RESULT");
}

#[test]
fn result_cursor_hidden() {
    assert_eq!(TetrominoMode::Result.cursor_style(), CursorStyle::Hidden);
}

#[test]
fn result_no_char_input() {
    assert!(!TetrominoMode::Result.accepts_char_input());
}

#[test]
fn result_no_selection() {
    assert!(!TetrominoMode::Result.has_selection());
}

#[test]
fn result_no_inheritance() {
    assert!(TetrominoMode::Result.inherits_from().is_none());
}

#[test]
fn result_not_entry() {
    assert!(!TetrominoMode::Result.is_entry());
}

#[test]
fn result_mode_id_matches_constant() {
    assert_eq!(TetrominoMode::Result.id(), TetrominoMode::RESULT_ID);
}

#[test]
fn all_modes() {
    assert_eq!(TetrominoMode::ALL.len(), 6);
    assert_eq!(TetrominoMode::ALL[0], TetrominoMode::Play);
    assert_eq!(TetrominoMode::ALL[1], TetrominoMode::Paused);
    assert_eq!(TetrominoMode::ALL[2], TetrominoMode::Menu);
    assert_eq!(TetrominoMode::ALL[3], TetrominoMode::Lobby);
    assert_eq!(TetrominoMode::ALL[4], TetrominoMode::Room);
    assert_eq!(TetrominoMode::ALL[5], TetrominoMode::Result);
}

#[test]
fn mode_copy_clone() {
    let mode = TetrominoMode::Play;
    let copied = mode;
    assert_eq!(mode, copied);
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn mode_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(TetrominoMode::Play);
    set.insert(TetrominoMode::Paused);
    set.insert(TetrominoMode::Menu);
    set.insert(TetrominoMode::Lobby);
    set.insert(TetrominoMode::Room);
    set.insert(TetrominoMode::Result);
    assert!(set.contains(&TetrominoMode::Play));
    assert!(set.contains(&TetrominoMode::Paused));
    assert!(set.contains(&TetrominoMode::Menu));
    assert!(set.contains(&TetrominoMode::Lobby));
    assert!(set.contains(&TetrominoMode::Room));
    assert!(set.contains(&TetrominoMode::Result));
    assert_eq!(set.len(), 6);
}

#[test]
fn mode_debug() {
    let debug = format!("{:?}", TetrominoMode::Play);
    assert!(debug.contains("Play"));
    let debug = format!("{:?}", TetrominoMode::Paused);
    assert!(debug.contains("Paused"));
    let debug = format!("{:?}", TetrominoMode::Menu);
    assert!(debug.contains("Menu"));
    let debug = format!("{:?}", TetrominoMode::Lobby);
    assert!(debug.contains("Lobby"));
    let debug = format!("{:?}", TetrominoMode::Room);
    assert!(debug.contains("Room"));
    let debug = format!("{:?}", TetrominoMode::Result);
    assert!(debug.contains("Result"));
}
