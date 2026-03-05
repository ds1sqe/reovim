//! Tetromino extension state bridge.
//!
//! Serializes [`TetrominoState`] to JSON for gRPC transmission to clients.
//! The TUI extension consumes this JSON to render the game board.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::{game, state::TetrominoState};

/// Bridge for tetromino game state.
///
/// Reads [`TetrominoState`] from the client's `ExtensionMap` and
/// serializes it to JSON for the TUI extension.
pub struct TetrominoBridge;

impl ExtensionStateBridge for TetrominoBridge {
    fn kind(&self) -> &'static str {
        "tetromino"
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<TetrominoState>()?;

        if !state.active {
            return Some(serde_json::json!({ "active": false }));
        }

        let Some(ref game_state) = state.game else {
            return Some(serde_json::json!({ "active": false }));
        };

        let board = serialize_board(&game_state.board);
        let active_piece = game_state.active_piece.as_ref().map(serialize_piece);
        let ghost = game_state
            .active_piece
            .as_ref()
            .map(|p| serialize_piece(&game::ghost_piece(&game_state.board, p)));
        let held = game_state.held_piece.map(|pt| {
            serde_json::json!({
                "type": pt.name(),
                "color": pt.color_name(),
            })
        });

        Some(serde_json::json!({
            "active": true,
            "paused": state.paused,
            "board": board,
            "activePiece": active_piece,
            "ghostPiece": ghost,
            "heldPiece": held,
            "nextPiece": game_state.next_piece.name(),
            "nextPieceColor": game_state.next_piece.color_name(),
            "score": game_state.score,
            "level": game_state.level,
            "linesCleared": game_state.lines_cleared,
            "gameOver": game_state.game_over,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions.get::<TetrominoState>().is_some_and(|s| s.active)
    }
}

/// Serialize the board as a 2D array of color strings.
fn serialize_board(board: &game::Board) -> Vec<Vec<&'static str>> {
    board
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| match cell {
                    Some(game::BlockColor::Amber) => "amber",
                    Some(game::BlockColor::Teal) => "teal",
                    Some(game::BlockColor::Rose) => "rose",
                    Some(game::BlockColor::Sky) => "sky",
                    Some(game::BlockColor::Lime) => "lime",
                    Some(game::BlockColor::Violet) => "violet",
                    Some(game::BlockColor::Coral) => "coral",
                    None => "",
                })
                .collect()
        })
        .collect()
}

