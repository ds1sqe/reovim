//! Tetromino board rendering.
//!
//! Renders the game board as a centered popup with score panel.
//!
//! ```text
//! ╭─ TETROMINO ──────────────╮
//! │  ┌────────────────┐     │
//! │  │                │ Next│
//! │  │   ████         │ ██  │
//! │  │    ██          │     │
//! │  │                │Score│
//! │  │                │ 100 │
//! │  │                │Level│
//! │  │                │  1  │
//! │  │                │Lines│
//! │  │                │  5  │
//! │  │  ██████████    │     │
//! │  │  ████████████  │     │
//! │  └────────────────┘     │
//! ╰─────────────────────────╯
//! ```

use {
    reovim_arch::Color,
    reovim_driver_display::{Style, popup_utils::render_box_border, render_backend::RenderBackend},
};

use crate::{BOARD_HEIGHT, BOARD_WIDTH, OpponentData, ResultPlayerData, TetrominoData};

/// Each board cell is 2 characters wide for a square look.
const CELL_WIDTH: u16 = 2;

// Board dimensions are small constants (8, 16) — truncation is impossible.
/// Popup width: board (2 chars * 8 cols) + board borders (2) + side panel (10) + popup padding (4)
#[allow(clippy::cast_possible_truncation)]
const POPUP_INNER_W: u16 = BOARD_WIDTH as u16 * CELL_WIDTH + 2 + 10 + 2;
const POPUP_W: u16 = POPUP_INNER_W + 2; // +2 for outer border
/// Popup height: title (1) + board (22) + board borders (2) + padding (1) + outer border (2)
/// Extra 4 rows for held piece display in side panel.
#[allow(clippy::cast_possible_truncation)]
const POPUP_H: u16 = BOARD_HEIGHT as u16 + 6;

/// Menu/lobby popup dimensions (smaller than game popup).
const MENU_W: u16 = 30;
const MENU_H: u16 = 12;
const LOBBY_W: u16 = 40;
const LOBBY_H: u16 = 18;
const ROOM_W: u16 = 36;
const ROOM_H: u16 = 14;
const COUNTDOWN_W: u16 = 26;
const COUNTDOWN_H: u16 = 7;
const RESULT_W: u16 = 36;
const RESULT_BASE_H: u16 = 10; // base height without player rows

/// Opponent minimap: each cell is 1 char, board is 8 wide, plus border (2).
#[allow(clippy::cast_possible_truncation)]
const MINI_W: u16 = BOARD_WIDTH as u16 + 2;
#[allow(clippy::cast_possible_truncation)]
const MINI_H: u16 = BOARD_HEIGHT as u16 / 2 + 2; // half-height + border

/// Map a color name to a terminal Color.
fn color_for_name(name: &str) -> Color {
    match name {
        "white" => Color::White,
        "amber" => Color::DarkYellow,
        "teal" => Color::DarkCyan,
        "sky" => Color::Cyan,
        "lime" => Color::Green,
        "rose" => Color::Magenta,
        "violet" => Color::Blue,
        "coral" => Color::Red,
        "blue" => Color::DarkBlue,
        "gold" => Color::Yellow,
        // "grey" (garbage blocks) falls through to default DarkGrey
        _ => Color::DarkGrey,
    }
}

/// Render the tetromino game UI, dispatching based on current screen.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn render_tetromino(backend: &mut dyn RenderBackend, data: &TetrominoData) {
    if !data.active {
        return;
    }

    match data.screen.as_str() {
        "menu" => render_menu(backend),
        "lobby" => render_lobby(backend, data),
        "room" => render_room(backend, data),
        "countdown" => render_countdown(backend, data),
        "result" => render_result(backend, data),
        _ => render_game(backend, data),
    }
}

