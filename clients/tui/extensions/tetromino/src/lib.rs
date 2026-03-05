#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Tetromino game TUI extension.
//!
//! Renders a centered tetromino game board as an overlay popup when active.
//! Receives state from the server via `apply_notification()` and
//! renders through `RenderBackend`.

pub mod render;

use reovim_driver_display::render_backend::{RenderBackend, TuiExtension};

/// Board dimensions from the server game logic.
const BOARD_HEIGHT: usize = 22;
const BOARD_WIDTH: usize = 12;

/// Deserialized tetromino state from server notification.
#[derive(Debug, Default)]
pub(crate) struct TetrominoData {
    pub active: bool,
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

impl TuiExtension for TetrominoExtension {
    fn kind(&self) -> &'static str {
        "tetromino"
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
        self.data.paused = json["paused"].as_bool().unwrap_or(false);
        self.data.game_over = json["gameOver"].as_bool().unwrap_or(false);
        self.data.score = json["score"].as_u64().unwrap_or(0);
        self.data.level = json["level"].as_u64().unwrap_or(0);
        self.data.lines_cleared = json["linesCleared"].as_u64().unwrap_or(0);
        json["nextPiece"]
            .as_str()
            .unwrap_or("")
            .clone_into(&mut self.data.next_piece);
        json["nextPieceColor"]
            .as_str()
            .unwrap_or("")
            .clone_into(&mut self.data.next_piece_color);

        // Parse board (2D array of color strings)
        if let Some(rows) = json["board"].as_array() {
            self.data.board = rows
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

        // Parse active piece
        self.data.active_piece = parse_piece_data(&json["activePiece"]);

        // Parse ghost piece
        self.data.ghost_piece = parse_piece_data(&json["ghostPiece"]);

        // Parse held piece
        if json["heldPiece"].is_object() {
            let held = &json["heldPiece"];
            self.data.held_piece = Some(HeldPieceData {
                piece_type: held["type"].as_str().unwrap_or("").to_owned(),
                color: held["color"].as_str().unwrap_or("").to_owned(),
            });
        } else {
            self.data.held_piece = None;
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
        assert_eq!(ext.kind(), "tetromino");
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
        assert_eq!(ext.data.next_piece, "T");
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
        assert_eq!(piece.color, "amber");
        assert_eq!(piece.cells.len(), 4);
    }

    #[test]
    fn apply_notification_no_active_piece() {
        let mut ext = TetrominoExtension::new();
        let json = r#"{"active":true,"paused":false,"gameOver":true,"score":0,"level":0,"linesCleared":0,"nextPiece":"T","nextPieceColor":"rose","board":[],"activePiece":null}"#;
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
        assert_eq!(ghost.color, "amber");
    }

    #[test]
    fn apply_notification_with_held_piece() {
        let mut ext = TetrominoExtension::new();
        let json = make_game_json_with_held(false, false, 0, 0, 0);
        ext.apply_notification(&json);
        assert!(ext.data.held_piece.is_some());
        let held = ext.data.held_piece.as_ref().unwrap();
        assert_eq!(held.piece_type, "T");
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
            "type": "I",
            "color": "amber",
            "cells": [[0, 4], [0, 5], [0, 6], [0, 7]],
        });
        let piece = super::parse_piece_data(&json).unwrap();
        assert_eq!(piece.color, "amber");
        assert_eq!(piece.cells.len(), 4);
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
            r#"{"type":"I","color":"amber","cells":[[0,4],[0,5],[0,6],[0,7]]}"#.to_owned()
        };

        let ghost_piece = if game_over {
            "null".to_owned()
        } else {
            r#"{"type":"I","color":"amber","cells":[[20,4],[20,5],[20,6],[20,7]]}"#.to_owned()
        };

        format!(
            r#"{{"active":true,"paused":{paused},"gameOver":{game_over},"score":{score},"level":{level},"linesCleared":{lines},"nextPiece":"T","nextPieceColor":"rose","board":{board},"activePiece":{active_piece},"ghostPiece":{ghost_piece},"heldPiece":null}}"#
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
        // Replace "heldPiece":null with an actual held piece
        base.replace(r#""heldPiece":null"#, r#""heldPiece":{"type":"T","color":"rose"}"#)
    }
}
