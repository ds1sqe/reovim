//! Pure tetromino game logic with zero framework dependencies.
//!
//! All functions are pure and testable. The board is 8 columns x 16 rows,
//! with row 0 at the top. Pieces use the standard 7 tetrominoes with
//! nudge-based wall kicks.

/// Board width in cells.
pub const BOARD_WIDTH: usize = 8;

/// Board height in cells.
pub const BOARD_HEIGHT: usize = 16;

/// Cell color corresponding to each piece type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockColor {
    White,  // Pip
    Amber,  // Dash
    Teal,   // Line
    Sky,    // Arc
    Lime,   // Bar
    Rose,   // Tee
    Violet, // Skew
    Coral,  // Zag
    Blue,   // Bend
    Gold,   // Star
    Grey,   // Garbage blocks
}

/// The 10 polyblocks piece types (mixed sizes, 1-5 cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PieceType {
    Pip,  // 1 cell
    Dash, // 2 cells
    Line, // 3 cells
    Arc,  // 3 cells
    Bar,  // 4 cells (horizontal/vertical)
    Tee,  // 4 cells (T-shape)
    Skew, // 4 cells (S-shape)
    Zag,  // 4 cells (Z-shape)
    Bend, // 4 cells (J-shape)
    Star, // 5 cells (plus shape)
}

impl PieceType {
    /// All piece types for iteration.
    pub const ALL: [Self; 10] = [
        Self::Pip,
        Self::Dash,
        Self::Line,
        Self::Arc,
        Self::Bar,
        Self::Tee,
        Self::Skew,
        Self::Zag,
        Self::Bend,
        Self::Star,
    ];

    /// Get the block color for this piece type.
    #[must_use]
    pub const fn color(self) -> BlockColor {
        match self {
            Self::Pip => BlockColor::White,
            Self::Dash => BlockColor::Amber,
            Self::Line => BlockColor::Teal,
            Self::Arc => BlockColor::Sky,
            Self::Bar => BlockColor::Lime,
            Self::Tee => BlockColor::Rose,
            Self::Skew => BlockColor::Violet,
            Self::Zag => BlockColor::Coral,
            Self::Bend => BlockColor::Blue,
            Self::Star => BlockColor::Gold,
        }
    }

    /// Get the color name as a string (for JSON serialization).
    #[must_use]
    pub const fn color_name(self) -> &'static str {
        match self {
            Self::Pip => "white",
            Self::Dash => "amber",
            Self::Line => "teal",
            Self::Arc => "sky",
            Self::Bar => "lime",
            Self::Tee => "rose",
            Self::Skew => "violet",
            Self::Zag => "coral",
            Self::Bend => "blue",
            Self::Star => "gold",
        }
    }

    /// Get the piece type name as a string.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pip => "Pip",
            Self::Dash => "Dash",
            Self::Line => "Line",
            Self::Arc => "Arc",
            Self::Bar => "Bar",
            Self::Tee => "Tee",
            Self::Skew => "Skew",
            Self::Zag => "Zag",
            Self::Bend => "Bend",
            Self::Star => "Star",
        }
    }
}

/// Rotation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    R0,
    R90,
    R180,
    R270,
}

impl Rotation {
    /// Rotate clockwise.
    #[must_use]
    pub const fn cw(self) -> Self {
        match self {
            Self::R0 => Self::R90,
            Self::R90 => Self::R180,
            Self::R180 => Self::R270,
            Self::R270 => Self::R0,
        }
    }

    /// Rotate counter-clockwise.
    #[must_use]
    pub const fn ccw(self) -> Self {
        match self {
            Self::R0 => Self::R270,
            Self::R90 => Self::R0,
            Self::R180 => Self::R90,
            Self::R270 => Self::R180,
        }
    }
}

/// A single cell on the board.
pub type Cell = Option<BlockColor>;

/// Board type alias.
pub type Board = [[Cell; BOARD_WIDTH]; BOARD_HEIGHT];

/// A piece with position and rotation on the board.
#[derive(Debug, Clone)]
pub struct ActivePiece {
    pub piece_type: PieceType,
    pub rotation: Rotation,
    /// Row of the piece origin (can be negative during spawn).
    pub row: i32,
    /// Column of the piece origin.
    pub col: i32,
}

/// Core game state (no framework dependencies).
#[derive(Debug, Clone)]
pub struct GameState {
    pub board: Board,
    pub active_piece: Option<ActivePiece>,
    pub next_piece: PieceType,
    pub held_piece: Option<PieceType>,
    /// Whether hold has been used for the current piece (reset on lock).
    pub hold_used: bool,
    pub score: u32,
    pub level: u32,
    pub lines_cleared: u32,
    pub game_over: bool,
}