/// Render the main menu screen.
#[allow(clippy::cast_possible_truncation)]
fn render_menu(backend: &mut dyn RenderBackend) {
    let (term_w, term_h) = backend.size();
    if term_w < MENU_W || term_h < MENU_H {
        let msg = "Terminal too small";
        let x = term_w.saturating_sub(msg.len() as u16) / 2;
        backend.write_str(x, term_h / 2, msg, &Style::new().fg(Color::Red));
        return;
    }

    let px = (term_w - MENU_W) / 2;
    let py = (term_h - MENU_H) / 2;

    let bg = Style::new().bg(Color::AnsiValue(234));
    backend.fill_region(px, py, MENU_W, MENU_H, ' ', &bg);

    let border = Style::new().fg(Color::DarkCyan).bg(Color::AnsiValue(234));
    render_box_border(backend, px, py, MENU_W, MENU_H, &border);

    let title_style = Style::new()
        .fg(Color::Cyan)
        .bg(Color::AnsiValue(234))
        .bold();
    backend.write_str(px + 2, py, " POLYBLOCKS ", &title_style);

    let label = Style::new().fg(Color::White).bg(Color::AnsiValue(234));
    let key_style = Style::new()
        .fg(Color::Yellow)
        .bg(Color::AnsiValue(234))
        .bold();
    let dim = Style::new().fg(Color::Grey).bg(Color::AnsiValue(234));

    let cx = px + 4;
    let cy = py + 3;

    backend.write_str(cx, cy, "[s]", &key_style);
    backend.write_str(cx + 4, cy, "Single Player", &label);

    backend.write_str(cx, cy + 2, "[m]", &key_style);
    backend.write_str(cx + 4, cy + 2, "Multiplayer", &label);

    backend.write_str(cx, cy + 4, "[q]", &key_style);
    backend.write_str(cx + 4, cy + 4, "Quit", &dim);
}

/// Render the lobby screen showing available rooms.
#[allow(clippy::cast_possible_truncation)]
fn render_lobby(backend: &mut dyn RenderBackend, data: &TetrominoData) {
    let (term_w, term_h) = backend.size();
    if term_w < LOBBY_W || term_h < LOBBY_H {
        let msg = "Terminal too small";
        let x = term_w.saturating_sub(msg.len() as u16) / 2;
        backend.write_str(x, term_h / 2, msg, &Style::new().fg(Color::Red));
        return;
    }

    let px = (term_w - LOBBY_W) / 2;
    let py = (term_h - LOBBY_H) / 2;

    let bg = Style::new().bg(Color::AnsiValue(234));
    backend.fill_region(px, py, LOBBY_W, LOBBY_H, ' ', &bg);

    let border = Style::new().fg(Color::DarkCyan).bg(Color::AnsiValue(234));
    render_box_border(backend, px, py, LOBBY_W, LOBBY_H, &border);

    let title_style = Style::new()
        .fg(Color::Cyan)
        .bg(Color::AnsiValue(234))
        .bold();
    backend.write_str(px + 2, py, " LOBBY ", &title_style);

    let label = Style::new().fg(Color::White).bg(Color::AnsiValue(234));
    let key_style = Style::new()
        .fg(Color::Yellow)
        .bg(Color::AnsiValue(234))
        .bold();
    let dim = Style::new().fg(Color::Grey).bg(Color::AnsiValue(234));

    let cx = px + 2;
    let mut cy = py + 2;

    backend.write_str(cx, cy, "[c] Create room   [q] Back", &key_style);
    cy += 2;

    if data.rooms.is_empty() {
        backend.write_str(cx, cy, "No rooms. Press [c] to create.", &dim);
    } else {
        backend.write_str(cx, cy, "Rooms (press digit to join):", &label);
        cy += 1;
        // Show up to 9 rooms
        for (i, room) in data.rooms.iter().take(9).enumerate() {
            cy += 1;
            let num = format!("[{}]", i + 1);
            backend.write_str(cx, cy, &num, &key_style);
            let info =
                format!("Room {} - {} players ({})", room.id, room.player_count, room.status);
            backend.write_str(cx + 4, cy, &info, &label);
        }
    }
}

