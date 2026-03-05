//! Pure tetromino game logic with zero framework dependencies.
//!
//! All functions are pure and testable. The board is 10 columns x 20 rows,
//! with row 0 at the top. Pieces use the standard 7 tetrominoes with
//! basic rotation (no wall kicks in v1).

/// Board width in cells.
pub const BOARD_WIDTH: usize = 10;

/// Board height in cells.
pub const BOARD_HEIGHT: usize = 20;

/// Cell color corresponding to each tetromino type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockColor {
    Cyan,   // I
    Yellow, // O
    Purple, // T
    Green,  // S
    Red,    // Z
    Blue,   // J
    Orange, // L
}

/// The 7 standard tetrominoes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PieceType {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

impl PieceType {
    /// All piece types for iteration.
    pub const ALL: [Self; 7] = [
        Self::I,
        Self::O,
        Self::T,
        Self::S,
        Self::Z,
        Self::J,
        Self::L,
    ];

    /// Get the block color for this piece type.
    #[must_use]
    pub const fn color(self) -> BlockColor {
        match self {
            Self::I => BlockColor::Cyan,
            Self::O => BlockColor::Yellow,
            Self::T => BlockColor::Purple,
            Self::S => BlockColor::Green,
            Self::Z => BlockColor::Red,
            Self::J => BlockColor::Blue,
            Self::L => BlockColor::Orange,
        }
    }

    /// Get the color name as a string (for JSON serialization).
    #[must_use]
    pub const fn color_name(self) -> &'static str {
        match self {
            Self::I => "cyan",
            Self::O => "yellow",
            Self::T => "purple",
            Self::S => "green",
            Self::Z => "red",
            Self::J => "blue",
            Self::L => "orange",
        }
    }

    /// Get the piece type name as a string.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::I => "I",
            Self::O => "O",
            Self::T => "T",
            Self::S => "S",
            Self::Z => "Z",
            Self::J => "J",
            Self::L => "L",
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
/// Returns 4 `(row, col)` offsets relative to the piece's origin.
#[must_use]
pub const fn piece_cells(piece: PieceType, rotation: Rotation) -> [(i32, i32); 4] {
    match piece {
        PieceType::I => match rotation {
            Rotation::R0 | Rotation::R180 => [(0, 0), (0, 1), (0, 2), (0, 3)],
            Rotation::R90 | Rotation::R270 => [(0, 0), (1, 0), (2, 0), (3, 0)],
        },
        PieceType::O => [(0, 0), (0, 1), (1, 0), (1, 1)],
        PieceType::T => match rotation {
            Rotation::R0 => [(0, 0), (0, 1), (0, 2), (1, 1)],
            Rotation::R90 => [(0, 0), (1, 0), (2, 0), (1, 1)],
            Rotation::R180 => [(1, 0), (1, 1), (1, 2), (0, 1)],
            Rotation::R270 => [(0, 1), (1, 1), (2, 1), (1, 0)],
        },
        PieceType::S => match rotation {
            Rotation::R0 | Rotation::R180 => [(0, 1), (0, 2), (1, 0), (1, 1)],
            Rotation::R90 | Rotation::R270 => [(0, 0), (1, 0), (1, 1), (2, 1)],
        },
        PieceType::Z => match rotation {
            Rotation::R0 | Rotation::R180 => [(0, 0), (0, 1), (1, 1), (1, 2)],
            Rotation::R90 | Rotation::R270 => [(0, 1), (1, 0), (1, 1), (2, 0)],
        },
        PieceType::J => match rotation {
            Rotation::R0 => [(0, 0), (1, 0), (1, 1), (1, 2)],
            Rotation::R90 => [(0, 0), (0, 1), (1, 0), (2, 0)],
            Rotation::R180 => [(0, 0), (0, 1), (0, 2), (1, 2)],
            Rotation::R270 => [(0, 1), (1, 1), (2, 0), (2, 1)],
        },
        PieceType::L => match rotation {
            Rotation::R0 => [(0, 2), (1, 0), (1, 1), (1, 2)],
            Rotation::R90 => [(0, 0), (1, 0), (2, 0), (2, 1)],
            Rotation::R180 => [(0, 0), (0, 1), (0, 2), (1, 0)],
            Rotation::R270 => [(0, 0), (0, 1), (1, 1), (2, 1)],
        },
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
    for (dr, dc) in piece_cells(piece.piece_type, piece.rotation) {
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
    for (dr, dc) in piece_cells(piece.piece_type, piece.rotation) {
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

/// Calculate score for cleared lines at given level (NES scoring).
#[must_use]
pub const fn calculate_score(lines: u32, level: u32) -> u32 {
    let base = match lines {
        0 => 0,
        1 => 40,
        2 => 100,
        3 => 300,
        _ => 1200,
    };
    base * (level + 1)
}

/// Calculate level from total lines cleared (every 10 lines).
#[must_use]
pub const fn calculate_level(total_lines: u32) -> u32 {
    total_lines / 10
}

/// Calculate tick interval in milliseconds from level.
///
/// Starts at 1000ms (level 0), decreasing by 75ms per level,
/// with a minimum of 100ms.
#[must_use]
pub const fn tick_interval_ms(level: u32) -> u64 {
    let base = 1000u64;
    let reduction = level as u64 * 75;
    if reduction >= base - 100 {
        100
    } else {
        base - reduction
    }
}

/// SRS wall kick offsets for J, L, S, T, Z pieces.
///
/// Each entry is `(from_rotation, to_rotation) -> [(col_offset, row_offset); 5]`.
/// Test 0 is always (0,0) (no offset). Tests 1-4 are the wall kick attempts.
#[must_use]
const fn srs_offsets_jlstz(from: Rotation, to: Rotation) -> [(i32, i32); 5] {
    match (from, to) {
        (Rotation::R0 | Rotation::R180, Rotation::R90) => {
            [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)]
        }
        (Rotation::R90, Rotation::R0 | Rotation::R180) => {
            [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)]
        }
        (Rotation::R180 | Rotation::R0, Rotation::R270) => {
            [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)]
        }
        (Rotation::R270, Rotation::R180 | Rotation::R0) => {
            [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)]
        }
        _ => [(0, 0); 5],
    }
}