/// Get the cell offsets for a piece at a given rotation.
///
/// Returns `(row, col)` offsets relative to the piece's origin.
/// Slice length varies by piece type (1-5 cells).
#[must_use]
pub const fn piece_cells(piece: PieceType, rotation: Rotation) -> &'static [(i32, i32)] {
    match piece {
        PieceType::Pip => &[(0, 0)],
        PieceType::Dash => match rotation {
            Rotation::R0 | Rotation::R180 => &[(0, 0), (0, 1)],
            Rotation::R90 | Rotation::R270 => &[(0, 0), (1, 0)],
        },
        PieceType::Line => match rotation {
            Rotation::R0 | Rotation::R180 => &[(0, 0), (0, 1), (0, 2)],
            Rotation::R90 | Rotation::R270 => &[(0, 0), (1, 0), (2, 0)],
        },
        PieceType::Arc => match rotation {
            Rotation::R0 => &[(0, 0), (0, 1), (1, 0)],
            Rotation::R90 => &[(0, 0), (0, 1), (1, 1)],
            Rotation::R180 => &[(0, 1), (1, 0), (1, 1)],
            Rotation::R270 => &[(0, 0), (1, 0), (1, 1)],
        },
        PieceType::Bar => match rotation {
            Rotation::R0 | Rotation::R180 => &[(0, 0), (0, 1), (0, 2), (0, 3)],
            Rotation::R90 | Rotation::R270 => &[(0, 0), (1, 0), (2, 0), (3, 0)],
        },
        PieceType::Tee => match rotation {
            Rotation::R0 => &[(0, 0), (0, 1), (0, 2), (1, 1)],
            Rotation::R90 => &[(0, 0), (1, 0), (2, 0), (1, 1)],
            Rotation::R180 => &[(0, 1), (1, 0), (1, 1), (1, 2)],
            Rotation::R270 => &[(0, 1), (1, 0), (1, 1), (2, 1)],
        },
        PieceType::Skew => match rotation {
            Rotation::R0 | Rotation::R180 => &[(0, 1), (0, 2), (1, 0), (1, 1)],
            Rotation::R90 | Rotation::R270 => &[(0, 0), (1, 0), (1, 1), (2, 1)],
        },
        PieceType::Zag => match rotation {
            Rotation::R0 | Rotation::R180 => &[(0, 0), (0, 1), (1, 1), (1, 2)],
            Rotation::R90 | Rotation::R270 => &[(0, 1), (1, 0), (1, 1), (2, 0)],
        },
        PieceType::Bend => match rotation {
            Rotation::R0 => &[(0, 0), (1, 0), (1, 1), (1, 2)],
            Rotation::R90 => &[(0, 0), (0, 1), (1, 0), (2, 0)],
            Rotation::R180 => &[(0, 0), (0, 1), (0, 2), (1, 2)],
            Rotation::R270 => &[(0, 1), (1, 1), (2, 0), (2, 1)],
        },
        PieceType::Star => &[(0, 1), (1, 0), (1, 1), (1, 2), (2, 1)],
    }
}

/// Check if a piece at a given position collides with the board or walls.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
pub fn collides(board: &Board, piece: &ActivePiece) -> bool {
    for &(dr, dc) in piece_cells(piece.piece_type, piece.rotation) {
        let r = piece.row + dr;
        let c = piece.col + dc;
        if c < 0 || c >= BOARD_WIDTH as i32 || r >= BOARD_HEIGHT as i32 {
            return true;
        }
        // Rows above the board are allowed (spawn zone)
        if r >= 0 && board[r as usize][c as usize].is_some() {
            return true;
        }
    }
    false
}

/// Lock the active piece into the board.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
pub fn lock_piece(board: &mut Board, piece: &ActivePiece) {
    let color = piece.piece_type.color();
    for &(dr, dc) in piece_cells(piece.piece_type, piece.rotation) {
        let r = piece.row + dr;
        let c = piece.col + dc;
        if r >= 0 && r < BOARD_HEIGHT as i32 && c >= 0 && c < BOARD_WIDTH as i32 {
            board[r as usize][c as usize] = Some(color);
        }
    }
}

/// Clear completed lines, returning the number cleared.
pub fn clear_lines(board: &mut Board) -> u32 {
    let mut cleared = 0u32;
    let mut write = BOARD_HEIGHT;

    // Scan from bottom to top, keeping non-full rows
    for read in (0..BOARD_HEIGHT).rev() {
        let full = board[read].iter().all(Option::is_some);
        if full {
            cleared += 1;
        } else {
            write -= 1;
            if write != read {
                board[write] = board[read];
            }
        }
    }

    // Fill the top rows with empty cells
    for row in board.iter_mut().take(write) {
        *row = [None; BOARD_WIDTH];
    }

    cleared
}