/// Render the room screen showing players and ready status.
#[allow(clippy::cast_possible_truncation)]
fn render_room(backend: &mut dyn RenderBackend, data: &TetrominoData) {
    let (term_w, term_h) = backend.size();
    if term_w < ROOM_W || term_h < ROOM_H {
        let msg = "Terminal too small";
        let x = term_w.saturating_sub(msg.len() as u16) / 2;
        backend.write_str(x, term_h / 2, msg, &Style::new().fg(Color::Red));
        return;
    }

    let px = (term_w - ROOM_W) / 2;
    let py = (term_h - ROOM_H) / 2;

    let bg = Style::new().bg(Color::AnsiValue(234));
    backend.fill_region(px, py, ROOM_W, ROOM_H, ' ', &bg);

    let border = Style::new().fg(Color::DarkCyan).bg(Color::AnsiValue(234));
    render_box_border(backend, px, py, ROOM_W, ROOM_H, &border);

    let title = format!(" ROOM {} ", data.room_id);
    let title_style = Style::new()
        .fg(Color::Cyan)
        .bg(Color::AnsiValue(234))
        .bold();
    backend.write_str(px + 2, py, &title, &title_style);

    let label = Style::new().fg(Color::White).bg(Color::AnsiValue(234));
    let key_style = Style::new()
        .fg(Color::Yellow)
        .bg(Color::AnsiValue(234))
        .bold();
    let ready_style = Style::new()
        .fg(Color::Green)
        .bg(Color::AnsiValue(234))
        .bold();
    let not_ready_style = Style::new().fg(Color::Grey).bg(Color::AnsiValue(234));

    let cx = px + 2;
    let mut cy = py + 2;

    backend.write_str(cx, cy, "[r] Toggle ready  [q] Leave", &key_style);
    cy += 2;

    let status_text = if data.self_ready {
        "READY"
    } else {
        "NOT READY"
    };
    let status_s = if data.self_ready {
        &ready_style
    } else {
        &not_ready_style
    };
    backend.write_str(cx, cy, "You: ", &label);
    backend.write_str(cx + 5, cy, status_text, status_s);
    cy += 2;

    backend.write_str(cx, cy, "Players:", &label);
    for player in &data.players {
        cy += 1;
        let marker = if player.ready { "+" } else { "-" };
        let style = if player.ready {
            &ready_style
        } else {
            &not_ready_style
        };
        let line = format!(" {marker} Player {}", player.id);
        backend.write_str(cx, cy, &line, style);
    }
}

/// Render the countdown screen before a multiplayer match.
#[allow(clippy::cast_possible_truncation)]
fn render_countdown(backend: &mut dyn RenderBackend, data: &TetrominoData) {
    let (term_w, term_h) = backend.size();
    if term_w < COUNTDOWN_W || term_h < COUNTDOWN_H {
        let msg = "Terminal too small";
        let x = term_w.saturating_sub(msg.len() as u16) / 2;
        backend.write_str(x, term_h / 2, msg, &Style::new().fg(Color::Red));
        return;
    }

    let px = (term_w - COUNTDOWN_W) / 2;
    let py = (term_h - COUNTDOWN_H) / 2;

    let bg = Style::new().bg(Color::AnsiValue(234));
    backend.fill_region(px, py, COUNTDOWN_W, COUNTDOWN_H, ' ', &bg);

    let border = Style::new().fg(Color::Yellow).bg(Color::AnsiValue(234));
    render_box_border(backend, px, py, COUNTDOWN_W, COUNTDOWN_H, &border);

    let title = format!(" ROOM {} ", data.room_id);
    let title_style = Style::new()
        .fg(Color::Cyan)
        .bg(Color::AnsiValue(234))
        .bold();
    backend.write_str(px + 2, py, &title, &title_style);

    let text = if data.countdown_remaining == 0 {
        "GO!".to_owned()
    } else {
        data.countdown_remaining.to_string()
    };

    let number_style = Style::new()
        .fg(Color::Yellow)
        .bg(Color::AnsiValue(234))
        .bold();
    let cx = px + (COUNTDOWN_W - text.len() as u16) / 2;
    let cy = py + COUNTDOWN_H / 2;
    backend.write_str(cx, cy, &text, &number_style);

    let hint_style = Style::new().fg(Color::Grey).bg(Color::AnsiValue(234));
    let hint = "Get ready...";
    let hx = px + (COUNTDOWN_W - hint.len() as u16) / 2;
    backend.write_str(hx, cy + 2, hint, &hint_style);
}

