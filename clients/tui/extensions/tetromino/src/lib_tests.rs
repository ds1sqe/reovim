use super::*;

#[test]
fn extension_kind() {
    let ext = TetrominoExtension::new();
    assert_eq!(ext.kind(), "polyblocks");
}

#[test]
fn initially_inactive() {
    let ext = TetrominoExtension::new();
    assert!(!ext.is_active());
}

#[test]
fn default_impl() {
    let ext = TetrominoExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_inactive() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(r#"{"active":false}"#);
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_invalid_json() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification("not json");
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_active() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, false, 100, 1, 5));
    assert!(ext.is_active());
    assert!(!ext.data.paused);
    assert!(!ext.data.game_over);
    assert_eq!(ext.data.score, 100);
    assert_eq!(ext.data.level, 1);
    assert_eq!(ext.data.lines_cleared, 5);
    assert_eq!(ext.data.next_piece, "Tee");
    assert_eq!(ext.data.next_piece_color, "rose");
}

#[test]
fn apply_notification_paused() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(true, false, 0, 0, 0));
    assert!(ext.data.paused);
}

#[test]
fn apply_notification_game_over() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, true, 500, 3, 20));
    assert!(ext.data.game_over);
    assert_eq!(ext.data.score, 500);
}

#[test]
fn apply_notification_with_active_piece() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, false, 0, 0, 0));
    let piece = ext.data.active_piece.as_ref().unwrap();
    assert_eq!(piece.color, "lime");
    assert_eq!(piece.cells.len(), 4);
}

#[test]
fn apply_notification_no_active_piece() {
    let mut ext = TetrominoExtension::new();
    let json = r#"{"active":true,"screen":"game","paused":false,"gameOver":true,"score":0,"level":0,"linesCleared":0,"nextPiece":"Tee","nextPieceColor":"rose","board":[],"activePiece":null}"#;
    ext.apply_notification(json);
    assert!(ext.data.active_piece.is_none());
}

#[test]
fn apply_notification_board_parsed() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, false, 0, 0, 0));
    assert_eq!(ext.data.board.len(), BOARD_HEIGHT);
    assert_eq!(ext.data.board[0].len(), BOARD_WIDTH);
}

#[test]
fn apply_notification_deactivate() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, false, 0, 0, 0));
    assert!(ext.is_active());
    ext.apply_notification(r#"{"active":false}"#);
    assert!(!ext.is_active());
}

#[test]
fn render_does_not_panic_inactive() {
    use reovim_driver_display::FrameBuffer;
    let ext = TetrominoExtension::new();
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
}

#[test]
fn render_does_not_panic_active() {
    use reovim_driver_display::FrameBuffer;
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, false, 100, 1, 5));
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
}

#[test]
fn render_paused_overlay() {
    use reovim_driver_display::FrameBuffer;
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(true, false, 0, 0, 0));
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
}

#[test]
fn render_game_over_overlay() {
    use reovim_driver_display::FrameBuffer;
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, true, 999, 5, 40));
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
}

#[test]
fn render_small_terminal() {
    use reovim_driver_display::FrameBuffer;
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, false, 0, 0, 0));
    let mut fb = FrameBuffer::new(20, 10);
    ext.render(&mut fb);
}

#[test]
fn piece_data_debug() {
    let piece = PieceData {
        color: "amber".to_owned(),
        cells: vec![(0, 0), (0, 1)],
    };
    let debug = format!("{piece:?}");
    assert!(debug.contains("amber"));
}

#[test]
fn tetromino_data_debug() {
    let data = TetrominoData::default();
    let debug = format!("{data:?}");
    assert!(debug.contains("TetrominoData"));
}

#[test]
fn apply_notification_with_ghost_piece() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, false, 0, 0, 0));
    assert!(ext.data.ghost_piece.is_some());
    let ghost = ext.data.ghost_piece.as_ref().unwrap();
    assert_eq!(ghost.color, "lime");
}

#[test]
fn apply_notification_with_held_piece() {
    let mut ext = TetrominoExtension::new();
    let json = make_game_json_with_held(false, false, 0, 0, 0);
    ext.apply_notification(&json);
    assert!(ext.data.held_piece.is_some());
    let held = ext.data.held_piece.as_ref().unwrap();
    assert_eq!(held.piece_type, "Tee");
    assert_eq!(held.color, "rose");
}

#[test]
fn apply_notification_no_held_piece() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(&make_game_json(false, false, 0, 0, 0));
    assert!(ext.data.held_piece.is_none());
}

#[test]
fn parse_piece_data_null() {
    assert!(super::parse_piece_data(&serde_json::Value::Null).is_none());
}

#[test]
fn parse_piece_data_valid() {
    let json: serde_json::Value = serde_json::json!({
        "type": "Bar",
        "color": "lime",
        "cells": [[0, 4], [0, 5], [0, 6], [0, 7]],
    });
    let piece = super::parse_piece_data(&json).unwrap();
    assert_eq!(piece.color, "lime");
    assert_eq!(piece.cells.len(), 4);
}

// =========================================================================
// Menu / Lobby / Room screen tests
// =========================================================================

#[test]
fn apply_notification_menu_screen() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(r#"{"active":true,"screen":"menu"}"#);
    assert!(ext.is_active());
    assert_eq!(ext.data.screen, "menu");
}

