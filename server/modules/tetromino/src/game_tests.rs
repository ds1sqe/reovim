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