/// How many opponent minimaps can fit vertically in the given height.
fn opponents_that_fit(available_h: u16, count: usize) -> usize {
    if count == 0 || available_h < MINI_H + 1 {
        return 0;
    }
    // First opponent: label (1) + minimap (MINI_H)
    // Each additional: gap (1) + label (1) + minimap (MINI_H)
    let first_cost = MINI_H + 1; // label + minimap
    let extra_cost = MINI_H + 2; // gap + label + minimap
    if available_h < first_cost {
        return 0;
    }
    let remaining = available_h - first_cost;
    #[allow(clippy::cast_possible_truncation)]
    let extras = (remaining / extra_cost) as usize;
    (1 + extras).min(count)
}

/// Render the game screen (board + side panel + optional opponents).
#[allow(clippy::cast_possible_truncation)]
fn render_game(backend: &mut dyn RenderBackend, data: &TetrominoData) {
    let (term_w, term_h) = backend.size();
    if term_w < POPUP_W || term_h < POPUP_H {
        let msg = "Terminal too small for Polyblocks";
        let x = term_w.saturating_sub(msg.len() as u16) / 2;
        let y = term_h / 2;
        backend.write_str(x, y, msg, &Style::new().fg(Color::Red));
        return;
    }

    // Calculate how many opponent minimaps can fit
    let has_opponents = !data.opponents.is_empty();
    let minimap_gap: u16 = 1; // gap between popup and minimaps
    let needed_for_mini = MINI_W + minimap_gap;

    // Check if we can show minimaps to the right
    let show_minimaps = has_opponents && term_w >= POPUP_W + needed_for_mini;
    let visible_opponents = if show_minimaps {
        opponents_that_fit(term_h.min(POPUP_H), data.opponents.len())
    } else {
        0
    };
    let show_minimaps = visible_opponents > 0;

    // Position popup: shift left when showing minimaps so both fit well
    let total_w = if show_minimaps {
        POPUP_W + minimap_gap + MINI_W
    } else {
        POPUP_W
    };
    let px = (term_w.saturating_sub(total_w)) / 2;
    let py = (term_h - POPUP_H) / 2;

    // Clear popup area
    let bg_style = Style::new().bg(Color::AnsiValue(234));
    backend.fill_region(px, py, POPUP_W, POPUP_H, ' ', &bg_style);

    // Outer border
    let border_style = Style::new().fg(Color::DarkCyan).bg(Color::AnsiValue(234));
    render_box_border(backend, px, py, POPUP_W, POPUP_H, &border_style);

    // Title
    let title = " POLYBLOCKS ";
    let title_style = Style::new()
        .fg(Color::Cyan)
        .bg(Color::AnsiValue(234))
        .bold();
    backend.write_str(px + 2, py, title, &title_style);

    // Board area (inside popup, offset by 2 from popup left edge)
    let board_x = px + 2;
    let board_y = py + 2;
    #[allow(clippy::cast_possible_truncation)]
    let board_w = BOARD_WIDTH as u16 * CELL_WIDTH + 2; // +2 for board border
    #[allow(clippy::cast_possible_truncation)]
    let board_h = BOARD_HEIGHT as u16 + 2; // +2 for board border

    // Board border
    let board_border_style = Style::new().fg(Color::Grey).bg(Color::AnsiValue(234));
    render_box_border(backend, board_x, board_y, board_w, board_h, &board_border_style);

    // Render board cells
    render_board(backend, data, board_x + 1, board_y + 1);

    // Side panel (to the right of the board)
    let panel_x = board_x + board_w + 1;
    let panel_y = board_y;
    render_side_panel(backend, data, panel_x, panel_y);

    // Overlays
    if data.paused {
        render_overlay(backend, "PAUSED", px, py);
    } else if data.game_over {
        render_overlay(backend, "GAME OVER", px, py);
    }

    // Opponent minimaps (to the right of the popup, only if they fit)
    if show_minimaps {
        let opp_x = px + POPUP_W + minimap_gap;
        let opp_y = py;
        render_opponents(backend, &data.opponents[..visible_opponents], opp_x, opp_y);
    }
}

