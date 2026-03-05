//! Tetromino board rendering.
//!
//! Renders the game board as a centered popup with score panel.
//!
//! ```text
//! ╭─ TETROMINO ──────────────────╮
//! │  ┌────────────────────┐     │
//! │  │                    │ Next│
//! │  │     ████           │ ██  │
//! │  │      ██            │     │
//! │  │                    │Score│
//! │  │                    │ 100 │
//! │  │                    │Level│
//! │  │                    │  1  │
//! │  │                    │Lines│
//! │  │                    │  5  │
//! │  │  ██████████████    │     │
//! │  │  ████████████████  │     │
//! │  └────────────────────┘     │
//! ╰─────────────────────────────╯
//! ```

use {
    reovim_arch::Color,
    reovim_driver_display::{Style, popup_utils::render_box_border, render_backend::RenderBackend},
};

use crate::{BOARD_HEIGHT, BOARD_WIDTH, TetrominoData};

/// Each board cell is 2 characters wide for a square look.
const CELL_WIDTH: u16 = 2;

// Board dimensions are small constants (10, 20) — truncation is impossible.
/// Popup width: board (2 chars * 10 cols) + board borders (2) + side panel (10) + popup padding (4)
#[allow(clippy::cast_possible_truncation)]
const POPUP_INNER_W: u16 = BOARD_WIDTH as u16 * CELL_WIDTH + 2 + 10 + 2;
const POPUP_W: u16 = POPUP_INNER_W + 2; // +2 for outer border
/// Popup height: title (1) + board (20) + board borders (2) + padding (1) + outer border (2)
/// Extra 4 rows for held piece display in side panel.
#[allow(clippy::cast_possible_truncation)]
const POPUP_H: u16 = BOARD_HEIGHT as u16 + 6;

/// Map a color name to a terminal Color.
fn color_for_name(name: &str) -> Color {
    match name {
        "cyan" => Color::Cyan,
        "yellow" => Color::Yellow,
        "purple" | "magenta" => Color::Magenta,
        "green" => Color::Green,
        "red" => Color::Red,
        "blue" => Color::Blue,
        "orange" => Color::DarkYellow,
        _ => Color::DarkGrey,
    }
}

