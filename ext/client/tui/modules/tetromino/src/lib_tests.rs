use {
    super::*,
    reovim_client_driver::testing::MockPlatformCapabilities,
    reovim_ext_client_tui_cap_cell::CellCapability,
};

// =============================================================================
// Helpers (Plan 23 / 17-β.2b-impl-c bulk migration)
// =============================================================================

fn has_content(g: &CellCapability) -> bool {
    g.iter().any(|(_, c)| c.ch != ' ')
}

fn chrome_render(module: &TetrominoModule, w: u16, h: u16) -> CellCapability {
    let mut surface = CellCapability::new(w, h);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    module.chrome_render(&mut surface, bounds, &MockPlatformCapabilities::new());
    surface
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

fn menu_payload() -> String {
    r#"{"active":true,"screen":"menu"}"#.to_owned()
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn identity() {
    let m = TetrominoModule::new();
    assert_eq!(m.id(), "polyblocks");
    assert_eq!(m.kind(), "polyblocks");
    assert_eq!(m.name(), "Polyblocks");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn default_is_inactive() {
    let m = TetrominoModule::default();
    assert!(!m.data.active);
}

#[test]
fn chrome_role() {
    let m = TetrominoModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 95);
}

#[test]
fn lifecycle() {
    let mut m = TetrominoModule::new();
    assert_eq!(m.exit().unwrap(), ());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activate_menu() {
    let mut m = TetrominoModule::new();
    m.on_notification(&menu_payload());
    assert!(m.data.active);
    assert_eq!(m.data.screen, "menu");
}

#[test]
fn notification_activate_game() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json(false, false, 100, 1, 5));
    assert!(m.data.active);
    assert!(!m.data.paused);
    assert!(!m.data.game_over);
    assert_eq!(m.data.score, 100);
    assert_eq!(m.data.level, 1);
    assert_eq!(m.data.lines_cleared, 5);
    assert_eq!(m.data.next_piece, "Tee");
    assert_eq!(m.data.next_piece_color, "rose");
    assert_eq!(m.data.board.len(), BOARD_HEIGHT);
    assert_eq!(m.data.board[0].len(), BOARD_WIDTH);
}

#[test]
fn notification_activate_lobby() {
    let mut m = TetrominoModule::new();
    let json =
        r#"{"active":true,"screen":"lobby","rooms":[{"id":1,"playerCount":2,"status":"Waiting"}]}"#;
    m.on_notification(json);
    assert!(m.data.active);
    assert_eq!(m.data.screen, "lobby");
    assert_eq!(m.data.rooms.len(), 1);
    assert_eq!(m.data.rooms[0].id, 1);
    assert_eq!(m.data.rooms[0].player_count, 2);
}

#[test]
fn notification_activate_room() {
    let mut m = TetrominoModule::new();
    let json = r#"{"active":true,"screen":"room","roomId":1,"selfReady":true,"players":[{"id":1,"ready":true},{"id":2,"ready":false}]}"#;
    m.on_notification(json);
    assert_eq!(m.data.screen, "room");
    assert_eq!(m.data.room_id, 1);
    assert!(m.data.self_ready);
    assert_eq!(m.data.players.len(), 2);
    assert!(m.data.players[0].ready);
    assert!(!m.data.players[1].ready);
}

#[test]
fn notification_deactivate() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json(false, false, 0, 0, 0));
    assert!(m.data.active);
    m.on_notification(r#"{"active":false}"#);
    assert!(!m.data.active);
}

#[test]
fn notification_invalid_json() {
    let mut m = TetrominoModule::new();
    m.on_notification("not json{{{");
    assert!(!m.data.active);
}

#[test]
fn notification_paused() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json(true, false, 0, 0, 0));
    assert!(m.data.paused);
}

#[test]
fn notification_game_over() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json(false, true, 500, 3, 20));
    assert!(m.data.game_over);
    assert_eq!(m.data.score, 500);
}

