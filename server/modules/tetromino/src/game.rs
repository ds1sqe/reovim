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
mod tests {
    use super::*;

    // =========================================================================
    // PieceType
    // =========================================================================

    #[test]
    fn piece_type_all_has_ten() {
        assert_eq!(PieceType::ALL.len(), 10);
    }

    #[test]
    fn piece_type_colors() {
        assert_eq!(PieceType::Pip.color(), BlockColor::White);
        assert_eq!(PieceType::Dash.color(), BlockColor::Amber);
        assert_eq!(PieceType::Line.color(), BlockColor::Teal);
        assert_eq!(PieceType::Arc.color(), BlockColor::Sky);
        assert_eq!(PieceType::Bar.color(), BlockColor::Lime);
        assert_eq!(PieceType::Tee.color(), BlockColor::Rose);
        assert_eq!(PieceType::Skew.color(), BlockColor::Violet);
        assert_eq!(PieceType::Zag.color(), BlockColor::Coral);
        assert_eq!(PieceType::Bend.color(), BlockColor::Blue);
        assert_eq!(PieceType::Star.color(), BlockColor::Gold);
    }

    #[test]
    fn piece_type_color_names() {
        assert_eq!(PieceType::Pip.color_name(), "white");
        assert_eq!(PieceType::Dash.color_name(), "amber");
        assert_eq!(PieceType::Line.color_name(), "teal");
        assert_eq!(PieceType::Arc.color_name(), "sky");
        assert_eq!(PieceType::Bar.color_name(), "lime");
        assert_eq!(PieceType::Tee.color_name(), "rose");
        assert_eq!(PieceType::Skew.color_name(), "violet");
        assert_eq!(PieceType::Zag.color_name(), "coral");
        assert_eq!(PieceType::Bend.color_name(), "blue");
        assert_eq!(PieceType::Star.color_name(), "gold");
    }

    #[test]
    fn piece_type_names() {
        assert_eq!(PieceType::Pip.name(), "Pip");
        assert_eq!(PieceType::Dash.name(), "Dash");
        assert_eq!(PieceType::Line.name(), "Line");
        assert_eq!(PieceType::Arc.name(), "Arc");
        assert_eq!(PieceType::Bar.name(), "Bar");
        assert_eq!(PieceType::Tee.name(), "Tee");
        assert_eq!(PieceType::Skew.name(), "Skew");
        assert_eq!(PieceType::Zag.name(), "Zag");
        assert_eq!(PieceType::Bend.name(), "Bend");
        assert_eq!(PieceType::Star.name(), "Star");
    }

    #[test]
    fn piece_type_debug() {
        let debug = format!("{:?}", PieceType::Pip);
        assert!(debug.contains("Pip"));
    }

    #[test]
    fn piece_type_eq() {
        assert_eq!(PieceType::Bar, PieceType::Bar);
        assert_ne!(PieceType::Bar, PieceType::Tee);
    }

    #[test]
    fn piece_type_clone() {
        let p = PieceType::Tee;
        #[allow(clippy::clone_on_copy)]
        let cloned = p.clone();
        assert_eq!(p, cloned);
    }

    #[test]
    fn block_color_debug() {
        let debug = format!("{:?}", BlockColor::Amber);
        assert!(debug.contains("Amber"));
    }

    #[test]
    fn block_color_eq() {
        assert_eq!(BlockColor::Amber, BlockColor::Amber);
        assert_ne!(BlockColor::Amber, BlockColor::Lime);
    }

    #[test]
    fn block_color_clone() {
        let c = BlockColor::Rose;
        #[allow(clippy::clone_on_copy)]
        let cloned = c.clone();
        assert_eq!(c, cloned);
    }

    // =========================================================================
    // Rotation
    // =========================================================================

    #[test]
    fn rotation_cw_cycle() {
        assert_eq!(Rotation::R0.cw(), Rotation::R90);
        assert_eq!(Rotation::R90.cw(), Rotation::R180);
        assert_eq!(Rotation::R180.cw(), Rotation::R270);
        assert_eq!(Rotation::R270.cw(), Rotation::R0);
    }

    #[test]
    fn rotation_full_cycle() {
        let r = Rotation::R0;
        assert_eq!(r.cw().cw().cw().cw(), Rotation::R0);
    }

    #[test]
    fn rotation_debug() {
        let debug = format!("{:?}", Rotation::R90);
        assert!(debug.contains("R90"));
    }