/// Render the tetromino game UI.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn render_tetromino(backend: &mut dyn RenderBackend, data: &TetrominoData) {
    if !data.active {
        return;
    }

    let (term_w, term_h) = backend.size();
    if term_w < POPUP_W || term_h < POPUP_H {
        // Terminal too small — render a minimal message
        let msg = "Terminal too small for Tetromino";
        let x = term_w.saturating_sub(msg.len() as u16) / 2;
        let y = term_h / 2;
        backend.write_str(x, y, msg, &Style::new().fg(Color::Red));
        return;
    }

    // Center the popup
    let px = (term_w - POPUP_W) / 2;
    let py = (term_h - POPUP_H) / 2;

    // Clear popup area
    let bg_style = Style::new().bg(Color::AnsiValue(234));
    backend.fill_region(px, py, POPUP_W, POPUP_H, ' ', &bg_style);

    // Outer border
    let border_style = Style::new().fg(Color::DarkCyan).bg(Color::AnsiValue(234));
    render_box_border(backend, px, py, POPUP_W, POPUP_H, &border_style);

    // Title
    let title = " TETROMINO ";
    let title_style = Style::new()
        .fg(Color::Cyan)
        .bg(Color::AnsiValue(234))
        .bold();
    backend.write_str(px + 2, py, title, &title_style);

    // Board area (inside popup, offset by 2 from popup left edge)
    let board_x = px + 2;
    let board_y = py + 2;
    let board_w = BOARD_WIDTH as u16 * CELL_WIDTH + 2; // +2 for board border
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
                backend.set_cell(cx, cy, '\u{2592}', &ghost_style); // ▒ medium shade
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
        "I" => vec![(0, 0), (0, 1), (0, 2), (0, 3)],
        "O" => vec![(0, 0), (0, 1), (1, 0), (1, 1)],
        "T" => vec![(0, 0), (0, 1), (0, 2), (1, 1)],
        "S" => vec![(0, 1), (0, 2), (1, 0), (1, 1)],
        "Z" => vec![(0, 0), (0, 1), (1, 1), (1, 2)],
        "J" => vec![(0, 0), (1, 0), (1, 1), (1, 2)],
        "L" => vec![(0, 2), (1, 0), (1, 1), (1, 2)],
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

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_display::FrameBuffer};

    use crate::{HeldPieceData, PieceData};

    fn make_active_data() -> TetrominoData {
        let mut board = Vec::with_capacity(BOARD_HEIGHT);
        for _ in 0..BOARD_HEIGHT {
            board.push(vec![String::new(); BOARD_WIDTH]);
        }
        // Add some locked blocks at the bottom
        board[19][0] = "cyan".to_owned();
        board[19][1] = "red".to_owned();
        board[19][2] = "blue".to_owned();

        TetrominoData {
            active: true,
            paused: false,
            game_over: false,
            score: 1234,
            level: 3,
            lines_cleared: 15,
            next_piece: "T".to_owned(),
            next_piece_color: "purple".to_owned(),
            board,
            active_piece: Some(PieceData {
                color: "cyan".to_owned(),
                cells: vec![(2, 3), (2, 4), (2, 5), (2, 6)],
            }),
            ghost_piece: Some(PieceData {
                color: "cyan".to_owned(),
                cells: vec![(18, 3), (18, 4), (18, 5), (18, 6)],
            }),
            held_piece: None,
        }
    }

    #[test]
    fn render_inactive_noop() {
        let mut fb = FrameBuffer::new(80, 30);
        let data = TetrominoData::default();
        render_tetromino(&mut fb, &data);
        // All cells should still be space
        assert_eq!(fb.get(0, 0).map(|c| c.char), Some(' '));
    }

    #[test]
    fn render_active_game() {
        let mut fb = FrameBuffer::new(80, 30);
        let data = make_active_data();
        render_tetromino(&mut fb, &data);
        // Should have rendered without panic — just verify the border exists
        // The popup is centered, check some non-space character exists
    }

    #[test]
    fn render_paused() {
        let mut fb = FrameBuffer::new(80, 30);
        let mut data = make_active_data();
        data.paused = true;
        render_tetromino(&mut fb, &data);
    }

    #[test]
    fn render_game_over() {
        let mut fb = FrameBuffer::new(80, 30);
        let mut data = make_active_data();
        data.game_over = true;
        data.active_piece = None;
        render_tetromino(&mut fb, &data);
    }

    #[test]
    fn render_too_small_terminal() {
        let mut fb = FrameBuffer::new(20, 10);
        let data = make_active_data();
        render_tetromino(&mut fb, &data);
    }

    #[test]
    fn color_mapping() {
        assert!(matches!(color_for_name("cyan"), Color::Cyan));
        assert!(matches!(color_for_name("yellow"), Color::Yellow));
        assert!(matches!(color_for_name("purple"), Color::Magenta));
        assert!(matches!(color_for_name("magenta"), Color::Magenta));
        assert!(matches!(color_for_name("green"), Color::Green));
        assert!(matches!(color_for_name("red"), Color::Red));
        assert!(matches!(color_for_name("blue"), Color::Blue));
        assert!(matches!(color_for_name("orange"), Color::DarkYellow));
        assert!(matches!(color_for_name("unknown"), Color::DarkGrey));
        assert!(matches!(color_for_name(""), Color::DarkGrey));
    }

    #[test]
    fn next_piece_preview_all_types() {
        assert_eq!(next_piece_preview_cells("I").len(), 4);
        assert_eq!(next_piece_preview_cells("O").len(), 4);
        assert_eq!(next_piece_preview_cells("T").len(), 4);
        assert_eq!(next_piece_preview_cells("S").len(), 4);
        assert_eq!(next_piece_preview_cells("Z").len(), 4);
        assert_eq!(next_piece_preview_cells("J").len(), 4);
        assert_eq!(next_piece_preview_cells("L").len(), 4);
        assert!(next_piece_preview_cells("?").is_empty());
    }

    #[test]
    fn render_board_with_all_colors() {
        let mut fb = FrameBuffer::new(80, 30);
        let mut data = make_active_data();
        data.board[18][0] = "yellow".to_owned();
        data.board[18][1] = "purple".to_owned();
        data.board[18][2] = "green".to_owned();
        data.board[18][3] = "orange".to_owned();
        render_tetromino(&mut fb, &data);
    }

    #[test]
    fn render_piece_at_negative_coords() {
        let mut fb = FrameBuffer::new(80, 30);
        let mut data = make_active_data();
        data.active_piece = Some(PieceData {
            color: "cyan".to_owned(),
            cells: vec![(-1, 3), (-1, 4), (0, 3), (0, 4)],
        });
        render_tetromino(&mut fb, &data);
    }

    #[test]
    fn render_overlay_centered() {
        let mut fb = FrameBuffer::new(80, 30);
        render_overlay(&mut fb, "TEST", 10, 5);
    }

    #[test]
    fn render_side_panel_display() {
        let mut fb = FrameBuffer::new(80, 30);
        let data = make_active_data();
        render_side_panel(&mut fb, &data, 40, 5);
    }

    #[test]
    fn render_board_empty() {
        let mut fb = FrameBuffer::new(80, 30);
        let data = TetrominoData {
            active: true,
            board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
            ..TetrominoData::default()
        };
        render_board(&mut fb, &data, 5, 5);
    }

    #[test]
    fn render_with_held_piece() {
        let mut fb = FrameBuffer::new(80, 30);
        let mut data = make_active_data();
        data.held_piece = Some(HeldPieceData {
            piece_type: "T".to_owned(),
            color: "purple".to_owned(),
        });
        render_tetromino(&mut fb, &data);
    }

    #[test]
    fn render_ghost_piece() {
        let mut fb = FrameBuffer::new(80, 30);
        let data = make_active_data();
        // Ghost piece is already set in make_active_data
        render_tetromino(&mut fb, &data);
    }

    #[test]
    fn held_piece_data_debug() {
        let held = HeldPieceData {
            piece_type: "I".to_owned(),
            color: "cyan".to_owned(),
        };
        let debug = format!("{held:?}");
        assert!(debug.contains("cyan"));
    }
}
