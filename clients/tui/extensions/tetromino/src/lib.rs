//! Tetromino game TUI extension.
//!
//! Renders a centered tetromino game board as an overlay popup when active.
//! Receives state from the server via `apply_notification()` and
//! renders through `RenderBackend`.

pub mod render;

use reovim_driver_display::render_backend::{RenderBackend, TuiExtension};

/// Board dimensions from the server game logic.
const BOARD_HEIGHT: usize = 16;
const BOARD_WIDTH: usize = 8;

/// Deserialized tetromino state from server notification.
#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct TetrominoData {
    pub active: bool,
    pub screen: String,
    pub paused: bool,
    pub game_over: bool,
    pub score: u64,
    pub level: u64,
    pub lines_cleared: u64,
    pub next_piece: String,
    pub next_piece_color: String,
    pub board: Vec<Vec<String>>,
    pub active_piece: Option<PieceData>,
    pub ghost_piece: Option<PieceData>,
    pub held_piece: Option<HeldPieceData>,
    pub rooms: Vec<RoomData>,
    pub players: Vec<PlayerData>,
    pub self_ready: bool,
    pub room_id: u64,
    pub opponents: Vec<OpponentData>,
    pub countdown_remaining: u64,
    pub winner_id: Option<u64>,
    pub result_players: Vec<ResultPlayerData>,
}

/// Deserialized held piece data (type + color only, no cells).
#[derive(Debug)]
pub(crate) struct HeldPieceData {
    pub piece_type: String,
    pub color: String,
}

/// Deserialized active piece data.
#[derive(Debug)]
pub(crate) struct PieceData {
    pub color: String,
    pub cells: Vec<(i64, i64)>,
}

/// Deserialized room data for lobby display.
#[derive(Debug)]
pub(crate) struct RoomData {
    pub id: u64,
    pub player_count: u64,
    pub status: String,
}

/// Deserialized player data for room display.
#[derive(Debug)]
pub(crate) struct PlayerData {
    pub id: u64,
    pub ready: bool,
}

/// Deserialized opponent data for multiplayer display.
#[derive(Debug)]
pub(crate) struct OpponentData {
    pub client_id: u64,
    pub board: Vec<Vec<String>>,
    pub score: u64,
    pub game_over: bool,
}

/// Deserialized player score data for result screen.
#[derive(Debug)]
pub(crate) struct ResultPlayerData {
    pub id: u64,
    pub score: u64,
}

/// Tetromino game TUI extension.
///
/// Renders the game board as a centered popup when active.
pub struct TetrominoExtension {
    data: TetrominoData,
}

impl TetrominoExtension {
    /// Create a new inactive extension.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: TetrominoData::default(),
        }
    }
}