    #[test]
    fn rotation_eq() {
        assert_eq!(Rotation::R0, Rotation::R0);
        assert_ne!(Rotation::R0, Rotation::R90);
    }

    #[test]
    fn rotation_clone() {
        let r = Rotation::R180;
        #[allow(clippy::clone_on_copy)]
        let cloned = r.clone();
        assert_eq!(r, cloned);
    }

    // =========================================================================
    // piece_cells — all rotations for all pieces
    // =========================================================================

    #[test]
    fn pip_piece_cells_symmetric() {
        assert_eq!(piece_cells(PieceType::Pip, Rotation::R0), [(0, 0)]);
        assert_eq!(piece_cells(PieceType::Pip, Rotation::R90), [(0, 0)]);
        assert_eq!(piece_cells(PieceType::Pip, Rotation::R180), [(0, 0)]);
        assert_eq!(piece_cells(PieceType::Pip, Rotation::R270), [(0, 0)]);
    }

    #[test]
    fn dash_piece_cells() {
        let h = piece_cells(PieceType::Dash, Rotation::R0);
        assert_eq!(h, [(0, 0), (0, 1)]);
        assert_eq!(piece_cells(PieceType::Dash, Rotation::R180), h);
        let v = piece_cells(PieceType::Dash, Rotation::R90);
        assert_eq!(v, [(0, 0), (1, 0)]);
        assert_eq!(piece_cells(PieceType::Dash, Rotation::R270), v);
    }

    #[test]
    fn line_piece_cells() {
        let h = piece_cells(PieceType::Line, Rotation::R0);
        assert_eq!(h, [(0, 0), (0, 1), (0, 2)]);
        assert_eq!(piece_cells(PieceType::Line, Rotation::R180), h);
        let v = piece_cells(PieceType::Line, Rotation::R90);
        assert_eq!(v, [(0, 0), (1, 0), (2, 0)]);
        assert_eq!(piece_cells(PieceType::Line, Rotation::R270), v);
    }

    #[test]
    fn arc_piece_cells_all_rotations() {
        assert_eq!(piece_cells(PieceType::Arc, Rotation::R0), [(0, 0), (0, 1), (1, 0)]);
        assert_eq!(piece_cells(PieceType::Arc, Rotation::R90), [(0, 0), (0, 1), (1, 1)]);
        assert_eq!(piece_cells(PieceType::Arc, Rotation::R180), [(0, 1), (1, 0), (1, 1)]);
        assert_eq!(piece_cells(PieceType::Arc, Rotation::R270), [(0, 0), (1, 0), (1, 1)]);
    }