/// Serialize a piece to JSON with absolute board positions.
fn serialize_piece(piece: &game::ActivePiece) -> serde_json::Value {
    let cells = game::absolute_cells(piece);
    serde_json::json!({
        "type": piece.piece_type.name(),
        "color": piece.piece_type.color_name(),
        "cells": cells.iter().map(|(r, c)| vec![*r, *c]).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::game::{ActivePiece, BlockColor, PieceType, Rotation},
    };

    #[test]
    fn bridge_kind() {
        assert_eq!(TetrominoBridge.kind(), "tetromino");
    }

    #[test]
    fn bridge_scope() {
        assert_eq!(TetrominoBridge.scope(), ExtensionScope::Client);
    }

    #[test]
    fn snapshot_no_state_returns_none() {
        let map = ExtensionMap::new();
        assert!(TetrominoBridge.snapshot(&map).is_none());
    }

    #[test]
    fn snapshot_inactive() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TetrominoState>();

        let snap = TetrominoBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
        assert!(snap.get("board").is_none());
    }

    #[test]
    fn snapshot_active_no_game() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<TetrominoState>();
        state.active = true;

        let snap = TetrominoBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
    }

    #[test]
    fn snapshot_active_with_game() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<TetrominoState>();
        state.start_game();

        let snap = TetrominoBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        assert_eq!(snap["paused"], false);
        assert_eq!(snap["gameOver"], false);
        assert_eq!(snap["score"], 0);
        assert_eq!(snap["level"], 0);
        assert_eq!(snap["linesCleared"], 0);

        // Board should be 22 rows of 12 columns
        let board = snap["board"].as_array().unwrap();
        assert_eq!(board.len(), 22);
        assert_eq!(board[0].as_array().unwrap().len(), 12);

        // Active piece should exist
        assert!(snap["activePiece"].is_object());
        assert!(snap["activePiece"]["type"].is_string());
        assert!(snap["activePiece"]["color"].is_string());
        assert!(snap["activePiece"]["cells"].is_array());

        // Ghost piece should exist (same piece type, lower row)
        assert!(snap["ghostPiece"].is_object());
        assert!(snap["ghostPiece"]["cells"].is_array());

        // Held piece should be null initially
        assert!(snap["heldPiece"].is_null());

        // Next piece
        assert!(snap["nextPiece"].is_string());
        assert!(snap["nextPieceColor"].is_string());
    }

    #[test]
    fn snapshot_with_held_piece() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<TetrominoState>();
        state.start_game();
        state.hold_piece();

        let snap = TetrominoBridge.snapshot(&map).unwrap();
        assert!(snap["heldPiece"].is_object());
        assert!(snap["heldPiece"]["type"].is_string());
        assert!(snap["heldPiece"]["color"].is_string());
    }

    #[test]
    fn snapshot_paused() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<TetrominoState>();
        state.start_game();
        state.paused = true;

        let snap = TetrominoBridge.snapshot(&map).unwrap();
        assert_eq!(snap["paused"], true);
    }

    #[test]
    fn snapshot_game_over() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<TetrominoState>();
        state.start_game();
        state.game.as_mut().unwrap().game_over = true;
        state.game.as_mut().unwrap().active_piece = None;

        let snap = TetrominoBridge.snapshot(&map).unwrap();
        assert_eq!(snap["gameOver"], true);
        assert!(snap["activePiece"].is_null());
    }

    #[test]
    fn snapshot_board_with_blocks() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<TetrominoState>();
        state.start_game();
        state.game.as_mut().unwrap().board[21][0] = Some(BlockColor::Amber);
        state.game.as_mut().unwrap().board[21][1] = Some(BlockColor::Lime);

        let snap = TetrominoBridge.snapshot(&map).unwrap();
        let board = snap["board"].as_array().unwrap();
        let last_row = board[21].as_array().unwrap();
        assert_eq!(last_row[0], "amber");
        assert_eq!(last_row[1], "lime");
        assert_eq!(last_row[2], "");
    }

    #[test]
    fn is_active_no_state() {
        let map = ExtensionMap::new();
        assert!(!TetrominoBridge.is_active(&map));
    }

    #[test]
    fn is_active_inactive_state() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TetrominoState>();
        assert!(!TetrominoBridge.is_active(&map));
    }

    #[test]
    fn is_active_active_state() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<TetrominoState>();
        state.active = true;
        assert!(TetrominoBridge.is_active(&map));
    }

    #[test]
    fn serialize_board_empty() {
        let board = [[None; game::BOARD_WIDTH]; game::BOARD_HEIGHT];
        let result = serialize_board(&board);
        assert_eq!(result.len(), game::BOARD_HEIGHT);
        for row in &result {
            assert!(row.iter().all(|c| c.is_empty()));
        }
    }

    #[test]
    fn serialize_board_all_colors() {
        let mut board = [[None; game::BOARD_WIDTH]; game::BOARD_HEIGHT];
        board[0][0] = Some(BlockColor::Amber);
        board[0][1] = Some(BlockColor::Teal);
        board[0][2] = Some(BlockColor::Rose);
        board[0][3] = Some(BlockColor::Sky);
        board[0][4] = Some(BlockColor::Lime);
        board[0][5] = Some(BlockColor::Violet);
        board[0][6] = Some(BlockColor::Coral);

        let result = serialize_board(&board);
        assert_eq!(result[0][0], "amber");
        assert_eq!(result[0][1], "teal");
        assert_eq!(result[0][2], "rose");
        assert_eq!(result[0][3], "sky");
        assert_eq!(result[0][4], "lime");
        assert_eq!(result[0][5], "violet");
        assert_eq!(result[0][6], "coral");
    }

    #[test]
    fn serialize_piece_format() {
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 5,
            col: 3,
        };
        let json = serialize_piece(&piece);
        assert_eq!(json["type"], "T");
        assert_eq!(json["color"], "rose");
        let cells = json["cells"].as_array().unwrap();
        assert_eq!(cells.len(), 4);
    }
}