/// Render the board grid with pieces.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn render_board(backend: &mut dyn RenderBackend, data: &TetrominoData, x: u16, y: u16) {
    let bg = Color::AnsiValue(234);

    // Draw locked blocks from board data
    for (row_idx, row) in data.board.iter().enumerate() {
        for (col_idx, cell_color) in row.iter().enumerate() {
            let cx = x + col_idx as u16 * CELL_WIDTH;
            let cy = y + row_idx as u16;
            if cell_color.is_empty() {
                // Empty cell — subtle grid dot
                let dot_style = Style::new().fg(Color::AnsiValue(236)).bg(bg);
                backend.set_cell(cx, cy, '\u{00B7}', &dot_style);
                backend.set_cell(cx + 1, cy, ' ', &Style::new().bg(bg));
            } else {
                let color = color_for_name(cell_color);
                let block_style = Style::new().fg(color).bg(color);
                backend.set_cell(cx, cy, '\u{2588}', &block_style);
                backend.set_cell(cx + 1, cy, '\u{2588}', &block_style);
            }
        }
    }

    // Draw ghost piece (translucent preview of landing position)
    if let Some(ref ghost) = data.ghost_piece {
        let color = color_for_name(&ghost.color);
        let ghost_style = Style::new().fg(color).bg(bg);
        for &(row, col) in &ghost.cells {
            if row >= 0 && col >= 0 {
                let cx = x + col as u16 * CELL_WIDTH;
                let cy = y + row as u16;
                backend.set_cell(cx, cy, '\u{2592}', &ghost_style); // medium shade
                backend.set_cell(cx + 1, cy, '\u{2592}', &ghost_style);
            }
        }
    }

    // Draw active piece on top
    if let Some(ref piece) = data.active_piece {
        let color = color_for_name(&piece.color);
        let piece_style = Style::new().fg(color).bg(color);
        for &(row, col) in &piece.cells {
            if row >= 0 && col >= 0 {
                let cx = x + col as u16 * CELL_WIDTH;
                let cy = y + row as u16;
                backend.set_cell(cx, cy, '\u{2588}', &piece_style);
                backend.set_cell(cx + 1, cy, '\u{2588}', &piece_style);
            }
        }
    }
}