#[test]
fn notification_with_active_piece() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json(false, false, 0, 0, 0));
    let piece = m.data.active_piece.as_ref().unwrap();
    assert_eq!(piece.color, "lime");
    assert_eq!(piece.cells.len(), 4);
}

#[test]
fn notification_no_active_piece() {
    let mut m = TetrominoModule::new();
    let json = r#"{"active":true,"screen":"game","paused":false,"gameOver":true,"score":0,"level":0,"linesCleared":0,"nextPiece":"Tee","nextPieceColor":"rose","board":[],"activePiece":null}"#;
    m.on_notification(json);
    assert!(m.data.active_piece.is_none());
}

#[test]
fn notification_with_ghost_piece() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json(false, false, 0, 0, 0));
    assert!(m.data.ghost_piece.is_some());
    let ghost = m.data.ghost_piece.as_ref().unwrap();
    assert_eq!(ghost.color, "lime");
}

#[test]
fn notification_with_held_piece() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json_with_held(false, false, 0, 0, 0));
    assert!(m.data.held_piece.is_some());
    let held = m.data.held_piece.as_ref().unwrap();
    assert_eq!(held.piece_type, "Tee");
    assert_eq!(held.color, "rose");
}

#[test]
fn notification_no_held_piece() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json(false, false, 0, 0, 0));
    assert!(m.data.held_piece.is_none());
}

#[test]
fn notification_game_with_opponents() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json_with_opponents());
    assert_eq!(m.data.opponents.len(), 1);
    assert_eq!(m.data.opponents[0].client_id, 2);
    assert_eq!(m.data.opponents[0].score, 50);
}

#[test]
fn notification_lobby_empty_rooms() {
    let mut m = TetrominoModule::new();
    m.on_notification(r#"{"active":true,"screen":"lobby","rooms":[]}"#);
    assert!(m.data.rooms.is_empty());
}

#[test]
fn notification_countdown() {
    let mut m = TetrominoModule::new();
    m.on_notification(r#"{"active":true,"screen":"countdown","roomId":3,"remaining":5}"#);
    assert_eq!(m.data.screen, "countdown");
    assert_eq!(m.data.room_id, 3);
    assert_eq!(m.data.countdown_remaining, 5);
}

#[test]
fn notification_result() {
    let mut m = TetrominoModule::new();
    m.on_notification(
        r#"{"active":true,"screen":"result","winnerId":1,"players":[{"id":1,"score":500},{"id":2,"score":200}]}"#,
    );
    assert_eq!(m.data.screen, "result");
    assert_eq!(m.data.winner_id, Some(1));
    assert_eq!(m.data.result_players.len(), 2);
}

// =============================================================================
// Parse helper tests
// =============================================================================

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

// =============================================================================
// Debug derive coverage
// =============================================================================

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
fn result_player_data_debug() {
    let rp = ResultPlayerData { id: 1, score: 999 };
    let debug = format!("{rp:?}");
    assert!(debug.contains("999"));
}

// =============================================================================
// Render tests (via chrome_render)
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = TetrominoModule::new();
    let surface = chrome_render(&m, 80, 24);
    assert!(!has_content(&surface));
}

#[test]
fn render_menu_screen() {
    let mut m = TetrominoModule::new();
    m.on_notification(&menu_payload());
    let surface = chrome_render(&m, 80, 30);
    assert!(has_content(&surface));
}

#[test]
fn render_game_screen() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json(false, false, 100, 1, 5));
    let surface = chrome_render(&m, 80, 30);
    assert!(has_content(&surface));
}

#[test]
fn render_small_terminal() {
    let mut m = TetrominoModule::new();
    m.on_notification(&make_game_json(false, false, 0, 0, 0));
    let surface = chrome_render(&m, 20, 10);
    assert!(has_content(&surface)); // "Terminal too small" message
}