/// SRS wall kick offsets for the I piece (different from JLSTZ).
#[must_use]
const fn srs_offsets_i(from: Rotation, to: Rotation) -> [(i32, i32); 5] {
    match (from, to) {
        (Rotation::R0, Rotation::R90) | (Rotation::R270, Rotation::R180) => {
            [(0, 0), (-2, 0), (1, 0), (-2, 1), (1, -2)]
        }
        (Rotation::R90, Rotation::R0) | (Rotation::R180, Rotation::R270) => {
            [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, 2)]
        }
        (Rotation::R90, Rotation::R180) | (Rotation::R0, Rotation::R270) => {
            [(0, 0), (-1, 0), (2, 0), (-1, -2), (2, 1)]
        }
        (Rotation::R180, Rotation::R90) | (Rotation::R270, Rotation::R0) => {
            [(0, 0), (1, 0), (-2, 0), (1, 2), (-2, -1)]
        }
        _ => [(0, 0); 5],
    }
}

/// Get the SRS wall kick offsets for a piece rotation transition.
#[must_use]
const fn wall_kick_offsets(piece: PieceType, from: Rotation, to: Rotation) -> [(i32, i32); 5] {
    match piece {
        PieceType::I => srs_offsets_i(from, to),
        PieceType::O => [(0, 0); 5], // O never needs wall kicks
        _ => srs_offsets_jlstz(from, to),
    }
}

