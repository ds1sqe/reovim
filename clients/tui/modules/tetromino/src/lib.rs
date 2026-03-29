#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Tetromino game native `ClientModule`.
//!
//! Renders a centered tetromino game board as an overlay popup when active.
//! Receives state from the server via `on_notification()` and renders
//! through `RenderSurface`.

pub mod render;

use reovim_client_driver::{
    ChromePosition, ClientModule, ClientModuleError, ModuleContext, PlatformCapabilities,
    ProbeResult, Rect, RenderSurface, Version,
};

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

/// Module kind and ID constant.
const KIND: &str = "polyblocks";

/// Tetromino game native `ClientModule`.
///
/// Renders the game board as a centered popup overlay when active.
pub struct TetrominoModule {
    data: TetrominoData,
}

impl TetrominoModule {
    /// Create a new inactive module.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: TetrominoData::default(),
        }
    }
}

impl Default for TetrominoModule {
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

/// Parse a board JSON array into `Vec<Vec<String>>`.
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

#[cfg_attr(coverage_nightly, coverage(off))]
impl ClientModule for TetrominoModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Polyblocks"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_chrome(&self) -> bool {
        true
    }

    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Overlay
    }

    fn chrome_priority(&self) -> u16 {
        95
    }

    fn on_notification(&mut self, data: &str) {
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

    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        render::render_tetromino(surface, &self.data, bounds);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