/// Calculate score for cleared lines at given level.
///
/// Formula: `lines^2 * 50 * (level + 1)`
/// - 1 line:  50 * (level+1)
/// - 2 lines: 200 * (level+1)
/// - 3 lines: 450 * (level+1)
/// - 4 lines: 800 * (level+1)
#[must_use]
pub const fn calculate_score(lines: u32, level: u32) -> u32 {
    lines * lines * 50 * (level + 1)
}

/// Calculate level from total lines cleared.
///
/// Levels 0-4 require 8 lines each. Levels 5+ require 12 lines each.
#[must_use]
pub const fn calculate_level(total_lines: u32) -> u32 {
    if total_lines < 40 {
        total_lines / 8
    } else {
        5 + (total_lines - 40) / 12
    }
}

/// Calculate tick interval in milliseconds from level.
///
/// Uses exponential decay: `900 * 0.85^level`, floored at 80ms.
/// Level 0: 900ms, Level 5: ~400ms, Level 10: ~177ms, Level 15: ~80ms.
#[must_use]
pub fn tick_interval_ms(level: u32) -> u64 {
    // 0.85 = 17/20, computed with integer math to avoid float
    let mut interval = 900u64;
    for _ in 0..level {
        interval = interval * 17 / 20;
    }
    interval.max(80)
}

/// Standard nudge offsets: in-place, left, right, up.
const NUDGES_STANDARD: &[(i32, i32)] = &[
    (0, 0),  // try in place
    (-1, 0), // nudge left
    (1, 0),  // nudge right
    (0, -1), // nudge up
];

/// Wide-piece (Bar) nudge offsets: standard + larger horizontal shifts.
const NUDGES_WIDE: &[(i32, i32)] = &[
    (0, 0),  // try in place
    (-1, 0), // nudge left
    (1, 0),  // nudge right
    (0, -1), // nudge up
    (-2, 0), // nudge left 2
    (2, 0),  // nudge right 2
];