#[test]
fn apply_notification_lobby_screen() {
    let mut ext = TetrominoExtension::new();
    let json =
        r#"{"active":true,"screen":"lobby","rooms":[{"id":1,"playerCount":2,"status":"Waiting"}]}"#;
    ext.apply_notification(json);
    assert!(ext.is_active());
    assert_eq!(ext.data.screen, "lobby");
    assert_eq!(ext.data.rooms.len(), 1);
    assert_eq!(ext.data.rooms[0].id, 1);
    assert_eq!(ext.data.rooms[0].player_count, 2);
}

#[test]
fn apply_notification_lobby_empty_rooms() {
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(r#"{"active":true,"screen":"lobby","rooms":[]}"#);
    assert!(ext.data.rooms.is_empty());
}

#[test]
fn apply_notification_room_screen() {
    let mut ext = TetrominoExtension::new();
    let json = r#"{"active":true,"screen":"room","roomId":1,"selfReady":true,"players":[{"id":1,"ready":true},{"id":2,"ready":false}]}"#;
    ext.apply_notification(json);
    assert_eq!(ext.data.screen, "room");
    assert_eq!(ext.data.room_id, 1);
    assert!(ext.data.self_ready);
    assert_eq!(ext.data.players.len(), 2);
    assert!(ext.data.players[0].ready);
    assert!(!ext.data.players[1].ready);
}

#[test]
fn apply_notification_game_with_opponents() {
    let mut ext = TetrominoExtension::new();
    let json = make_game_json_with_opponents();
    ext.apply_notification(&json);
    assert_eq!(ext.data.opponents.len(), 1);
    assert_eq!(ext.data.opponents[0].client_id, 2);
    assert_eq!(ext.data.opponents[0].score, 50);
}

#[test]
fn room_data_debug() {
    let room = RoomData {
        id: 1,
        player_count: 2,
        status: "Waiting".to_owned(),
    };
    let debug = format!("{room:?}");
    assert!(debug.contains("RoomData"));
}

#[test]
fn player_data_debug() {
    let player = PlayerData { id: 1, ready: true };
    let debug = format!("{player:?}");
    assert!(debug.contains("PlayerData"));
}

#[test]
fn opponent_data_debug() {
    let opp = OpponentData {
        client_id: 2,
        board: vec![],
        score: 100,
        game_over: false,
    };
    let debug = format!("{opp:?}");
    assert!(debug.contains("OpponentData"));
}

#[test]
fn held_piece_data_debug() {
    let held = HeldPieceData {
        piece_type: "Tee".to_owned(),
        color: "rose".to_owned(),
    };
    let debug = format!("{held:?}");
    assert!(debug.contains("rose"));
}

#[test]
fn render_menu_no_panic() {
    use reovim_driver_display::FrameBuffer;
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(r#"{"active":true,"screen":"menu"}"#);
    let mut fb = FrameBuffer::new(80, 30);
    ext.render(&mut fb);
}

#[test]
fn render_lobby_no_panic() {
    use reovim_driver_display::FrameBuffer;
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(
        r#"{"active":true,"screen":"lobby","rooms":[{"id":1,"playerCount":1,"status":"Waiting"}]}"#,
    );
    let mut fb = FrameBuffer::new(80, 30);
    ext.render(&mut fb);
}

#[test]
fn render_room_no_panic() {
    use reovim_driver_display::FrameBuffer;
    let mut ext = TetrominoExtension::new();
    ext.apply_notification(r#"{"active":true,"screen":"room","roomId":1,"selfReady":false,"players":[{"id":1,"ready":false}]}"#);
    let mut fb = FrameBuffer::new(80, 30);
    ext.render(&mut fb);
}

/// Helper to build a full game JSON notification.
fn make_game_json(paused: bool, game_over: bool, score: u64, level: u64, lines: u64) -> String {
    let mut board_rows = Vec::new();
    for _ in 0..BOARD_HEIGHT {
        let row: Vec<String> = (0..BOARD_WIDTH).map(|_| "\"\"".to_owned()).collect();
        board_rows.push(format!("[{}]", row.join(",")));
    }
    let board = format!("[{}]", board_rows.join(","));

    let active_piece = if game_over {
        "null".to_owned()
    } else {
        r#"{"type":"Bar","color":"lime","cells":[[0,2],[0,3],[0,4],[0,5]]}"#.to_owned()
    };

    let ghost_piece = if game_over {
        "null".to_owned()
    } else {
        r#"{"type":"Bar","color":"lime","cells":[[15,2],[15,3],[15,4],[15,5]]}"#.to_owned()
    };

    format!(
        r#"{{"active":true,"screen":"game","paused":{paused},"gameOver":{game_over},"score":{score},"level":{level},"linesCleared":{lines},"nextPiece":"Tee","nextPieceColor":"rose","board":{board},"activePiece":{active_piece},"ghostPiece":{ghost_piece},"heldPiece":null}}"#
    )
}

fn make_game_json_with_held(
    paused: bool,
    game_over: bool,
    score: u64,
    level: u64,
    lines: u64,
) -> String {
    let base = make_game_json(paused, game_over, score, level, lines);
    base.replace(r#""heldPiece":null"#, r#""heldPiece":{"type":"Tee","color":"rose"}"#)
}

fn make_game_json_with_opponents() -> String {
    let base = make_game_json(false, false, 100, 1, 5);
    // Add opponents array
    let opponents = r#","opponents":[{"clientId":2,"board":[],"score":50,"gameOver":false}]}"#;
    base.strip_suffix('}').unwrap().to_owned() + opponents
}