    #[test]
    fn bar_piece_cells() {
        let h = piece_cells(PieceType::Bar, Rotation::R0);
        assert_eq!(h, [(0, 0), (0, 1), (0, 2), (0, 3)]);
        assert_eq!(piece_cells(PieceType::Bar, Rotation::R180), h);
        let v = piece_cells(PieceType::Bar, Rotation::R90);
        assert_eq!(v, [(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(piece_cells(PieceType::Bar, Rotation::R270), v);
    }

    #[test]
    fn tee_piece_cells_all_rotations() {
        assert_eq!(piece_cells(PieceType::Tee, Rotation::R0), [(0, 0), (0, 1), (0, 2), (1, 1)]);
        assert_eq!(piece_cells(PieceType::Tee, Rotation::R90), [(0, 0), (1, 0), (2, 0), (1, 1)]);
        assert_eq!(piece_cells(PieceType::Tee, Rotation::R180), [(0, 1), (1, 0), (1, 1), (1, 2)]);
        assert_eq!(piece_cells(PieceType::Tee, Rotation::R270), [(0, 1), (1, 0), (1, 1), (2, 1)]);
    }

    #[test]
    fn skew_piece_cells() {
        let h = piece_cells(PieceType::Skew, Rotation::R0);
        assert_eq!(h, [(0, 1), (0, 2), (1, 0), (1, 1)]);
        assert_eq!(piece_cells(PieceType::Skew, Rotation::R180), h);
        let v = piece_cells(PieceType::Skew, Rotation::R90);
        assert_eq!(v, [(0, 0), (1, 0), (1, 1), (2, 1)]);
        assert_eq!(piece_cells(PieceType::Skew, Rotation::R270), v);
    }

    #[test]
    fn zag_piece_cells() {
        let h = piece_cells(PieceType::Zag, Rotation::R0);
        assert_eq!(h, [(0, 0), (0, 1), (1, 1), (1, 2)]);
        assert_eq!(piece_cells(PieceType::Zag, Rotation::R180), h);
        let v = piece_cells(PieceType::Zag, Rotation::R90);
        assert_eq!(v, [(0, 1), (1, 0), (1, 1), (2, 0)]);
        assert_eq!(piece_cells(PieceType::Zag, Rotation::R270), v);
    }

    #[test]
    fn bend_piece_cells_all_rotations() {
        assert_eq!(piece_cells(PieceType::Bend, Rotation::R0), [(0, 0), (1, 0), (1, 1), (1, 2)]);
        assert_eq!(piece_cells(PieceType::Bend, Rotation::R90), [(0, 0), (0, 1), (1, 0), (2, 0)]);
        assert_eq!(piece_cells(PieceType::Bend, Rotation::R180), [(0, 0), (0, 1), (0, 2), (1, 2)]);
        assert_eq!(piece_cells(PieceType::Bend, Rotation::R270), [(0, 1), (1, 1), (2, 0), (2, 1)]);
    }

    #[test]
    fn star_piece_cells_symmetric() {
        let cells = piece_cells(PieceType::Star, Rotation::R0);
        assert_eq!(cells, [(0, 1), (1, 0), (1, 1), (1, 2), (2, 1)]);
        // Star is rotationally symmetric
        assert_eq!(piece_cells(PieceType::Star, Rotation::R90), cells);
        assert_eq!(piece_cells(PieceType::Star, Rotation::R180), cells);
        assert_eq!(piece_cells(PieceType::Star, Rotation::R270), cells);
    }

    #[test]
    fn piece_cell_counts() {
        assert_eq!(piece_cells(PieceType::Pip, Rotation::R0).len(), 1);
        assert_eq!(piece_cells(PieceType::Dash, Rotation::R0).len(), 2);
        assert_eq!(piece_cells(PieceType::Line, Rotation::R0).len(), 3);
        assert_eq!(piece_cells(PieceType::Arc, Rotation::R0).len(), 3);
        assert_eq!(piece_cells(PieceType::Bar, Rotation::R0).len(), 4);
        assert_eq!(piece_cells(PieceType::Tee, Rotation::R0).len(), 4);
        assert_eq!(piece_cells(PieceType::Skew, Rotation::R0).len(), 4);
        assert_eq!(piece_cells(PieceType::Zag, Rotation::R0).len(), 4);
        assert_eq!(piece_cells(PieceType::Bend, Rotation::R0).len(), 4);
        assert_eq!(piece_cells(PieceType::Star, Rotation::R0).len(), 5);
    }

    #[test]
    fn all_rotations_nonempty() {
        for piece in PieceType::ALL {
            for rot in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
                assert!(!piece_cells(piece, rot).is_empty());
            }
        }
    }

    // =========================================================================
    // Collision detection
    // =========================================================================

    #[test]
    fn no_collision_on_empty_board() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = spawn_piece(PieceType::Tee);
        assert!(!collides(&board, &piece));
    }

    #[test]
    fn collision_left_wall() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Bar,
            rotation: Rotation::R0,
            row: 5,
            col: -1,
        };
        assert!(collides(&board, &piece));
    }

    #[test]
    fn collision_right_wall() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Bar,
            rotation: Rotation::R0,
            row: 5,
            col: 6,
        };
        assert!(collides(&board, &piece));
    }

    #[test]
    fn collision_floor() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Bar,
            rotation: Rotation::R90,
            row: 14,
            col: 5,
        };
        assert!(collides(&board, &piece));
    }

    #[test]
    fn collision_with_existing_block() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[5][4] = Some(BlockColor::Amber);
        let piece = ActivePiece {
            piece_type: PieceType::Arc,
            rotation: Rotation::R0,
            row: 4,
            col: 4,
        };
        assert!(collides(&board, &piece));
    }

    #[test]
    fn no_collision_above_board() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Bar,
            rotation: Rotation::R90,
            row: -2,
            col: 5,
        };
        assert!(!collides(&board, &piece));
    }

    // =========================================================================
    // Lock piece
    // =========================================================================

    #[test]
    fn lock_piece_places_cells() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Arc,
            rotation: Rotation::R0,
            row: 0,
            col: 0,
        };
        lock_piece(&mut board, &piece);
        assert_eq!(board[0][0], Some(BlockColor::Sky));
        assert_eq!(board[0][1], Some(BlockColor::Sky));
        assert_eq!(board[1][0], Some(BlockColor::Sky));
    }

    #[test]
    fn lock_piece_skips_negative_rows() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Bar,
            rotation: Rotation::R90,
            row: -2,
            col: 5,
        };
        lock_piece(&mut board, &piece);
        assert_eq!(board[0][5], Some(BlockColor::Lime));
        assert_eq!(board[1][5], Some(BlockColor::Lime));
    }

    // =========================================================================
    // Line clearing
    // =========================================================================

    #[test]
    fn clear_no_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[15][0] = Some(BlockColor::Amber);
        assert_eq!(clear_lines(&mut board), 0);
        assert_eq!(board[15][0], Some(BlockColor::Amber));
    }

    #[test]
    fn clear_single_line() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[15].fill(Some(BlockColor::Amber));
        board[14][0] = Some(BlockColor::Lime);
        assert_eq!(clear_lines(&mut board), 1);
        assert_eq!(board[15][0], Some(BlockColor::Lime));
        assert!(board[14].iter().all(Option::is_none));
    }

    #[test]
    fn clear_double_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[14].fill(Some(BlockColor::Amber));
        board[15].fill(Some(BlockColor::Lime));
        assert_eq!(clear_lines(&mut board), 2);
    }

    #[test]
    fn clear_triple_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[13].fill(Some(BlockColor::Violet));
        board[14].fill(Some(BlockColor::Amber));
        board[15].fill(Some(BlockColor::Lime));
        assert_eq!(clear_lines(&mut board), 3);
    }

    #[test]
    fn clear_four_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        for row in &mut board[12..16] {
            row.fill(Some(BlockColor::Sky));
        }
        assert_eq!(clear_lines(&mut board), 4);
        for row in &board {
            assert!(row.iter().all(Option::is_none));
        }
    }

    #[test]
    fn clear_non_contiguous_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[13].fill(Some(BlockColor::Amber));
        board[15].fill(Some(BlockColor::Lime));
        board[14][0] = Some(BlockColor::Violet);
        assert_eq!(clear_lines(&mut board), 2);
        assert_eq!(board[15][0], Some(BlockColor::Violet));
    }

    // =========================================================================
    // Scoring
    // =========================================================================

    #[test]
    fn score_zero_lines() {
        assert_eq!(calculate_score(0, 0), 0);
        assert_eq!(calculate_score(0, 5), 0);
    }

    #[test]
    fn score_single_line_level_0() {
        assert_eq!(calculate_score(1, 0), 50);
    }

    #[test]
    fn score_double_line_level_0() {
        assert_eq!(calculate_score(2, 0), 200);
    }

    #[test]
    fn score_triple_line_level_0() {
        assert_eq!(calculate_score(3, 0), 450);
    }

    #[test]
    fn score_four_lines_level_0() {
        assert_eq!(calculate_score(4, 0), 800);
    }

    #[test]
    fn score_scales_with_level() {
        assert_eq!(calculate_score(1, 5), 50 * 6);
        assert_eq!(calculate_score(4, 9), 800 * 10);
    }

    // =========================================================================
    // Level
    // =========================================================================

    #[test]
    fn level_from_lines() {
        assert_eq!(calculate_level(0), 0);
        assert_eq!(calculate_level(7), 0);
        assert_eq!(calculate_level(8), 1);
        assert_eq!(calculate_level(39), 4);
        assert_eq!(calculate_level(40), 5);
        assert_eq!(calculate_level(51), 5);
        assert_eq!(calculate_level(52), 6);
    }

    // =========================================================================
    // Tick interval
    // =========================================================================

    #[test]
    fn tick_interval_level_0() {
        assert_eq!(tick_interval_ms(0), 900);
    }

    #[test]
    fn tick_interval_decreases() {
        assert!(tick_interval_ms(1) < tick_interval_ms(0));
    }

    #[test]
    fn tick_interval_minimum() {
        assert_eq!(tick_interval_ms(100), 80);
        assert_eq!(tick_interval_ms(50), 80);
    }

    // =========================================================================
    // Movement functions
    // =========================================================================

    #[test]
    fn move_left_success() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Tee,
            rotation: Rotation::R0,
            row: 5,
            col: 4,
        };
        let moved = try_move_left(&board, &piece).unwrap();
        assert_eq!(moved.col, 3);
        assert_eq!(moved.row, 5);
    }

    #[test]
    fn move_left_blocked() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Tee,
            rotation: Rotation::R0,
            row: 5,
            col: 0,
        };
        assert!(try_move_left(&board, &piece).is_none());
    }

    #[test]
    fn move_right_success() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Tee,
            rotation: Rotation::R0,
            row: 5,
            col: 3,
        };
        let moved = try_move_right(&board, &piece).unwrap();
        assert_eq!(moved.col, 4);
    }

    #[test]
    fn move_right_blocked() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Tee,
            rotation: Rotation::R0,
            row: 5,
            col: 6,
        };
        assert!(try_move_right(&board, &piece).is_none());
    }

    #[test]
    fn move_down_success() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Dash,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        let moved = try_move_down(&board, &piece).unwrap();
        assert_eq!(moved.row, 6);
    }

    #[test]
    fn move_down_blocked_at_floor() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Dash,
            rotation: Rotation::R0,
            row: 15,
            col: 5,
        };
        assert!(try_move_down(&board, &piece).is_none());
    }

    // =========================================================================
    // Hard drop
    // =========================================================================

    #[test]
    fn hard_drop_to_bottom() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Dash,
            rotation: Rotation::R0,
            row: 0,
            col: 5,
        };
        let dropped = hard_drop(&board, &piece);
        assert_eq!(dropped.row, 15);
    }

    #[test]
    fn hard_drop_onto_piece() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[10][5] = Some(BlockColor::Amber);
        let piece = ActivePiece {
            piece_type: PieceType::Dash,
            rotation: Rotation::R0,
            row: 0,
            col: 5,
        };
        let dropped = hard_drop(&board, &piece);
        assert_eq!(dropped.row, 9);
    }

    // =========================================================================
    // Rotation
    // =========================================================================

    #[test]
    fn rotate_cw_success() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Tee,
            rotation: Rotation::R0,
            row: 5,
            col: 4,
        };
        let rotated = try_rotate_cw(&board, &piece).unwrap();
        assert_eq!(rotated.rotation, Rotation::R90);
    }

    #[test]
    fn rotate_cw_near_bottom() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Bar,
            rotation: Rotation::R90,
            row: 14,
            col: 2,
        };
        // R90 -> R180 is horizontal at (14,2)-(14,5) — fits
        let rotated = try_rotate_cw(&board, &piece);
        assert!(rotated.is_some());
    }

    #[test]
    fn rotate_cw_blocked_at_wall() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // Bar vertical at col 7 can't rotate to horizontal — all nudges fail
        let piece = ActivePiece {
            piece_type: PieceType::Bar,
            rotation: Rotation::R90,
            row: 5,
            col: 7,
        };
        assert!(try_rotate_cw(&board, &piece).is_none());
    }

    // =========================================================================
    // Spawn
    // =========================================================================

    #[test]
    fn spawn_centering() {
        // Pip (width 1): col = (8-1)/2 = 3
        assert_eq!(spawn_piece(PieceType::Pip).col, 3);
        // Dash (width 2): col = (8-2)/2 = 3
        assert_eq!(spawn_piece(PieceType::Dash).col, 3);
        // Tee (width 3): col = (8-3)/2 = 2
        assert_eq!(spawn_piece(PieceType::Tee).col, 2);
        // Bar (width 4): col = (8-4)/2 = 2
        assert_eq!(spawn_piece(PieceType::Bar).col, 2);
        // Star (width 3): col = (8-3)/2 = 2
        assert_eq!(spawn_piece(PieceType::Star).col, 2);
    }

    #[test]
    fn spawn_all_pieces_valid() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        for piece_type in PieceType::ALL {
            let piece = spawn_piece(piece_type);
            assert_eq!(piece.row, 0);
            assert_eq!(piece.rotation, Rotation::R0);
            assert!(!collides(&board, &piece), "Spawn collision for {piece_type:?}");
        }
    }

    // =========================================================================
    // New game
    // =========================================================================

    #[test]
    fn new_game_initial_state() {
        let state = new_game(PieceType::Tee, PieceType::Bar);
        assert!(state.active_piece.is_some());
        assert_eq!(state.active_piece.as_ref().unwrap().piece_type, PieceType::Tee);
        assert_eq!(state.next_piece, PieceType::Bar);
        assert!(state.held_piece.is_none());
        assert!(!state.hold_used);
        assert_eq!(state.score, 0);
        assert_eq!(state.level, 0);
        assert_eq!(state.lines_cleared, 0);
        assert!(!state.game_over);
        for row in &state.board {
            assert!(row.iter().all(Option::is_none));
        }
    }

    // =========================================================================
    // Tick
    // =========================================================================

    #[test]
    fn tick_moves_piece_down() {
        let mut state = new_game(PieceType::Tee, PieceType::Bar);
        let orig_row = state.active_piece.as_ref().unwrap().row;
        let (lines, locked) = tick(&mut state, PieceType::Pip);
        assert_eq!(lines, 0);
        assert!(!locked);
        assert_eq!(state.active_piece.as_ref().unwrap().row, orig_row + 1);
    }

    #[test]
    fn tick_locks_at_bottom() {
        let mut state = new_game(PieceType::Dash, PieceType::Tee);
        state.active_piece.as_mut().unwrap().row = 15;
        let (lines, locked) = tick(&mut state, PieceType::Pip);
        assert!(locked);
        assert_eq!(lines, 0);
        assert_eq!(state.active_piece.as_ref().unwrap().piece_type, PieceType::Tee);
        assert_eq!(state.next_piece, PieceType::Pip);
    }

    #[test]
    fn tick_clears_lines() {
        let mut state = new_game(PieceType::Dash, PieceType::Tee);
        // Fill bottom two rows except col 0
        for col in 1..BOARD_WIDTH {
            state.board[14][col] = Some(BlockColor::Amber);
            state.board[15][col] = Some(BlockColor::Amber);
        }
        // Place Dash piece vertically at bottom-left
        state.active_piece = Some(ActivePiece {
            piece_type: PieceType::Dash,
            rotation: Rotation::R90,
            row: 14,
            col: 0,
        });
        let (lines, locked) = tick(&mut state, PieceType::Pip);
        assert!(locked);
        assert_eq!(lines, 2);
        assert_eq!(state.lines_cleared, 2);
        assert!(state.score > 0);
    }

    #[test]
    fn tick_game_over() {
        let mut state = new_game(PieceType::Tee, PieceType::Tee);
        // Fill top rows partially so they don't clear but next spawn collides.
        // Tee spawns at col 2 with cells (0,2)(0,3)(0,4)(1,3) — fill those spots.
        // Leave col 0 empty so the rows don't get cleared.
        for col in 1..BOARD_WIDTH {
            state.board[0][col] = Some(BlockColor::Amber);
            state.board[1][col] = Some(BlockColor::Amber);
        }
        state.active_piece.as_mut().unwrap().row = 14;
        let (_lines, locked) = tick(&mut state, PieceType::Pip);
        assert!(locked);
        assert!(state.game_over);
        assert!(state.active_piece.is_none());
    }

    #[test]
    fn tick_when_game_over_does_nothing() {
        let mut state = new_game(PieceType::Tee, PieceType::Bar);
        state.game_over = true;
        let (lines, locked) = tick(&mut state, PieceType::Pip);
        assert_eq!(lines, 0);
        assert!(!locked);
    }

    #[test]
    fn tick_no_active_piece() {
        let mut state = new_game(PieceType::Tee, PieceType::Bar);
        state.active_piece = None;
        let (lines, locked) = tick(&mut state, PieceType::Pip);
        assert_eq!(lines, 0);
        assert!(!locked);
    }

    // =========================================================================
    // absolute_cells
    // =========================================================================

    #[test]
    fn absolute_cells_of_piece() {
        let piece = ActivePiece {
            piece_type: PieceType::Arc,
            rotation: Rotation::R0,
            row: 5,
            col: 4,
        };
        let cells = absolute_cells(&piece);
        assert_eq!(cells, vec![(5, 4), (5, 5), (6, 4)]);
    }

    // =========================================================================
    // ActivePiece / GameState
    // =========================================================================

    #[test]
    fn active_piece_debug() {
        let piece = spawn_piece(PieceType::Bar);
        let debug = format!("{piece:?}");
        assert!(debug.contains("ActivePiece"));
    }

    #[test]
    fn active_piece_clone() {
        let piece = spawn_piece(PieceType::Tee);
        let cloned = piece.clone();
        assert_eq!(cloned.piece_type, piece.piece_type);
        assert_eq!(cloned.row, piece.row);
        assert_eq!(cloned.col, piece.col);
    }

    #[test]
    fn game_state_debug() {
        let state = new_game(PieceType::Tee, PieceType::Bar);
        let debug = format!("{state:?}");
        assert!(debug.contains("GameState"));
    }

    #[test]
    fn game_state_clone() {
        let state = new_game(PieceType::Tee, PieceType::Bar);
        let cloned = state.clone();
        assert_eq!(cloned.score, state.score);
        assert_eq!(cloned.level, state.level);
    }

    // =========================================================================
    // Counter-clockwise rotation
    // =========================================================================

    #[test]
    fn rotation_ccw_cycle() {
        assert_eq!(Rotation::R0.ccw(), Rotation::R270);
        assert_eq!(Rotation::R90.ccw(), Rotation::R0);
        assert_eq!(Rotation::R180.ccw(), Rotation::R90);
        assert_eq!(Rotation::R270.ccw(), Rotation::R180);
    }

    #[test]
    fn rotation_ccw_full_cycle() {
        let r = Rotation::R0;
        assert_eq!(r.ccw().ccw().ccw().ccw(), Rotation::R0);
    }

    #[test]
    fn cw_ccw_inverse() {
        for rot in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
            assert_eq!(rot.cw().ccw(), rot);
            assert_eq!(rot.ccw().cw(), rot);
        }
    }

    #[test]
    fn rotate_ccw_success() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Tee,
            rotation: Rotation::R0,
            row: 5,
            col: 4,
        };
        let rotated = try_rotate_ccw(&board, &piece).unwrap();
        assert_eq!(rotated.rotation, Rotation::R270);
    }

    #[test]
    fn rotate_ccw_blocked() {
        let mut blocked = [[Some(BlockColor::Amber); BOARD_WIDTH]; BOARD_HEIGHT];
        blocked[10][5] = None;
        blocked[11][5] = None;
        blocked[12][5] = None;
        blocked[13][5] = None;
        let piece = ActivePiece {
            piece_type: PieceType::Bar,
            rotation: Rotation::R90,
            row: 10,
            col: 5,
        };
        assert!(try_rotate_ccw(&blocked, &piece).is_none());
    }

    // =========================================================================
    // Wall kicks
    // =========================================================================

    #[test]
    fn wall_kick_cw_near_floor() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Tee,
            rotation: Rotation::R0,
            row: 14,
            col: 4,
        };
        let rotated = try_rotate_cw(&board, &piece);
        assert!(rotated.is_some());
        assert_eq!(rotated.unwrap().rotation, Rotation::R90);
    }

    #[test]
    fn wall_kick_cw_with_blocks() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[6][5] = Some(BlockColor::Lime);
        let piece = ActivePiece {
            piece_type: PieceType::Tee,
            rotation: Rotation::R0,
            row: 5,
            col: 4,
        };
        let rotated = try_rotate_cw(&board, &piece);
        assert!(rotated.is_some());
        assert_eq!(rotated.unwrap().rotation, Rotation::R90);
    }

    #[test]
    fn wall_kick_bar_piece() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // Bar vertical at col=5, rotating CW to horizontal.
        // R180 cells at col=5: (5,5)(5,6)(5,7)(5,8) — col 8 OOB.
        // Nudge left to col=4: fits.
        let piece = ActivePiece {
            piece_type: PieceType::Bar,
            rotation: Rotation::R90,
            row: 5,
            col: 5,
        };
        let rotated = try_rotate_cw(&board, &piece);
        assert!(rotated.is_some());
        assert_eq!(rotated.unwrap().rotation, Rotation::R180);
    }

    #[test]
    fn wall_kick_ccw_at_wall() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Tee,
            rotation: Rotation::R90,
            row: 5,
            col: -1,
        };
        assert!(try_rotate_ccw(&board, &piece).is_some());
    }

    #[test]
    fn pip_rotation_always_succeeds() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Pip,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        let rotated = try_rotate_cw(&board, &piece).unwrap();
        assert_eq!(rotated.rotation, Rotation::R90);
    }

    // =========================================================================
    // Ghost piece
    // =========================================================================

    #[test]
    fn ghost_piece_on_empty_board() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::Dash,
            rotation: Rotation::R0,
            row: 0,
            col: 5,
        };
        let ghost = ghost_piece(&board, &piece);
        assert_eq!(ghost.row, 15);
    }

    #[test]
    fn ghost_piece_above_blocks() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[10][5] = Some(BlockColor::Lime);
        let piece = ActivePiece {
            piece_type: PieceType::Dash,
            rotation: Rotation::R0,
            row: 0,
            col: 5,
        };
        let ghost = ghost_piece(&board, &piece);
        assert_eq!(ghost.row, 9);
    }

    // =========================================================================
    // lock_and_advance
    // =========================================================================

    #[test]
    fn lock_and_advance_resets_hold() {
        let mut state = new_game(PieceType::Dash, PieceType::Tee);
        state.hold_used = true;
        state.active_piece.as_mut().unwrap().row = 15;
        let piece = state.active_piece.clone().unwrap();
        lock_and_advance(&mut state, &piece, PieceType::Pip);
        assert!(!state.hold_used);
    }

    #[test]
    fn lock_and_advance_spawns_next() {
        let mut state = new_game(PieceType::Dash, PieceType::Tee);
        state.active_piece.as_mut().unwrap().row = 15;
        let piece = state.active_piece.clone().unwrap();
        lock_and_advance(&mut state, &piece, PieceType::Pip);
        assert_eq!(state.active_piece.as_ref().unwrap().piece_type, PieceType::Tee);
        assert_eq!(state.next_piece, PieceType::Pip);
    }

    #[test]
    fn lock_and_advance_game_over() {
        let mut state = new_game(PieceType::Dash, PieceType::Tee);
        // Fill top rows partially (leave col 0 empty to avoid line clears).
        // Tee spawns at col 2 with cells (0,2)(0,3)(0,4)(1,3).
        for col in 1..BOARD_WIDTH {
            state.board[0][col] = Some(BlockColor::Amber);
            state.board[1][col] = Some(BlockColor::Amber);
        }
        state.active_piece.as_mut().unwrap().row = 15;
        let piece = state.active_piece.clone().unwrap();
        lock_and_advance(&mut state, &piece, PieceType::Pip);
        assert!(state.game_over);
    }

    // =========================================================================
    // apply_garbage_line
    // =========================================================================

    #[test]
    fn garbage_line_shifts_rows_up() {
        let mut state = new_game(PieceType::Tee, PieceType::Tee);
        state.board[BOARD_HEIGHT - 1][0] = Some(BlockColor::Amber);
        apply_garbage_line(&mut state, 3);
        assert_eq!(state.board[BOARD_HEIGHT - 2][0], Some(BlockColor::Amber));
    }

    #[test]
    fn garbage_line_fills_bottom_with_grey() {
        let mut state = new_game(PieceType::Tee, PieceType::Tee);
        apply_garbage_line(&mut state, 5);
        let bottom = &state.board[BOARD_HEIGHT - 1];
        for (col, cell) in bottom.iter().enumerate() {
            if col == 5 {
                assert!(cell.is_none());
            } else {
                assert_eq!(*cell, Some(BlockColor::Grey));
            }
        }
    }

    #[test]
    fn garbage_line_gap_clamped() {
        let mut state = new_game(PieceType::Tee, PieceType::Tee);
        apply_garbage_line(&mut state, 999);
        let bottom = &state.board[BOARD_HEIGHT - 1];
        assert!(bottom[BOARD_WIDTH - 1].is_none());
        assert_eq!(bottom[0], Some(BlockColor::Grey));
    }

    #[test]
    fn garbage_line_multiple() {
        let mut state = new_game(PieceType::Tee, PieceType::Tee);
        apply_garbage_line(&mut state, 0);
        apply_garbage_line(&mut state, 1);
        let bottom = &state.board[BOARD_HEIGHT - 1];
        assert!(bottom[1].is_none());
        let second = &state.board[BOARD_HEIGHT - 2];
        assert!(second[0].is_none());
    }

    // =========================================================================
    // BlockColor variants
    // =========================================================================

    #[test]
    fn block_color_grey_debug() {
        let debug = format!("{:?}", BlockColor::Grey);
        assert!(debug.contains("Grey"));
    }

    #[test]
    fn block_color_grey_eq() {
        assert_eq!(BlockColor::Grey, BlockColor::Grey);
        assert_ne!(BlockColor::Grey, BlockColor::Amber);
    }

    #[test]
    fn block_color_grey_copy() {
        let a = BlockColor::Grey;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn block_color_new_variants() {
        let debug = format!("{:?}", BlockColor::White);
        assert!(debug.contains("White"));
        assert_eq!(BlockColor::Blue, BlockColor::Blue);
        assert_ne!(BlockColor::Gold, BlockColor::White);
    }
}