/// Common nudge-rotation logic for CW and CCW rotation.
///
/// Tries the rotation with a sequence of small offsets (nudges).
/// Standard pieces try 4 offsets; the I-piece tries 6.
fn try_rotate_with_nudges(
    board: &Board,
    piece: &ActivePiece,
    to_rot: Rotation,
) -> Option<ActivePiece> {
    let offsets = if piece.piece_type == PieceType::Bar {
        NUDGES_WIDE
    } else {
        NUDGES_STANDARD
    };
    for &(dc, dr) in offsets {
        let candidate = ActivePiece {
            rotation: to_rot,
            col: piece.col + dc,
            row: piece.row + dr,
            ..piece.clone()
        };
        if !collides(board, &candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Try to rotate the piece clockwise with nudge wall kicks.
#[must_use]
pub fn try_rotate_cw(board: &Board, piece: &ActivePiece) -> Option<ActivePiece> {
    let to_rot = piece.rotation.cw();
    try_rotate_with_nudges(board, piece, to_rot)
}

/// Try to rotate the piece counter-clockwise with nudge wall kicks.
#[must_use]
pub fn try_rotate_ccw(board: &Board, piece: &ActivePiece) -> Option<ActivePiece> {
    let to_rot = piece.rotation.ccw();
    try_rotate_with_nudges(board, piece, to_rot)
}

/// Try to move the piece left. Returns the moved piece if valid.
#[must_use]
pub fn try_move_left(board: &Board, piece: &ActivePiece) -> Option<ActivePiece> {
    let moved = ActivePiece {
        col: piece.col - 1,
        ..piece.clone()
    };
    if collides(board, &moved) {
        None
    } else {
        Some(moved)
    }
}

/// Try to move the piece right. Returns the moved piece if valid.
#[must_use]
pub fn try_move_right(board: &Board, piece: &ActivePiece) -> Option<ActivePiece> {
    let moved = ActivePiece {
        col: piece.col + 1,
        ..piece.clone()
    };
    if collides(board, &moved) {
        None
    } else {
        Some(moved)
    }
}

/// Try to move the piece down one row. Returns the moved piece if valid.
#[must_use]
pub fn try_move_down(board: &Board, piece: &ActivePiece) -> Option<ActivePiece> {
    let moved = ActivePiece {
        row: piece.row + 1,
        ..piece.clone()
    };
    if collides(board, &moved) {
        None
    } else {
        Some(moved)
    }
}

/// Hard drop: move piece down until it collides.
#[must_use]
pub fn hard_drop(board: &Board, piece: &ActivePiece) -> ActivePiece {
    let mut dropped = piece.clone();
    loop {
        let next = ActivePiece {
            row: dropped.row + 1,
            ..dropped.clone()
        };
        if collides(board, &next) {
            return dropped;
        }
        dropped = next;
    }
}

/// R0 width of each piece type (for spawn centering).
const fn piece_r0_width(piece: PieceType) -> i32 {
    match piece {
        PieceType::Pip => 1,
        PieceType::Dash | PieceType::Arc => 2,
        PieceType::Line
        | PieceType::Tee
        | PieceType::Skew
        | PieceType::Zag
        | PieceType::Bend
        | PieceType::Star => 3,
        PieceType::Bar => 4,
    }
}

/// Spawn a new piece at the top center of the board.
// BOARD_WIDTH is a small constant (8) — truncation/wrap is impossible.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub const fn spawn_piece(piece_type: PieceType) -> ActivePiece {
    let width = piece_r0_width(piece_type);
    ActivePiece {
        piece_type,
        rotation: Rotation::R0,
        row: 0,
        col: (BOARD_WIDTH as i32 - width) / 2,
    }
}

/// Create a new game state with the first two pieces.
#[must_use]
pub const fn new_game(first_piece: PieceType, next_piece: PieceType) -> GameState {
    GameState {
        board: [[None; BOARD_WIDTH]; BOARD_HEIGHT],
        active_piece: Some(spawn_piece(first_piece)),
        next_piece,
        held_piece: None,
        hold_used: false,
        score: 0,
        level: 0,
        lines_cleared: 0,
        game_over: false,
    }
}

/// Compute the ghost piece position (where the active piece would land).
#[must_use]
pub fn ghost_piece(board: &Board, piece: &ActivePiece) -> ActivePiece {
    hard_drop(board, piece)
}

/// Lock the current piece, clear lines, spawn next. Shared by tick/soft-drop/hard-drop.
///
/// Returns the number of lines cleared.
pub fn lock_and_advance(
    state: &mut GameState,
    piece: &ActivePiece,
    next_next_piece: PieceType,
) -> u32 {
    lock_piece(&mut state.board, piece);
    let lines = clear_lines(&mut state.board);
    state.lines_cleared += lines;
    state.score += calculate_score(lines, state.level);
    state.level = calculate_level(state.lines_cleared);
    state.hold_used = false;

    // Spawn next piece
    let next = spawn_piece(state.next_piece);
    if collides(&state.board, &next) {
        state.game_over = true;
        state.active_piece = None;
    } else {
        state.active_piece = Some(next);
    }
    state.next_piece = next_next_piece;
    lines
}

/// Advance one tick: move active piece down; if collision, lock, clear, spawn.
///
/// Returns `(lines_cleared_this_tick, piece_was_locked)`.
pub fn tick(state: &mut GameState, next_next_piece: PieceType) -> (u32, bool) {
    if state.game_over {
        return (0, false);
    }

    let Some(piece) = state.active_piece.take() else {
        return (0, false);
    };

    if let Some(moved) = try_move_down(&state.board, &piece) {
        state.active_piece = Some(moved);
        return (0, false);
    }

    // Piece can't move down: lock it
    let lines = lock_and_advance(state, &piece, next_next_piece);
    (lines, true)
}

/// Get the absolute board positions of a piece's cells.
#[must_use]
pub fn absolute_cells(piece: &ActivePiece) -> Vec<(i32, i32)> {
    piece_cells(piece.piece_type, piece.rotation)
        .iter()
        .map(|(dr, dc)| (piece.row + dr, piece.col + dc))
        .collect()
}

/// Apply one garbage line: shift all rows up, insert garbage at bottom.
///
/// `gap_col` is the column index that remains empty. Clamped to valid range.
/// Pure function — caller provides the random column.
pub fn apply_garbage_line(game: &mut GameState, gap_col: usize) {
    // Shift all rows up by 1
    for row in 0..(BOARD_HEIGHT - 1) {
        game.board[row] = game.board[row + 1];
    }
    // Fill bottom row with grey blocks, one gap
    let mut garbage_row = [Some(BlockColor::Grey); BOARD_WIDTH];
    garbage_row[gap_col.min(BOARD_WIDTH - 1)] = None;
    game.board[BOARD_HEIGHT - 1] = garbage_row;
}

#[cfg(test)]
#[path = "game_tests.rs"]
mod tests;