/// Try to rotate the piece clockwise with SRS wall kicks.
#[must_use]
pub fn try_rotate_cw(board: &Board, piece: &ActivePiece) -> Option<ActivePiece> {
    let to_rot = piece.rotation.cw();
    let offsets = wall_kick_offsets(piece.piece_type, piece.rotation, to_rot);
    for (dc, dr) in offsets {
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

/// Try to rotate the piece counter-clockwise with SRS wall kicks.
#[must_use]
pub fn try_rotate_ccw(board: &Board, piece: &ActivePiece) -> Option<ActivePiece> {
    let to_rot = piece.rotation.ccw();
    let offsets = wall_kick_offsets(piece.piece_type, piece.rotation, to_rot);
    for (dc, dr) in offsets {
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

/// Spawn a new piece at the top center of the board.
#[must_use]
pub const fn spawn_piece(piece_type: PieceType) -> ActivePiece {
    ActivePiece {
        piece_type,
        rotation: Rotation::R0,
        row: 0,
        col: 3,
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

/// Check if the last locked T-piece constitutes a T-spin.
///
/// A T-spin occurs when a T-piece is locked and at least 3 of the 4
/// diagonal corners around its center are occupied (by walls or blocks).
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
pub fn is_t_spin(board: &Board, piece: &ActivePiece) -> bool {
    if piece.piece_type != PieceType::T {
        return false;
    }
    // T-piece center is at offset (1,1) for R0/R180 or varies by rotation.
    // Use the canonical center: for T in any rotation, the center cell is
    // the one shared by all rotations conceptually. We compute it from the
    // piece origin based on rotation.
    let (center_r, center_c) = match piece.rotation {
        Rotation::R0 => (piece.row, piece.col + 1),
        Rotation::R90 => (piece.row + 1, piece.col),
        Rotation::R180 | Rotation::R270 => (piece.row + 1, piece.col + 1),
    };

    let corners = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
    let mut filled = 0u8;
    for (dr, dc) in corners {
        let r = center_r + dr;
        let c = center_c + dc;
        if r < 0 || r >= BOARD_HEIGHT as i32 || c < 0 || c >= BOARD_WIDTH as i32 {
            filled += 1; // Walls count as filled
        } else if board[r as usize][c as usize].is_some() {
            filled += 1;
        }
    }
    filled >= 3
}

/// Calculate T-spin bonus score.
#[must_use]
pub const fn t_spin_score(lines: u32, level: u32) -> u32 {
    let base = match lines {
        0 => 400,  // T-spin no lines
        1 => 800,  // T-spin single
        2 => 1200, // T-spin double
        _ => 1600, // T-spin triple
    };
    base * (level + 1)
}

/// Lock the current piece, clear lines, spawn next. Shared by tick/soft-drop/hard-drop.
///
/// Returns the number of lines cleared.
pub fn lock_and_advance(
    state: &mut GameState,
    piece: &ActivePiece,
    next_next_piece: PieceType,
) -> u32 {
    let t_spin = is_t_spin(&state.board, piece);
    lock_piece(&mut state.board, piece);
    let lines = clear_lines(&mut state.board);
    state.lines_cleared += lines;
    if t_spin {
        state.score += t_spin_score(lines, state.level);
    } else {
        state.score += calculate_score(lines, state.level);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PieceType
    // =========================================================================

    #[test]
    fn piece_type_all_has_seven() {
        assert_eq!(PieceType::ALL.len(), 7);
    }

    #[test]
    fn piece_type_colors() {
        assert_eq!(PieceType::I.color(), BlockColor::Cyan);
        assert_eq!(PieceType::O.color(), BlockColor::Yellow);
        assert_eq!(PieceType::T.color(), BlockColor::Purple);
        assert_eq!(PieceType::S.color(), BlockColor::Green);
        assert_eq!(PieceType::Z.color(), BlockColor::Red);
        assert_eq!(PieceType::J.color(), BlockColor::Blue);
        assert_eq!(PieceType::L.color(), BlockColor::Orange);
    }

    #[test]
    fn piece_type_color_names() {
        assert_eq!(PieceType::I.color_name(), "cyan");
        assert_eq!(PieceType::O.color_name(), "yellow");
        assert_eq!(PieceType::T.color_name(), "purple");
        assert_eq!(PieceType::S.color_name(), "green");
        assert_eq!(PieceType::Z.color_name(), "red");
        assert_eq!(PieceType::J.color_name(), "blue");
        assert_eq!(PieceType::L.color_name(), "orange");
    }

    #[test]
    fn piece_type_names() {
        assert_eq!(PieceType::I.name(), "I");
        assert_eq!(PieceType::O.name(), "O");
        assert_eq!(PieceType::T.name(), "T");
        assert_eq!(PieceType::S.name(), "S");
        assert_eq!(PieceType::Z.name(), "Z");
        assert_eq!(PieceType::J.name(), "J");
        assert_eq!(PieceType::L.name(), "L");
    }

    #[test]
    fn piece_type_debug() {
        let debug = format!("{:?}", PieceType::I);
        assert!(debug.contains('I'));
    }

    #[test]
    fn piece_type_eq() {
        assert_eq!(PieceType::I, PieceType::I);
        assert_ne!(PieceType::I, PieceType::O);
    }

    #[test]
    fn piece_type_clone() {
        let p = PieceType::T;
        #[allow(clippy::clone_on_copy)]
        let cloned = p.clone();
        assert_eq!(p, cloned);
    }

    #[test]
    fn block_color_debug() {
        let debug = format!("{:?}", BlockColor::Cyan);
        assert!(debug.contains("Cyan"));
    }

    #[test]
    fn block_color_eq() {
        assert_eq!(BlockColor::Cyan, BlockColor::Cyan);
        assert_ne!(BlockColor::Cyan, BlockColor::Red);
    }

    #[test]
    fn block_color_clone() {
        let c = BlockColor::Purple;
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
    // piece_cells - all rotations for all pieces
    // =========================================================================

    #[test]
    fn i_piece_cells_r0() {
        let cells = piece_cells(PieceType::I, Rotation::R0);
        assert_eq!(cells, [(0, 0), (0, 1), (0, 2), (0, 3)]);
    }

    #[test]
    fn i_piece_cells_r90() {
        let cells = piece_cells(PieceType::I, Rotation::R90);
        assert_eq!(cells, [(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn i_piece_cells_r180() {
        // Same as R0 for I piece
        assert_eq!(
            piece_cells(PieceType::I, Rotation::R180),
            piece_cells(PieceType::I, Rotation::R0)
        );
    }

    #[test]
    fn i_piece_cells_r270() {
        // Same as R90 for I piece
        assert_eq!(
            piece_cells(PieceType::I, Rotation::R270),
            piece_cells(PieceType::I, Rotation::R90)
        );
    }

    #[test]
    fn o_piece_all_rotations_identical() {
        let base = piece_cells(PieceType::O, Rotation::R0);
        assert_eq!(piece_cells(PieceType::O, Rotation::R90), base);
        assert_eq!(piece_cells(PieceType::O, Rotation::R180), base);
        assert_eq!(piece_cells(PieceType::O, Rotation::R270), base);
    }

    #[test]
    fn t_piece_cells_all_rotations() {
        assert_eq!(piece_cells(PieceType::T, Rotation::R0), [(0, 0), (0, 1), (0, 2), (1, 1)]);
        assert_eq!(piece_cells(PieceType::T, Rotation::R90), [(0, 0), (1, 0), (2, 0), (1, 1)]);
        assert_eq!(piece_cells(PieceType::T, Rotation::R180), [(1, 0), (1, 1), (1, 2), (0, 1)]);
        assert_eq!(piece_cells(PieceType::T, Rotation::R270), [(0, 1), (1, 1), (2, 1), (1, 0)]);
    }

    #[test]
    fn s_piece_cells_all_rotations() {
        let horiz = piece_cells(PieceType::S, Rotation::R0);
        assert_eq!(horiz, [(0, 1), (0, 2), (1, 0), (1, 1)]);
        assert_eq!(piece_cells(PieceType::S, Rotation::R180), horiz);

        let vert = piece_cells(PieceType::S, Rotation::R90);
        assert_eq!(vert, [(0, 0), (1, 0), (1, 1), (2, 1)]);
        assert_eq!(piece_cells(PieceType::S, Rotation::R270), vert);
    }

    #[test]
    fn z_piece_cells_all_rotations() {
        let horiz = piece_cells(PieceType::Z, Rotation::R0);
        assert_eq!(horiz, [(0, 0), (0, 1), (1, 1), (1, 2)]);
        assert_eq!(piece_cells(PieceType::Z, Rotation::R180), horiz);

        let vert = piece_cells(PieceType::Z, Rotation::R90);
        assert_eq!(vert, [(0, 1), (1, 0), (1, 1), (2, 0)]);
        assert_eq!(piece_cells(PieceType::Z, Rotation::R270), vert);
    }

    #[test]
    fn j_piece_cells_all_rotations() {
        assert_eq!(piece_cells(PieceType::J, Rotation::R0), [(0, 0), (1, 0), (1, 1), (1, 2)]);
        assert_eq!(piece_cells(PieceType::J, Rotation::R90), [(0, 0), (0, 1), (1, 0), (2, 0)]);
        assert_eq!(piece_cells(PieceType::J, Rotation::R180), [(0, 0), (0, 1), (0, 2), (1, 2)]);
        assert_eq!(piece_cells(PieceType::J, Rotation::R270), [(0, 1), (1, 1), (2, 0), (2, 1)]);
    }

    #[test]
    fn l_piece_cells_all_rotations() {
        assert_eq!(piece_cells(PieceType::L, Rotation::R0), [(0, 2), (1, 0), (1, 1), (1, 2)]);
        assert_eq!(piece_cells(PieceType::L, Rotation::R90), [(0, 0), (1, 0), (2, 0), (2, 1)]);
        assert_eq!(piece_cells(PieceType::L, Rotation::R180), [(0, 0), (0, 1), (0, 2), (1, 0)]);
        assert_eq!(piece_cells(PieceType::L, Rotation::R270), [(0, 0), (0, 1), (1, 1), (2, 1)]);
    }

    #[test]
    fn each_piece_has_four_cells() {
        for piece in PieceType::ALL {
            for rot in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
                assert_eq!(piece_cells(piece, rot).len(), 4);
            }
        }
    }

    // =========================================================================
    // Collision detection
    // =========================================================================

    #[test]
    fn no_collision_on_empty_board() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = spawn_piece(PieceType::T);
        assert!(!collides(&board, &piece));
    }

    #[test]
    fn collision_left_wall() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::I,
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
            piece_type: PieceType::I,
            rotation: Rotation::R0,
            row: 5,
            col: 8,
        };
        assert!(collides(&board, &piece));
    }

    #[test]
    fn collision_floor() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::I,
            rotation: Rotation::R90,
            row: 18,
            col: 5,
        };
        assert!(collides(&board, &piece));
    }

    #[test]
    fn collision_with_existing_block() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[5][4] = Some(BlockColor::Cyan);
        let piece = ActivePiece {
            piece_type: PieceType::O,
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
            piece_type: PieceType::I,
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
            piece_type: PieceType::O,
            rotation: Rotation::R0,
            row: 0,
            col: 0,
        };
        lock_piece(&mut board, &piece);
        assert_eq!(board[0][0], Some(BlockColor::Yellow));
        assert_eq!(board[0][1], Some(BlockColor::Yellow));
        assert_eq!(board[1][0], Some(BlockColor::Yellow));
        assert_eq!(board[1][1], Some(BlockColor::Yellow));
    }

    #[test]
    fn lock_piece_skips_negative_rows() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::I,
            rotation: Rotation::R90,
            row: -2,
            col: 5,
        };
        lock_piece(&mut board, &piece);
        // Only rows 0 and 1 should have blocks (rows -2 and -1 are skipped)
        assert_eq!(board[0][5], Some(BlockColor::Cyan));
        assert_eq!(board[1][5], Some(BlockColor::Cyan));
    }

    // =========================================================================
    // Line clearing
    // =========================================================================

    #[test]
    fn clear_no_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[19][0] = Some(BlockColor::Cyan);
        assert_eq!(clear_lines(&mut board), 0);
        assert_eq!(board[19][0], Some(BlockColor::Cyan));
    }

    #[test]
    fn clear_single_line() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[19].fill(Some(BlockColor::Cyan));
        board[18][0] = Some(BlockColor::Red);
        assert_eq!(clear_lines(&mut board), 1);
        // Row 18 should have shifted down to 19
        assert_eq!(board[19][0], Some(BlockColor::Red));
        // Row 18 should now be empty
        assert!(board[18].iter().all(Option::is_none));
    }

    #[test]
    fn clear_double_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[18].fill(Some(BlockColor::Cyan));
        board[19].fill(Some(BlockColor::Red));
        assert_eq!(clear_lines(&mut board), 2);
    }

    #[test]
    fn clear_triple_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[17].fill(Some(BlockColor::Blue));
        board[18].fill(Some(BlockColor::Cyan));
        board[19].fill(Some(BlockColor::Red));
        assert_eq!(clear_lines(&mut board), 3);
    }

    #[test]
    fn clear_four_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        for row in &mut board[16..20] {
            row.fill(Some(BlockColor::Green));
        }
        assert_eq!(clear_lines(&mut board), 4);
        // All rows should be empty after clearing 4 bottom rows
        for row in &board {
            assert!(row.iter().all(Option::is_none));
        }
    }

    #[test]
    fn clear_non_contiguous_lines() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // Fill rows 17 and 19 (not 18)
        board[17].fill(Some(BlockColor::Cyan));
        board[19].fill(Some(BlockColor::Red));
        board[18][0] = Some(BlockColor::Blue);
        assert_eq!(clear_lines(&mut board), 2);
        // The partial row 18 should shift down
        assert_eq!(board[19][0], Some(BlockColor::Blue));
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
        assert_eq!(calculate_score(1, 0), 40);
    }

    #[test]
    fn score_double_line_level_0() {
        assert_eq!(calculate_score(2, 0), 100);
    }

    #[test]
    fn score_triple_line_level_0() {
        assert_eq!(calculate_score(3, 0), 300);
    }

    #[test]
    fn score_four_lines_level_0() {
        assert_eq!(calculate_score(4, 0), 1200);
    }

    #[test]
    fn score_scales_with_level() {
        assert_eq!(calculate_score(1, 5), 40 * 6);
        assert_eq!(calculate_score(4, 9), 1200 * 10);
    }

    // =========================================================================
    // Level
    // =========================================================================

    #[test]
    fn level_from_lines() {
        assert_eq!(calculate_level(0), 0);
        assert_eq!(calculate_level(9), 0);
        assert_eq!(calculate_level(10), 1);
        assert_eq!(calculate_level(25), 2);
    }

    // =========================================================================
    // Tick interval
    // =========================================================================

    #[test]
    fn tick_interval_level_0() {
        assert_eq!(tick_interval_ms(0), 1000);
    }

    #[test]
    fn tick_interval_decreases() {
        assert!(tick_interval_ms(1) < tick_interval_ms(0));
    }

    #[test]
    fn tick_interval_minimum() {
        assert_eq!(tick_interval_ms(100), 100);
        assert_eq!(tick_interval_ms(12), 100);
    }

    // =========================================================================
    // Movement functions
    // =========================================================================

    #[test]
    fn move_left_success() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        let moved = try_move_left(&board, &piece).unwrap();
        assert_eq!(moved.col, 4);
        assert_eq!(moved.row, 5);
    }

    #[test]
    fn move_left_blocked() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::T,
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
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        let moved = try_move_right(&board, &piece).unwrap();
        assert_eq!(moved.col, 6);
    }

    #[test]
    fn move_right_blocked() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 5,
            col: 7,
        };
        assert!(try_move_right(&board, &piece).is_none());
    }

    #[test]
    fn move_down_success() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::O,
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
            piece_type: PieceType::O,
            rotation: Rotation::R0,
            row: 18,
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
            piece_type: PieceType::O,
            rotation: Rotation::R0,
            row: 0,
            col: 5,
        };
        let dropped = hard_drop(&board, &piece);
        assert_eq!(dropped.row, 18);
    }

    #[test]
    fn hard_drop_onto_piece() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[10][5] = Some(BlockColor::Cyan);
        let piece = ActivePiece {
            piece_type: PieceType::O,
            rotation: Rotation::R0,
            row: 0,
            col: 5,
        };
        let dropped = hard_drop(&board, &piece);
        assert_eq!(dropped.row, 8);
    }

    // =========================================================================
    // Rotation
    // =========================================================================

    #[test]
    fn rotate_cw_success() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        let rotated = try_rotate_cw(&board, &piece).unwrap();
        assert_eq!(rotated.rotation, Rotation::R90);
    }

    #[test]
    fn rotate_cw_blocked() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // I piece at right wall in R0 would go out of bounds when rotated to R90
        let piece = ActivePiece {
            piece_type: PieceType::I,
            rotation: Rotation::R90,
            row: 18,
            col: 5,
        };
        // R90 -> R180 is horizontal, at row 18, col 5, cells span (18,5)..(18,8)
        // That's within bounds, so this should succeed
        let rotated = try_rotate_cw(&board, &piece);
        assert!(rotated.is_some());
    }

    #[test]
    fn rotate_cw_blocked_at_wall() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // I piece vertical at col 9 can't rotate to horizontal
        let piece = ActivePiece {
            piece_type: PieceType::I,
            rotation: Rotation::R90,
            row: 5,
            col: 9,
        };
        assert!(try_rotate_cw(&board, &piece).is_none());
    }

    // =========================================================================
    // Spawn
    // =========================================================================

    #[test]
    fn spawn_at_top_center() {
        let piece = spawn_piece(PieceType::T);
        assert_eq!(piece.row, 0);
        assert_eq!(piece.col, 3);
        assert_eq!(piece.rotation, Rotation::R0);
        assert_eq!(piece.piece_type, PieceType::T);
    }

    // =========================================================================
    // New game
    // =========================================================================

    #[test]
    fn new_game_initial_state() {
        let state = new_game(PieceType::T, PieceType::I);
        assert!(state.active_piece.is_some());
        assert_eq!(state.active_piece.as_ref().unwrap().piece_type, PieceType::T);
        assert_eq!(state.next_piece, PieceType::I);
        assert!(state.held_piece.is_none());
        assert!(!state.hold_used);
        assert_eq!(state.score, 0);
        assert_eq!(state.level, 0);
        assert_eq!(state.lines_cleared, 0);
        assert!(!state.game_over);
        // Board should be empty
        for row in &state.board {
            assert!(row.iter().all(Option::is_none));
        }
    }

    // =========================================================================
    // Tick
    // =========================================================================

    #[test]
    fn tick_moves_piece_down() {
        let mut state = new_game(PieceType::O, PieceType::T);
        let orig_row = state.active_piece.as_ref().unwrap().row;
        let (lines, locked) = tick(&mut state, PieceType::I);
        assert_eq!(lines, 0);
        assert!(!locked);
        assert_eq!(state.active_piece.as_ref().unwrap().row, orig_row + 1);
    }

    #[test]
    fn tick_locks_at_bottom() {
        let mut state = new_game(PieceType::O, PieceType::T);
        // Move piece to near bottom
        state.active_piece.as_mut().unwrap().row = 18;
        let (lines, locked) = tick(&mut state, PieceType::I);
        assert!(locked);
        assert_eq!(lines, 0);
        // New piece should be spawned
        assert_eq!(state.active_piece.as_ref().unwrap().piece_type, PieceType::T);
        assert_eq!(state.next_piece, PieceType::I);
    }

    #[test]
    fn tick_clears_lines() {
        let mut state = new_game(PieceType::O, PieceType::T);
        // Fill bottom two rows except cols 0-1
        for col in 2..BOARD_WIDTH {
            state.board[18][col] = Some(BlockColor::Cyan);
            state.board[19][col] = Some(BlockColor::Cyan);
        }
        // Place O piece at bottom-left (cols 0-1, rows 18-19)
        state.active_piece = Some(ActivePiece {
            piece_type: PieceType::O,
            rotation: Rotation::R0,
            row: 18,
            col: 0,
        });
        let (lines, locked) = tick(&mut state, PieceType::I);
        // Can't move down, so piece locks and clears 2 lines
        assert!(locked);
        assert_eq!(lines, 2);
        assert_eq!(state.lines_cleared, 2);
        assert!(state.score > 0);
    }

    #[test]
    fn tick_game_over() {
        let mut state = new_game(PieceType::O, PieceType::O);
        // Fill the top so the next spawn collides
        for col in 3..5 {
            state.board[0][col] = Some(BlockColor::Cyan);
            state.board[1][col] = Some(BlockColor::Cyan);
        }
        // Force lock the current piece
        state.active_piece.as_mut().unwrap().row = 18;
        let (_lines, locked) = tick(&mut state, PieceType::T);
        assert!(locked);
        assert!(state.game_over);
        assert!(state.active_piece.is_none());
    }

    #[test]
    fn tick_when_game_over_does_nothing() {
        let mut state = new_game(PieceType::T, PieceType::I);
        state.game_over = true;
        let (lines, locked) = tick(&mut state, PieceType::O);
        assert_eq!(lines, 0);
        assert!(!locked);
    }

    #[test]
    fn tick_no_active_piece() {
        let mut state = new_game(PieceType::T, PieceType::I);
        state.active_piece = None;
        let (lines, locked) = tick(&mut state, PieceType::O);
        assert_eq!(lines, 0);
        assert!(!locked);
    }

    // =========================================================================
    // absolute_cells
    // =========================================================================

    #[test]
    fn absolute_cells_of_piece() {
        let piece = ActivePiece {
            piece_type: PieceType::O,
            rotation: Rotation::R0,
            row: 5,
            col: 3,
        };
        let cells = absolute_cells(&piece);
        assert_eq!(cells, vec![(5, 3), (5, 4), (6, 3), (6, 4)]);
    }

    // =========================================================================
    // ActivePiece / GameState
    // =========================================================================

    #[test]
    fn active_piece_debug() {
        let piece = spawn_piece(PieceType::I);
        let debug = format!("{piece:?}");
        assert!(debug.contains("ActivePiece"));
    }

    #[test]
    fn active_piece_clone() {
        let piece = spawn_piece(PieceType::T);
        let cloned = piece.clone();
        assert_eq!(cloned.piece_type, piece.piece_type);
        assert_eq!(cloned.row, piece.row);
        assert_eq!(cloned.col, piece.col);
    }

    #[test]
    fn game_state_debug() {
        let state = new_game(PieceType::T, PieceType::I);
        let debug = format!("{state:?}");
        assert!(debug.contains("GameState"));
    }

    #[test]
    fn game_state_clone() {
        let state = new_game(PieceType::T, PieceType::I);
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
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        let rotated = try_rotate_ccw(&board, &piece).unwrap();
        assert_eq!(rotated.rotation, Rotation::R270);
    }

    #[test]
    fn rotate_ccw_blocked() {
        // Surrounded on all sides — no wall kick can help
        let mut blocked = [[Some(BlockColor::Cyan); BOARD_WIDTH]; BOARD_HEIGHT];
        blocked[10][5] = None;
        blocked[11][5] = None;
        blocked[12][5] = None;
        blocked[13][5] = None;
        let piece = ActivePiece {
            piece_type: PieceType::I,
            rotation: Rotation::R90,
            row: 10,
            col: 5,
        };
        assert!(try_rotate_ccw(&blocked, &piece).is_none());
    }

    // =========================================================================
    // Wall kicks (SRS)
    // =========================================================================

    #[test]
    fn wall_kick_cw_near_floor() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // T piece at R0 near floor, row=18. CW to R90 would put a cell
        // at row 20 (out of bounds). SRS offset test 3 (dc=-1, dr=-1)
        // kicks it up+left to fit.
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 18,
            col: 4,
        };
        let rotated = try_rotate_cw(&board, &piece);
        assert!(rotated.is_some());
        let r = rotated.unwrap();
        assert_eq!(r.rotation, Rotation::R90);
    }

    #[test]
    fn wall_kick_cw_with_blocks() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // Place blocks that prevent basic rotation
        board[6][5] = Some(BlockColor::Red);
        // T piece at R0, col=4, row=5. CW to R90 would put a cell at (6,5)
        // which is blocked. SRS kick should find an alternate position.
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 5,
            col: 4,
        };
        let rotated = try_rotate_cw(&board, &piece);
        assert!(rotated.is_some());
        let r = rotated.unwrap();
        assert_eq!(r.rotation, Rotation::R90);
    }

    #[test]
    fn wall_kick_i_piece() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // I piece vertical (R90) at col=7, rotating CW to R180 (horizontal).
        // R180 cells at col=7: (5,7)(5,8)(5,9)(5,10) — col 10 out of bounds.
        // SRS test 2 (dc=-1, dr=0) kicks to col=6: (5,6)(5,7)(5,8)(5,9) — fits.
        let piece = ActivePiece {
            piece_type: PieceType::I,
            rotation: Rotation::R90,
            row: 5,
            col: 7,
        };
        let rotated = try_rotate_cw(&board, &piece);
        assert!(rotated.is_some());
        let r = rotated.unwrap();
        assert_eq!(r.rotation, Rotation::R180);
    }

    #[test]
    fn wall_kick_ccw_at_wall() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R90,
            row: 5,
            col: -1,
        };
        let rotated = try_rotate_ccw(&board, &piece);
        assert!(rotated.is_some());
    }

    #[test]
    fn o_piece_no_wall_kick_needed() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::O,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        // O piece rotation is identity — always succeeds
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
            piece_type: PieceType::O,
            rotation: Rotation::R0,
            row: 0,
            col: 5,
        };
        let ghost = ghost_piece(&board, &piece);
        assert_eq!(ghost.row, 18); // O piece lands at row 18 (rows 18-19)
    }

    #[test]
    fn ghost_piece_above_blocks() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        board[10][5] = Some(BlockColor::Red);
        let piece = ActivePiece {
            piece_type: PieceType::O,
            rotation: Rotation::R0,
            row: 0,
            col: 5,
        };
        let ghost = ghost_piece(&board, &piece);
        assert_eq!(ghost.row, 8);
    }

    // =========================================================================
    // T-spin detection
    // =========================================================================

    #[test]
    fn t_spin_not_t_piece() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::I,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        assert!(!is_t_spin(&board, &piece));
    }

    #[test]
    fn t_spin_no_corners_filled() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        assert!(!is_t_spin(&board, &piece));
    }

    #[test]
    fn t_spin_three_corners_filled() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // T at R0, origin (5,5), center at (5,6)
        // Corners: (4,5), (4,7), (6,5), (6,7)
        board[4][5] = Some(BlockColor::Cyan);
        board[4][7] = Some(BlockColor::Cyan);
        board[6][5] = Some(BlockColor::Cyan);
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 5,
            col: 5,
        };
        assert!(is_t_spin(&board, &piece));
    }

    #[test]
    fn t_spin_wall_counts_as_filled() {
        let board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // T at R90, origin (0,0), center at (1,0)
        // Corners: (0,-1), (0,1), (2,-1), (2,1)
        // col -1 is wall -> 2 filled automatically
        // Need 1 more corner filled
        let mut board2 = board;
        board2[0][1] = Some(BlockColor::Cyan); // (0,1) is a corner
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R90,
            row: 0,
            col: 0,
        };
        assert!(is_t_spin(&board2, &piece));
    }

    #[test]
    fn t_spin_r180() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // T at R180, origin (5,3), center at (6,4)
        // Corners: (5,3), (5,5), (7,3), (7,5)
        board[5][3] = Some(BlockColor::Cyan);
        board[5][5] = Some(BlockColor::Cyan);
        board[7][3] = Some(BlockColor::Cyan);
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R180,
            row: 5,
            col: 3,
        };
        assert!(is_t_spin(&board, &piece));
    }

    #[test]
    fn t_spin_r270() {
        let mut board = [[None; BOARD_WIDTH]; BOARD_HEIGHT];
        // T at R270, origin (5,3), center at (6,4)
        // Corners: (5,3), (5,5), (7,3), (7,5)
        board[5][3] = Some(BlockColor::Cyan);
        board[5][5] = Some(BlockColor::Cyan);
        board[7][3] = Some(BlockColor::Cyan);
        let piece = ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R270,
            row: 5,
            col: 3,
        };
        assert!(is_t_spin(&board, &piece));
    }

    // =========================================================================
    // T-spin scoring
    // =========================================================================

    #[test]
    fn t_spin_score_values() {
        assert_eq!(t_spin_score(0, 0), 400);
        assert_eq!(t_spin_score(1, 0), 800);
        assert_eq!(t_spin_score(2, 0), 1200);
        assert_eq!(t_spin_score(3, 0), 1600);
    }

    #[test]
    fn t_spin_score_scales_with_level() {
        assert_eq!(t_spin_score(1, 5), 800 * 6);
        assert_eq!(t_spin_score(2, 9), 1200 * 10);
    }

    // =========================================================================
    // lock_and_advance
    // =========================================================================

    #[test]
    fn lock_and_advance_resets_hold() {
        let mut state = new_game(PieceType::O, PieceType::T);
        state.hold_used = true;
        state.active_piece.as_mut().unwrap().row = 18;
        let piece = state.active_piece.clone().unwrap();
        lock_and_advance(&mut state, &piece, PieceType::I);
        assert!(!state.hold_used);
    }

    #[test]
    fn lock_and_advance_spawns_next() {
        let mut state = new_game(PieceType::O, PieceType::T);
        state.active_piece.as_mut().unwrap().row = 18;
        let piece = state.active_piece.clone().unwrap();
        lock_and_advance(&mut state, &piece, PieceType::I);
        assert_eq!(state.active_piece.as_ref().unwrap().piece_type, PieceType::T);
        assert_eq!(state.next_piece, PieceType::I);
    }

    #[test]
    fn lock_and_advance_game_over() {
        let mut state = new_game(PieceType::O, PieceType::O);
        // Fill spawn zone
        for col in 3..5 {
            state.board[0][col] = Some(BlockColor::Cyan);
            state.board[1][col] = Some(BlockColor::Cyan);
        }
        state.active_piece.as_mut().unwrap().row = 18;
        let piece = state.active_piece.clone().unwrap();
        lock_and_advance(&mut state, &piece, PieceType::T);
        assert!(state.game_over);
    }

    #[test]
    fn lock_and_advance_with_t_spin() {
        let mut state = new_game(PieceType::T, PieceType::I);
        // Set up T-spin scenario: 3 corners filled around T center
        // T at R0, origin (18,5), center at (18,6)
        // Corners: (17,5), (17,7), (19,5), (19,7)
        state.board[17][5] = Some(BlockColor::Cyan);
        state.board[17][7] = Some(BlockColor::Cyan);
        state.board[19][5] = Some(BlockColor::Cyan);
        state.active_piece = Some(ActivePiece {
            piece_type: PieceType::T,
            rotation: Rotation::R0,
            row: 18,
            col: 5,
        });
        let piece = state.active_piece.clone().unwrap();
        let lines = lock_and_advance(&mut state, &piece, PieceType::S);
        assert_eq!(lines, 0); // No full lines
        // T-spin no lines = 400 * (level + 1) = 400
        assert_eq!(state.score, 400);
    }
}