/// Render the side info panel (next piece, score, level, lines).
#[allow(clippy::cast_possible_truncation)]
fn render_side_panel(backend: &mut dyn RenderBackend, data: &TetrominoData, x: u16, y: u16) {
    let label_style = Style::new().fg(Color::Grey).bg(Color::AnsiValue(234));
    let value_style = Style::new()
        .fg(Color::White)
        .bg(Color::AnsiValue(234))
        .bold();

    // Next piece label
    backend.write_str(x, y, "Next", &label_style);

    // Render next piece preview (small 4x2 area)
    let next_color = color_for_name(&data.next_piece_color);
    let preview_style = Style::new().fg(next_color).bg(next_color);
    let preview_cells = next_piece_preview_cells(&data.next_piece);
    for (dr, dc) in preview_cells {
        let cx = x + dc as u16 * CELL_WIDTH;
        let cy = y + 1 + dr as u16;
        backend.set_cell(cx, cy, '\u{2588}', &preview_style);
        backend.set_cell(cx + 1, cy, '\u{2588}', &preview_style);
    }

    // Held piece
    if let Some(ref held) = data.held_piece {
        let held_y = y + 4;
        backend.write_str(x, held_y, "Hold", &label_style);
        let held_color = color_for_name(&held.color);
        let held_style = Style::new().fg(held_color).bg(held_color);
        let held_cells = next_piece_preview_cells(&held.piece_type);
        for (dr, dc) in held_cells {
            let cx = x + dc as u16 * CELL_WIDTH;
            let cy = held_y + 1 + dr as u16;
            backend.set_cell(cx, cy, '\u{2588}', &held_style);
            backend.set_cell(cx + 1, cy, '\u{2588}', &held_style);
        }
    }

    // Score
    let score_y = y + 8;
    backend.write_str(x, score_y, "Score", &label_style);
    let score_str = data.score.to_string();
    backend.write_str(x, score_y + 1, &score_str, &value_style);

    // Level
    let level_y = score_y + 3;
    backend.write_str(x, level_y, "Level", &label_style);
    let level_str = data.level.to_string();
    backend.write_str(x, level_y + 1, &level_str, &value_style);

    // Lines
    let lines_y = level_y + 3;
    backend.write_str(x, lines_y, "Lines", &label_style);
    let lines_str = data.lines_cleared.to_string();
    backend.write_str(x, lines_y + 1, &lines_str, &value_style);
}

/// Return preview cell positions for the next piece (relative offsets).
fn next_piece_preview_cells(piece_name: &str) -> Vec<(usize, usize)> {
    match piece_name {
        "Pip" => vec![(0, 0)],
        "Dash" => vec![(0, 0), (0, 1)],
        "Line" => vec![(0, 0), (0, 1), (0, 2)],
        "Arc" => vec![(0, 0), (0, 1), (1, 0)],
        "Bar" => vec![(0, 0), (0, 1), (0, 2), (0, 3)],
        "Tee" => vec![(0, 0), (0, 1), (0, 2), (1, 1)],
        "Skew" => vec![(0, 1), (0, 2), (1, 0), (1, 1)],
        "Zag" => vec![(0, 0), (0, 1), (1, 1), (1, 2)],
        "Bend" => vec![(0, 0), (1, 0), (1, 1), (1, 2)],
        "Star" => vec![(0, 1), (1, 0), (1, 1), (1, 2), (2, 1)],
        _ => vec![],
    }
}

/// Render a centered text overlay (for PAUSED / GAME OVER).
#[allow(clippy::cast_possible_truncation)]
fn render_overlay(backend: &mut dyn RenderBackend, text: &str, popup_x: u16, popup_y: u16) {
    let overlay_w = text.len() as u16 + 4;
    let overlay_h: u16 = 3;
    let ox = popup_x + (POPUP_W - overlay_w) / 2;
    let oy = popup_y + (POPUP_H - overlay_h) / 2;

    let bg_style = Style::new().bg(Color::AnsiValue(232));
    backend.fill_region(ox, oy, overlay_w, overlay_h, ' ', &bg_style);

    let border_style = Style::new().fg(Color::Yellow).bg(Color::AnsiValue(232));
    render_box_border(backend, ox, oy, overlay_w, overlay_h, &border_style);

    let text_style = Style::new()
        .fg(Color::Yellow)
        .bg(Color::AnsiValue(232))
        .bold();
    backend.write_str(ox + 2, oy + 1, text, &text_style);
}