impl Default for TetrominoExtension {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a piece data object from JSON (shared by active and ghost pieces).
fn parse_piece_data(json: &serde_json::Value) -> Option<PieceData> {
    if !json.is_object() {
        return None;
    }
    let color = json["color"].as_str().unwrap_or("").to_owned();
    let cells = json["cells"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|cell| {
                    let coords = cell.as_array()?;
                    Some((coords.first()?.as_i64()?, coords.get(1)?.as_i64()?))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(PieceData { color, cells })
}

/// Parse lobby screen fields from JSON.
fn parse_lobby(data: &mut TetrominoData, json: &serde_json::Value) {
    data.rooms = json["rooms"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    Some(RoomData {
                        id: r["id"].as_u64()?,
                        player_count: r["playerCount"].as_u64().unwrap_or(0),
                        status: r["status"].as_str().unwrap_or("").to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
}

/// Parse result screen fields from JSON.
fn parse_result(data: &mut TetrominoData, json: &serde_json::Value) {
    data.winner_id = json["winnerId"].as_u64();
    data.result_players = json["players"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    Some(ResultPlayerData {
                        id: p["id"].as_u64()?,
                        score: p["score"].as_u64().unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
}

/// Parse countdown screen fields from JSON.
fn parse_countdown(data: &mut TetrominoData, json: &serde_json::Value) {
    data.room_id = json["roomId"].as_u64().unwrap_or(0);
    data.countdown_remaining = json["remaining"].as_u64().unwrap_or(0);
}

/// Parse room screen fields from JSON.
fn parse_room(data: &mut TetrominoData, json: &serde_json::Value) {
    data.room_id = json["roomId"].as_u64().unwrap_or(0);
    data.self_ready = json["selfReady"].as_bool().unwrap_or(false);
    data.players = json["players"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    Some(PlayerData {
                        id: p["id"].as_u64()?,
                        ready: p["ready"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
}

/// Parse game screen fields from JSON.
fn parse_game_screen(data: &mut TetrominoData, json: &serde_json::Value) {
    data.paused = json["paused"].as_bool().unwrap_or(false);
    data.game_over = json["gameOver"].as_bool().unwrap_or(false);
    data.score = json["score"].as_u64().unwrap_or(0);
    data.level = json["level"].as_u64().unwrap_or(0);
    data.lines_cleared = json["linesCleared"].as_u64().unwrap_or(0);
    json["nextPiece"]
        .as_str()
        .unwrap_or("")
        .clone_into(&mut data.next_piece);
    json["nextPieceColor"]
        .as_str()
        .unwrap_or("")
        .clone_into(&mut data.next_piece_color);

    // Parse board
    if let Some(rows) = json["board"].as_array() {
        data.board = rows
            .iter()
            .map(|row| {
                row.as_array()
                    .map(|cols| {
                        cols.iter()
                            .map(|c| c.as_str().unwrap_or("").to_owned())
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect();
    }

    data.active_piece = parse_piece_data(&json["activePiece"]);
    data.ghost_piece = parse_piece_data(&json["ghostPiece"]);

    if json["heldPiece"].is_object() {
        let held = &json["heldPiece"];
        data.held_piece = Some(HeldPieceData {
            piece_type: held["type"].as_str().unwrap_or("").to_owned(),
            color: held["color"].as_str().unwrap_or("").to_owned(),
        });
    } else {
        data.held_piece = None;
    }

    // Parse opponents (multiplayer)
    data.opponents = json["opponents"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|o| {
                    Some(OpponentData {
                        client_id: o["clientId"].as_u64()?,
                        board: parse_board_json(&o["board"]),
                        score: o["score"].as_u64().unwrap_or(0),
                        game_over: o["gameOver"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
}

/// Parse a board JSON array into Vec<Vec<String>>.
fn parse_board_json(json: &serde_json::Value) -> Vec<Vec<String>> {
    json.as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.as_array()
                        .map(|cols| {
                            cols.iter()
                                .map(|c| c.as_str().unwrap_or("").to_owned())
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

impl TuiExtension for TetrominoExtension {
    fn kind(&self) -> &'static str {
        "polyblocks"
    }

    fn is_active(&self) -> bool {
        self.data.active
    }

    fn apply_notification(&mut self, data: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };

        let active = json["active"].as_bool().unwrap_or(false);
        if !active {
            self.data.active = false;
            return;
        }

        self.data.active = true;

        // Parse screen field
        json["screen"]
            .as_str()
            .unwrap_or("game")
            .clone_into(&mut self.data.screen);

        match self.data.screen.as_str() {
            "menu" => {
                // No extra data for menu
            }
            "lobby" => parse_lobby(&mut self.data, &json),
            "room" => parse_room(&mut self.data, &json),
            "countdown" => parse_countdown(&mut self.data, &json),
            "result" => parse_result(&mut self.data, &json),
            _ => parse_game_screen(&mut self.data, &json),
        }
    }

    fn render(&self, backend: &mut dyn RenderBackend) {
        render::render_tetromino(backend, &self.data);
    }
}

#[cfg(test)]
mod tests {
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
        let json = r#"{"active":true,"screen":"lobby","rooms":[{"id":1,"playerCount":2,"status":"Waiting"}]}"#;
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
        ext.apply_notification(r#"{"active":true,"screen":"lobby","rooms":[{"id":1,"playerCount":1,"status":"Waiting"}]}"#);
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

    #[test]
    fn apply_notification_countdown_screen() {
        let mut ext = TetrominoExtension::new();
        ext.apply_notification(r#"{"active":true,"screen":"countdown","roomId":1,"remaining":2}"#);
        assert!(ext.is_active());
        assert_eq!(ext.data.screen, "countdown");
        assert_eq!(ext.data.room_id, 1);
        assert_eq!(ext.data.countdown_remaining, 2);
    }

    #[test]
    fn render_countdown_no_panic() {
        use reovim_driver_display::FrameBuffer;
        let mut ext = TetrominoExtension::new();
        ext.apply_notification(r#"{"active":true,"screen":"countdown","roomId":1,"remaining":3}"#);
        let mut fb = FrameBuffer::new(80, 30);
        ext.render(&mut fb);
    }

    #[test]
    fn apply_notification_result_screen() {
        let mut ext = TetrominoExtension::new();
        ext.apply_notification(r#"{"active":true,"screen":"result","winnerId":2,"players":[{"id":1,"score":100},{"id":2,"score":500}]}"#);
        assert!(ext.is_active());
        assert_eq!(ext.data.screen, "result");
        assert_eq!(ext.data.winner_id, Some(2));
        assert_eq!(ext.data.result_players.len(), 2);
        assert_eq!(ext.data.result_players[0].id, 1);
        assert_eq!(ext.data.result_players[0].score, 100);
        assert_eq!(ext.data.result_players[1].id, 2);
        assert_eq!(ext.data.result_players[1].score, 500);
    }

    #[test]
    fn apply_notification_result_no_winner() {
        let mut ext = TetrominoExtension::new();
        ext.apply_notification(
            r#"{"active":true,"screen":"result","winnerId":null,"players":[{"id":1,"score":0}]}"#,
        );
        assert_eq!(ext.data.screen, "result");
        assert!(ext.data.winner_id.is_none());
        assert_eq!(ext.data.result_players.len(), 1);
    }

    #[test]
    fn render_result_no_panic() {
        use reovim_driver_display::FrameBuffer;
        let mut ext = TetrominoExtension::new();
        ext.apply_notification(
            r#"{"active":true,"screen":"result","winnerId":1,"players":[{"id":1,"score":300}]}"#,
        );
        let mut fb = FrameBuffer::new(80, 30);
        ext.render(&mut fb);
    }

    #[test]
    fn result_player_data_debug() {
        let p = ResultPlayerData { id: 1, score: 999 };
        let debug = format!("{p:?}");
        assert!(debug.contains("999"));
    }

    // ========================================================================
    // MC/DC edge case tests for JSON parsing
    // ========================================================================

    #[test]
    fn parse_board_json_non_array() {
        let val = serde_json::json!("not an array");
        let board = parse_board_json(&val);
        assert!(board.is_empty());
    }

    #[test]
    fn parse_board_json_row_not_array() {
        let val = serde_json::json!([42, "not_array"]);
        let board = parse_board_json(&val);
        assert_eq!(board.len(), 2);
        assert!(board[0].is_empty()); // 42 is not an array
        assert!(board[1].is_empty()); // "not_array" is not an array
    }

    #[test]
    fn parse_piece_data_non_object() {
        let val = serde_json::json!("not an object");
        assert!(parse_piece_data(&val).is_none());
    }

    #[test]
    fn parse_piece_data_cell_not_array() {
        let val = serde_json::json!({"color": "red", "cells": ["not_array", 42]});
        let piece = parse_piece_data(&val).unwrap();
        assert!(piece.cells.is_empty()); // filter_map skips non-arrays
    }

    #[test]
    fn parse_piece_data_cell_missing_coords() {
        let val = serde_json::json!({"color": "red", "cells": [[], [1]]});
        let piece = parse_piece_data(&val).unwrap();
        // [] → first() returns None, [1] → get(1) returns None
        assert!(piece.cells.is_empty());
    }

    #[test]
    fn parse_piece_data_cell_coords_not_numbers() {
        let val = serde_json::json!({"color": "red", "cells": [["a", "b"]]});
        let piece = parse_piece_data(&val).unwrap();
        // as_i64() returns None for strings
        assert!(piece.cells.is_empty());
    }

    #[test]
    fn parse_lobby_room_missing_id() {
        let mut data = TetrominoData::default();
        let json = serde_json::json!({"rooms": [{"playerCount": 2, "status": "waiting"}]});
        parse_lobby(&mut data, &json);
        assert!(data.rooms.is_empty()); // id is required (uses ?)
    }

    #[test]
    fn parse_result_player_missing_id() {
        let mut data = TetrominoData::default();
        let json = serde_json::json!({"players": [{"score": 100}]});
        parse_result(&mut data, &json);
        assert!(data.result_players.is_empty()); // id is required (uses ?)
    }
}
