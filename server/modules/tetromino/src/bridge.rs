//! Tetromino extension state bridge.
//!
//! Serializes [`TetrominoState`] to JSON for gRPC transmission to clients.
//! The TUI extension consumes this JSON to render the game board.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{BridgeContext, ExtensionScope, ExtensionStateBridge},
};

use crate::{
    game,
    state::{PlayerStatus, TetrominoLobbyState, TetrominoScreen, TetrominoState},
};

/// Bridge for tetromino game state.
///
/// Reads [`TetrominoState`] from the client's `ExtensionMap` and
/// serializes it to JSON for the TUI extension.
pub struct TetrominoBridge;

impl ExtensionStateBridge for TetrominoBridge {
    fn kind(&self) -> &'static str {
        reovim_extension_kinds::POLYBLOCKS
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<TetrominoState>()?;

        if !state.active {
            return Some(serde_json::json!({ "active": false }));
        }

        match state.screen {
            TetrominoScreen::Menu => Some(serde_json::json!({
                "active": true,
                "screen": "menu",
            })),
            TetrominoScreen::Lobby
            | TetrominoScreen::Room
            | TetrominoScreen::Countdown
            | TetrominoScreen::Result => {
                // These screens need shared extensions via snapshot_with_context
                // Fall back to menu if no context available
                Some(serde_json::json!({
                    "active": true,
                    "screen": "menu",
                }))
            }
            TetrominoScreen::Game => Some(snapshot_game(state)),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn snapshot_with_context(
        &self,
        extensions: &ExtensionMap,
        context: &BridgeContext<'_>,
    ) -> Option<serde_json::Value> {
        let state = extensions.get::<TetrominoState>()?;

        if !state.active {
            return Some(serde_json::json!({ "active": false }));
        }

        match state.screen {
            TetrominoScreen::Menu => Some(serde_json::json!({
                "active": true,
                "screen": "menu",
            })),
            TetrominoScreen::Lobby => {
                let rooms = context
                    .shared_extensions
                    .get::<TetrominoLobbyState>()
                    .map(|l| {
                        l.room_summaries()
                            .iter()
                            .map(|s| {
                                serde_json::json!({
                                    "id": s.id.0,
                                    "playerCount": s.player_count,
                                    "status": format!("{:?}", s.status),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                Some(serde_json::json!({
                    "active": true,
                    "screen": "lobby",
                    "rooms": rooms,
                }))
            }
            TetrominoScreen::Room => {
                let room_id = state.current_room?;
                let lobby = context.shared_extensions.get::<TetrominoLobbyState>()?;
                let room = lobby.rooms.get(&room_id)?;
                let players: Vec<_> = room
                    .players
                    .iter()
                    .map(|(id, status)| {
                        serde_json::json!({
                            "id": id.as_usize(),
                            "ready": matches!(status, PlayerStatus::Ready),
                        })
                    })
                    .collect();
                let self_ready = room
                    .players
                    .get(&context.client_id)
                    .is_some_and(|s| matches!(s, PlayerStatus::Ready));
                Some(serde_json::json!({
                    "active": true,
                    "screen": "room",
                    "roomId": room_id.0,
                    "players": players,
                    "selfReady": self_ready,
                }))
            }
            TetrominoScreen::Countdown => {
                let room_id = state.current_room?;
                let lobby = context.shared_extensions.get::<TetrominoLobbyState>()?;
                let remaining = lobby.countdown_remaining(room_id).unwrap_or(0);
                Some(serde_json::json!({
                    "active": true,
                    "screen": "countdown",
                    "roomId": room_id.0,
                    "remaining": remaining,
                }))
            }
            TetrominoScreen::Result => {
                let room_id = state.current_room?;
                let lobby = context.shared_extensions.get::<TetrominoLobbyState>()?;
                let room = lobby.rooms.get(&room_id)?;
                let winner_id = room.winner.map(|w| w.as_usize());
                let self_score = state.game.as_ref().map_or(0, |g| g.score);
                let mut players = vec![serde_json::json!({
                    "id": context.client_id.as_usize(),
                    "score": self_score,
                })];
                context.for_each_opponent(|id, ext| {
                    if let Some(opp) = ext.get::<TetrominoState>()
                        && opp.multiplayer
                    {
                        players.push(serde_json::json!({
                            "id": id.as_usize(),
                            "score": opp.game.as_ref().map_or(0, |g| g.score),
                        }));
                    }
                });
                Some(serde_json::json!({
                    "active": true,
                    "screen": "result",
                    "winnerId": winner_id,
                    "players": players,
                }))
            }
            TetrominoScreen::Game => {
                let mut snap = snapshot_game(state);
                if state.multiplayer {
                    let mut opponents = Vec::new();
                    context.for_each_opponent(|id, ext| {
                        if let Some(opp) = ext.get::<TetrominoState>()
                            && opp.multiplayer
                            && let Some(ref g) = opp.game
                        {
                            opponents.push(serde_json::json!({
                                "clientId": id.as_usize(),
                                "board": serialize_board(&g.board),
                                "score": g.score,
                                "gameOver": g.game_over,
                            }));
                        }
                    });
                    snap["opponents"] = serde_json::json!(opponents);
                }
                Some(snap)
            }
        }
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions.get::<TetrominoState>().is_some_and(|s| s.active)
    }

    #[allow(clippy::too_many_lines)] // multi-phase state machine (countdown, gravity, line clear, match end)
    fn tick(
        &self,
        client_extensions: &mut ExtensionMap,
        shared_extensions: &mut ExtensionMap,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> bool {
        // Extract active/screen/room early, drop borrow before re-borrowing.
        let (active, screen, room_id) = {
            let game_state = client_extensions.get_or_insert::<TetrominoState>();
            (game_state.active, game_state.screen, game_state.current_room)
        };

        if !active {
            return false;
        }

        // Catch up players whose screen is still Room after another player
        // triggered the countdown via ReadyToggle. The tick scheduler was
        // started for all players, so this runs on every tick.
        if screen == TetrominoScreen::Room
            && let Some(room_id) = room_id
        {
            let lobby = shared_extensions.get_or_insert::<TetrominoLobbyState>();
            if lobby.is_in_progress(room_id) {
                let game_state = client_extensions.get_or_insert::<TetrominoState>();
                game_state.screen = TetrominoScreen::Game;
                game_state.multiplayer = true;
                game_state.start_game();
                return true;
            }
            if lobby.is_countdown(room_id) {
                let game_state = client_extensions.get_or_insert::<TetrominoState>();
                game_state.screen = TetrominoScreen::Countdown;
                // Fall through to countdown handler below
            }
        }

        // Re-read screen since it may have been updated by Room catch-up above.
        let game_state = client_extensions.get_or_insert::<TetrominoState>();

        // Handle countdown phase: check if countdown expired → start match
        if game_state.screen == TetrominoScreen::Countdown {
            if let Some(room_id) = game_state.current_room {
                let lobby = shared_extensions.get_or_insert::<TetrominoLobbyState>();
                if lobby.countdown_remaining(room_id) == Some(0) {
                    lobby.start_match(room_id);
                    let game_state = client_extensions.get_or_insert::<TetrominoState>();
                    game_state.screen = TetrominoScreen::Game;
                    game_state.multiplayer = true;
                    game_state.start_game();
                    return true;
                }
                // Another client's tick already called start_match() —
                // this client's countdown_remaining() returns None because
                // the room moved to InProgress. Catch up.
                if lobby.is_in_progress(room_id) {
                    let game_state = client_extensions.get_or_insert::<TetrominoState>();
                    game_state.screen = TetrominoScreen::Game;
                    game_state.multiplayer = true;
                    game_state.start_game();
                    return true;
                }
            }
            // Still counting down — notify for display update
            return true;
        }

        if game_state.paused {
            return false;
        }

        // Multiplayer match-finished detection: if the match ended (another player
        // died and triggered finish), transition the surviving player to Result.
        if game_state.multiplayer
            && game_state.screen == TetrominoScreen::Game
            && let Some(room_id) = game_state.current_room
        {
            let lobby = shared_extensions.get_or_insert::<TetrominoLobbyState>();
            if lobby.is_finished(room_id) {
                let game_state = client_extensions.get_or_insert::<TetrominoState>();
                game_state.screen = TetrominoScreen::Result;
                return true;
            }
        }

        // Multiplayer death detection: when a player's game ends, mark them dead
        // and check if the match should finish. Must run before the game_over
        // early return so we can transition to the Result screen.
        if game_state.multiplayer
            && game_state.game.as_ref().is_some_and(|g| g.game_over)
            && game_state.screen == TetrominoScreen::Game
            && let (Some(room_id), Some(client_id)) =
                (game_state.current_room, game_state.client_id)
        {
            let lobby = shared_extensions.get_or_insert::<TetrominoLobbyState>();
            lobby.mark_dead(room_id, client_id);
            if lobby.alive_count(room_id) <= 1
                && let Some(winner) = lobby.last_alive(room_id)
            {
                lobby.finish_match(room_id, winner);
            }
            let game_state = client_extensions.get_or_insert::<TetrominoState>();
            game_state.screen = TetrominoScreen::Result;
            return true;
        }

        if game_state.game.as_ref().is_none_or(|g| g.game_over) {
            return false;
        }

        let mut changed = false;

        // Apply pending garbage from opponents (multiplayer only)
        if game_state.multiplayer
            && let (Some(room_id), Some(client_id)) =
                (game_state.current_room, game_state.client_id)
            && let Some(lobby) = shared_extensions.get_mut::<TetrominoLobbyState>()
        {
            let garbage = lobby.take_garbage(room_id, client_id);
            if let Some(ref mut g) = game_state.game {
                let mut rng = rand::thread_rng();
                for count in garbage {
                    for _ in 0..count {
                        let gap = rand::Rng::gen_range(&mut rng, 0..game::BOARD_WIDTH);
                        game::apply_garbage_line(g, gap);
                    }
                    changed = true;
                }
            }
        }

        // Gravity tick
        let game_state = client_extensions.get_or_insert::<TetrominoState>();
        if game_state.should_tick() {
            game_state.apply_tick();
            changed = true;
        }

        changed
    }
}

/// Serialize the game state to JSON.
fn snapshot_game(state: &TetrominoState) -> serde_json::Value {
    let Some(ref game_state) = state.game else {
        return serde_json::json!({ "active": false });
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

    serde_json::json!({
        "active": true,
        "screen": "game",
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
    })
}

/// Serialize the board as a 2D array of color strings.
fn serialize_board(board: &game::Board) -> Vec<Vec<&'static str>> {
    board
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| match cell {
                    Some(game::BlockColor::White) => "white",
                    Some(game::BlockColor::Amber) => "amber",
                    Some(game::BlockColor::Teal) => "teal",
                    Some(game::BlockColor::Rose) => "rose",
                    Some(game::BlockColor::Sky) => "sky",
                    Some(game::BlockColor::Lime) => "lime",
                    Some(game::BlockColor::Violet) => "violet",
                    Some(game::BlockColor::Coral) => "coral",
                    Some(game::BlockColor::Blue) => "blue",
                    Some(game::BlockColor::Gold) => "gold",
                    Some(game::BlockColor::Grey) => "grey",
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
#[path = "bridge_tests.rs"]
mod tests;