/// Render the result screen after a multiplayer match.
#[allow(clippy::cast_possible_truncation)]
fn render_result(backend: &mut dyn RenderBackend, data: &TetrominoData) {
    let player_rows = data.result_players.len().max(1);
    #[allow(clippy::cast_possible_truncation)]
    let result_h = RESULT_BASE_H + player_rows as u16;

    let (term_w, term_h) = backend.size();
    if term_w < RESULT_W || term_h < result_h {
        let msg = "Terminal too small";
        let x = term_w.saturating_sub(msg.len() as u16) / 2;
        backend.write_str(x, term_h / 2, msg, &Style::new().fg(Color::Red));
        return;
    }

    let px = (term_w - RESULT_W) / 2;
    let py = (term_h - result_h) / 2;

    let bg = Style::new().bg(Color::AnsiValue(234));
    backend.fill_region(px, py, RESULT_W, result_h, ' ', &bg);

    let border = Style::new().fg(Color::DarkCyan).bg(Color::AnsiValue(234));
    render_box_border(backend, px, py, RESULT_W, result_h, &border);

    let title_style = Style::new()
        .fg(Color::Cyan)
        .bg(Color::AnsiValue(234))
        .bold();
    backend.write_str(px + 2, py, " MATCH RESULT ", &title_style);

    let label = Style::new().fg(Color::White).bg(Color::AnsiValue(234));
    let winner_style = Style::new()
        .fg(Color::Yellow)
        .bg(Color::AnsiValue(234))
        .bold();
    let key_style = Style::new()
        .fg(Color::Yellow)
        .bg(Color::AnsiValue(234))
        .bold();

    let cx = px + 3;
    let mut cy = py + 2;

    // Winner announcement
    if let Some(winner_id) = data.winner_id {
        let msg = format!("Player {winner_id} WINS!");
        let wx = px + (RESULT_W - msg.len() as u16) / 2;
        backend.write_str(wx, cy, &msg, &winner_style);
    } else {
        let msg = "No winner";
        let wx = px + (RESULT_W - msg.len() as u16) / 2;
        backend.write_str(wx, cy, msg, &label);
    }
    cy += 2;

    // Player scores
    let mut sorted: Vec<&ResultPlayerData> = data.result_players.iter().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.score));
    for player in sorted {
        let id = player.id;
        let score = player.score;
        let line = format!("Player {id}: {score} pts");
        backend.write_str(cx, cy, &line, &label);
        cy += 1;
    }

    cy += 1;
    backend.write_str(cx, cy, "[q] Return to Lobby", &key_style);
}

/// Render opponent minimaps to the right of the main board.
#[allow(clippy::cast_possible_truncation)]
fn render_opponents(
    backend: &mut dyn RenderBackend,
    opponents: &[OpponentData],
    start_x: u16,
    start_y: u16,
) {
    let label_style = Style::new().fg(Color::Grey).bg(Color::AnsiValue(234));
    let bg = Color::AnsiValue(234);
    let border_style = Style::new().fg(Color::Grey).bg(Color::AnsiValue(234));

    let mut oy = start_y;
    for opp in opponents {
        // Label
        let label = format!("P{}", opp.client_id);
        let score_label = format!(" {}", opp.score);
        backend.write_str(start_x, oy, &label, &label_style);
        backend.write_str(start_x + label.len() as u16, oy, &score_label, &label_style);
        oy += 1;

        // Border
        render_box_border(backend, start_x, oy, MINI_W, MINI_H, &border_style);

        // Minimap: 1 char per cell, half-height (every 2 rows merged)
        let inner_x = start_x + 1;
        let inner_y = oy + 1;
        for (row_idx, row) in opp.board.iter().enumerate().step_by(2) {
            for (col_idx, cell_color) in row.iter().enumerate() {
                let cx = inner_x + col_idx as u16;
                let cy = inner_y + (row_idx / 2) as u16;
                if cell_color.is_empty() {
                    backend.set_cell(cx, cy, ' ', &Style::new().bg(bg));
                } else {
                    let color = color_for_name(cell_color);
                    backend.set_cell(cx, cy, '\u{2588}', &Style::new().fg(color).bg(bg));
                }
            }
        }

        if opp.game_over {
            let go_style = Style::new().fg(Color::Red).bg(Color::AnsiValue(234)).bold();
            backend.write_str(inner_x, inner_y + MINI_H / 2 - 1, "GAME", &go_style);
            backend.write_str(inner_x, inner_y + MINI_H / 2, "OVER", &go_style);
        }

        oy += MINI_H + 1;
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
