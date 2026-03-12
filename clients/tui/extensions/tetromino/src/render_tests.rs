use {super::*, reovim_driver_display::FrameBuffer};

use crate::{HeldPieceData, PieceData};

fn make_active_data() -> TetrominoData {
    let mut board = Vec::with_capacity(BOARD_HEIGHT);
    for _ in 0..BOARD_HEIGHT {
        board.push(vec![String::new(); BOARD_WIDTH]);
    }
    // Add some locked blocks at the bottom
    board[15][0] = "amber".to_owned();
    board[15][1] = "lime".to_owned();
    board[15][2] = "violet".to_owned();

    TetrominoData {
        active: true,
        screen: "game".to_owned(),
        paused: false,
        game_over: false,
        score: 1234,
        level: 3,
        lines_cleared: 15,
        next_piece: "Tee".to_owned(),
        next_piece_color: "rose".to_owned(),
        board,
        active_piece: Some(PieceData {
            color: "amber".to_owned(),
            cells: vec![(2, 4), (2, 5), (2, 6), (2, 7)],
        }),
        ghost_piece: Some(PieceData {
            color: "amber".to_owned(),
            cells: vec![(14, 4), (14, 5), (14, 6), (14, 7)],
        }),
        held_piece: None,
        rooms: vec![],
        players: vec![],
        self_ready: false,
        room_id: 0,
        opponents: vec![],
        countdown_remaining: 0,
        winner_id: None,
        result_players: vec![],
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
    assert!(matches!(color_for_name("white"), Color::White));
    assert!(matches!(color_for_name("amber"), Color::DarkYellow));
    assert!(matches!(color_for_name("teal"), Color::DarkCyan));
    assert!(matches!(color_for_name("sky"), Color::Cyan));
    assert!(matches!(color_for_name("lime"), Color::Green));
    assert!(matches!(color_for_name("rose"), Color::Magenta));
    assert!(matches!(color_for_name("violet"), Color::Blue));
    assert!(matches!(color_for_name("coral"), Color::Red));
    assert!(matches!(color_for_name("blue"), Color::DarkBlue));
    assert!(matches!(color_for_name("gold"), Color::Yellow));
    assert!(matches!(color_for_name("grey"), Color::DarkGrey));
    assert!(matches!(color_for_name("unknown"), Color::DarkGrey));
    assert!(matches!(color_for_name(""), Color::DarkGrey));
}

#[test]
fn next_piece_preview_all_types() {
    assert_eq!(next_piece_preview_cells("Pip").len(), 1);
    assert_eq!(next_piece_preview_cells("Dash").len(), 2);
    assert_eq!(next_piece_preview_cells("Line").len(), 3);
    assert_eq!(next_piece_preview_cells("Arc").len(), 3);
    assert_eq!(next_piece_preview_cells("Bar").len(), 4);
    assert_eq!(next_piece_preview_cells("Tee").len(), 4);
    assert_eq!(next_piece_preview_cells("Skew").len(), 4);
    assert_eq!(next_piece_preview_cells("Zag").len(), 4);
    assert_eq!(next_piece_preview_cells("Bend").len(), 4);
    assert_eq!(next_piece_preview_cells("Star").len(), 5);
    assert!(next_piece_preview_cells("?").is_empty());
}

#[test]
fn render_board_with_all_colors() {
    let mut fb = FrameBuffer::new(80, 30);
    let mut data = make_active_data();
    data.board[14][0] = "teal".to_owned();
    data.board[14][1] = "rose".to_owned();
    data.board[14][2] = "sky".to_owned();
    data.board[14][3] = "coral".to_owned();
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_piece_at_negative_coords() {
    let mut fb = FrameBuffer::new(80, 30);
    let mut data = make_active_data();
    data.active_piece = Some(PieceData {
        color: "amber".to_owned(),
        cells: vec![(-1, 4), (-1, 5), (0, 4), (0, 5)],
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
        piece_type: "Tee".to_owned(),
        color: "rose".to_owned(),
    });
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_ghost_piece() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = make_active_data();
    render_tetromino(&mut fb, &data);
}

#[test]
fn held_piece_data_debug() {
    let held = HeldPieceData {
        piece_type: "Bar".to_owned(),
        color: "lime".to_owned(),
    };
    let debug = format!("{held:?}");
    assert!(debug.contains("lime"));
}

// =========================================================================
// Menu / Lobby / Room screen rendering tests
// =========================================================================

#[test]
fn render_menu_screen() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "menu".to_owned(),
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_menu_too_small() {
    let mut fb = FrameBuffer::new(10, 5);
    let data = TetrominoData {
        active: true,
        screen: "menu".to_owned(),
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_lobby_screen() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "lobby".to_owned(),
        rooms: vec![
            crate::RoomData {
                id: 1,
                player_count: 2,
                status: "Waiting".to_owned(),
            },
            crate::RoomData {
                id: 2,
                player_count: 1,
                status: "InProgress".to_owned(),
            },
        ],
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_lobby_empty() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "lobby".to_owned(),
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_lobby_too_small() {
    let mut fb = FrameBuffer::new(10, 5);
    let data = TetrominoData {
        active: true,
        screen: "lobby".to_owned(),
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_room_screen() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "room".to_owned(),
        room_id: 1,
        self_ready: true,
        players: vec![
            crate::PlayerData { id: 1, ready: true },
            crate::PlayerData {
                id: 2,
                ready: false,
            },
        ],
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_room_not_ready() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "room".to_owned(),
        room_id: 3,
        self_ready: false,
        players: vec![crate::PlayerData {
            id: 1,
            ready: false,
        }],
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_room_too_small() {
    let mut fb = FrameBuffer::new(10, 5);
    let data = TetrominoData {
        active: true,
        screen: "room".to_owned(),
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_game_with_opponents() {
    let mut fb = FrameBuffer::new(120, 30);
    let mut data = make_active_data();
    let mut opp_board = vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT];
    opp_board[14][0] = "coral".to_owned();
    opp_board[15][0] = "amber".to_owned();
    data.opponents = vec![OpponentData {
        client_id: 2,
        board: opp_board,
        score: 500,
        game_over: false,
    }];
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_opponent_game_over() {
    let mut fb = FrameBuffer::new(120, 30);
    let mut data = make_active_data();
    data.opponents = vec![OpponentData {
        client_id: 3,
        board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
        score: 200,
        game_over: true,
    }];
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_multiple_opponents() {
    let mut fb = FrameBuffer::new(120, 60);
    let mut data = make_active_data();
    data.opponents = vec![
        OpponentData {
            client_id: 2,
            board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
            score: 100,
            game_over: false,
        },
        OpponentData {
            client_id: 3,
            board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
            score: 200,
            game_over: true,
        },
    ];
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_opponents_direct() {
    let mut fb = FrameBuffer::new(80, 30);
    let opponents = vec![OpponentData {
        client_id: 5,
        board: vec![],
        score: 42,
        game_over: false,
    }];
    render_opponents(&mut fb, &opponents, 0, 0);
}

#[test]
fn render_board_with_grey() {
    let mut fb = FrameBuffer::new(80, 30);
    let mut data = make_active_data();
    data.board[15][3] = "grey".to_owned();
    render_tetromino(&mut fb, &data);
}

// =========================================================================
// Countdown rendering tests (#544)
// =========================================================================

#[test]
fn render_countdown_screen() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "countdown".to_owned(),
        room_id: 1,
        countdown_remaining: 3,
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_countdown_go() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "countdown".to_owned(),
        room_id: 1,
        countdown_remaining: 0,
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_countdown_too_small() {
    let mut fb = FrameBuffer::new(10, 3);
    let data = TetrominoData {
        active: true,
        screen: "countdown".to_owned(),
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

// =========================================================================
// Reactive minimap tests (#544)
// =========================================================================

#[test]
fn opponents_hidden_when_terminal_too_narrow() {
    // POPUP_W (32) + 1 + MINI_W (10) = 43 minimum for minimaps
    let mut fb = FrameBuffer::new(40, 22);
    let mut data = make_active_data();
    data.opponents = vec![OpponentData {
        client_id: 2,
        board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
        score: 100,
        game_over: false,
    }];
    render_tetromino(&mut fb, &data);
    // Should still render without panic — opponents just hidden
}

#[test]
fn opponents_shown_when_terminal_wide_enough() {
    // 44 columns = POPUP_W(32) + 1 + MINI_W(10) + 1 margin
    let mut fb = FrameBuffer::new(44, 22);
    let mut data = make_active_data();
    data.opponents = vec![OpponentData {
        client_id: 2,
        board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
        score: 100,
        game_over: false,
    }];
    render_tetromino(&mut fb, &data);
}

#[test]
fn opponents_limited_by_height() {
    // Only 60 rows — each opponent needs ~11 rows (MINI_H+1)
    // capped by POPUP_H = 22
    let mut fb = FrameBuffer::new(120, 30);
    let mut data = make_active_data();
    data.opponents = vec![
        OpponentData {
            client_id: 2,
            board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
            score: 100,
            game_over: false,
        },
        OpponentData {
            client_id: 3,
            board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
            score: 200,
            game_over: false,
        },
        OpponentData {
            client_id: 4,
            board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
            score: 300,
            game_over: false,
        },
    ];
    render_tetromino(&mut fb, &data);
}

#[test]
fn opponents_that_fit_none() {
    assert_eq!(opponents_that_fit(5, 1), 0);
    assert_eq!(opponents_that_fit(0, 1), 0);
    assert_eq!(opponents_that_fit(22, 0), 0);
}

#[test]
fn opponents_that_fit_one() {
    // MINI_H + 1 = 11 needed for one opponent
    assert_eq!(opponents_that_fit(11, 1), 1);
    assert_eq!(opponents_that_fit(14, 3), 1);
}

#[test]
fn opponents_that_fit_multiple() {
    // First: 11, each additional: 12 (gap + label + mini)
    // 11 + 12 = 23 for two
    assert_eq!(opponents_that_fit(23, 2), 2);
    assert_eq!(opponents_that_fit(29, 5), 2);
    // 11 + 12 + 12 = 35 for three
    assert_eq!(opponents_that_fit(35, 3), 3);
}

// =========================================================================
// Result screen rendering tests
// =========================================================================

#[test]
fn render_result_screen() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "result".to_owned(),
        winner_id: Some(2),
        result_players: vec![
            crate::ResultPlayerData { id: 1, score: 1200 },
            crate::ResultPlayerData { id: 2, score: 3400 },
        ],
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_result_no_winner() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "result".to_owned(),
        winner_id: None,
        result_players: vec![crate::ResultPlayerData { id: 1, score: 100 }],
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_result_too_small() {
    let mut fb = FrameBuffer::new(10, 5);
    let data = TetrominoData {
        active: true,
        screen: "result".to_owned(),
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}

#[test]
fn render_result_empty_players() {
    let mut fb = FrameBuffer::new(80, 30);
    let data = TetrominoData {
        active: true,
        screen: "result".to_owned(),
        winner_id: Some(1),
        result_players: vec![],
        ..TetrominoData::default()
    };
    render_tetromino(&mut fb, &data);
}
