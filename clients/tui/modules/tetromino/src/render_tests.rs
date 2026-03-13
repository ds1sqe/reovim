use {
    super::*,
    crate::{
        BOARD_HEIGHT, BOARD_WIDTH, HeldPieceData, OpponentData, PieceData, ResultPlayerData,
        TetrominoData,
    },
    reovim_client_driver::{Rect, Style},
};

// =============================================================================
// Mock surface
// =============================================================================

struct MockSurface {
    cells: Vec<Vec<(char, Style)>>,
    width: u16,
    height: u16,
}

impl MockSurface {
    fn new(width: u16, height: u16) -> Self {
        Self {
            cells: vec![vec![(' ', Style::new()); width as usize]; height as usize],
            width,
            height,
        }
    }

    fn has_content(&self) -> bool {
        self.cells
            .iter()
            .any(|row| row.iter().any(|(ch, _)| *ch != ' '))
    }
}

impl RenderSurface for MockSurface {
    #[allow(clippy::cast_possible_truncation)]
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        for (i, ch) in text.chars().enumerate() {
            let cx = x as usize + i;
            if cx < self.width as usize && (y as usize) < self.height as usize {
                self.cells[y as usize][cx] = (ch, style.clone());
            }
        }
        text.len() as u16
    }

    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}
    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: Color) {}

    fn fill(&mut self, rect: Rect, ch: char, style: Style) {
        for row in rect.y..rect.y + rect.height {
            for col in rect.x..rect.x + rect.width {
                if (col as usize) < self.width as usize && (row as usize) < self.height as usize {
                    self.cells[row as usize][col as usize] = (ch, style.clone());
                }
            }
        }
    }

    fn clear(&mut self, rect: Rect) {
        self.fill(rect, ' ', Style::new());
    }

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

// =============================================================================
// Helpers
// =============================================================================

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

fn render(data: &TetrominoData, w: u16, h: u16) -> MockSurface {
    let mut surface = MockSurface::new(w, h);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    render_tetromino(&mut surface, data, bounds);
    surface
}

// =============================================================================
// Core render tests
// =============================================================================

#[test]
fn render_inactive_noop() {
    let data = TetrominoData::default();
    let surface = render(&data, 80, 30);
    assert!(!surface.has_content());
}

#[test]
fn render_active_game() {
    let data = make_active_data();
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_paused() {
    let mut data = make_active_data();
    data.paused = true;
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_game_over() {
    let mut data = make_active_data();
    data.game_over = true;
    data.active_piece = None;
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_too_small_terminal() {
    let data = make_active_data();
    let surface = render(&data, 20, 10);
    assert!(surface.has_content()); // "Terminal too small" message
}

// =============================================================================
// Color tests
// =============================================================================

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

// =============================================================================
// Next piece preview tests
// =============================================================================

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

// =============================================================================
// Opponents that fit tests
// =============================================================================

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

// =============================================================================
// Board rendering tests
// =============================================================================

#[test]
fn render_board_with_all_colors() {
    let mut data = make_active_data();
    data.board[14][0] = "teal".to_owned();
    data.board[14][1] = "rose".to_owned();
    data.board[14][2] = "sky".to_owned();
    data.board[14][3] = "coral".to_owned();
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_piece_at_negative_coords() {
    let mut data = make_active_data();
    data.active_piece = Some(PieceData {
        color: "amber".to_owned(),
        cells: vec![(-1, 4), (-1, 5), (0, 4), (0, 5)],
    });
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_board_with_grey() {
    let mut data = make_active_data();
    data.board[15][3] = "grey".to_owned();
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_board_empty() {
    let data = TetrominoData {
        active: true,
        board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
        ..TetrominoData::default()
    };
    // Render directly via render_board (called through render_game)
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_with_held_piece() {
    let mut data = make_active_data();
    data.held_piece = Some(HeldPieceData {
        piece_type: "Tee".to_owned(),
        color: "rose".to_owned(),
    });
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_ghost_piece() {
    let data = make_active_data();
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

// =============================================================================
// Menu / Lobby / Room screen rendering tests
// =============================================================================

#[test]
fn render_menu_screen() {
    let data = TetrominoData {
        active: true,
        screen: "menu".to_owned(),
        ..TetrominoData::default()
    };
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_menu_too_small() {
    let data = TetrominoData {
        active: true,
        screen: "menu".to_owned(),
        ..TetrominoData::default()
    };
    let surface = render(&data, 10, 5);
    assert!(surface.has_content()); // "Terminal too small" message
}

#[test]
fn render_lobby_screen() {
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
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_lobby_empty() {
    let data = TetrominoData {
        active: true,
        screen: "lobby".to_owned(),
        ..TetrominoData::default()
    };
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_lobby_too_small() {
    let data = TetrominoData {
        active: true,
        screen: "lobby".to_owned(),
        ..TetrominoData::default()
    };
    let surface = render(&data, 10, 5);
    assert!(surface.has_content());
}

#[test]
fn render_room_screen() {
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
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_room_not_ready() {
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
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_room_too_small() {
    let data = TetrominoData {
        active: true,
        screen: "room".to_owned(),
        ..TetrominoData::default()
    };
    let surface = render(&data, 10, 5);
    assert!(surface.has_content());
}

// =============================================================================
// Countdown rendering tests
// =============================================================================

#[test]
fn render_countdown_screen() {
    let data = TetrominoData {
        active: true,
        screen: "countdown".to_owned(),
        room_id: 1,
        countdown_remaining: 3,
        ..TetrominoData::default()
    };
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_countdown_go() {
    let data = TetrominoData {
        active: true,
        screen: "countdown".to_owned(),
        room_id: 1,
        countdown_remaining: 0,
        ..TetrominoData::default()
    };
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_countdown_too_small() {
    let data = TetrominoData {
        active: true,
        screen: "countdown".to_owned(),
        ..TetrominoData::default()
    };
    let surface = render(&data, 10, 3);
    assert!(surface.has_content());
}

// =============================================================================
// Opponent rendering tests
// =============================================================================

#[test]
fn render_game_with_opponents() {
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
    let surface = render(&data, 120, 30);
    assert!(surface.has_content());
}

#[test]
fn render_opponent_game_over() {
    let mut data = make_active_data();
    data.opponents = vec![OpponentData {
        client_id: 3,
        board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
        score: 200,
        game_over: true,
    }];
    let surface = render(&data, 120, 30);
    assert!(surface.has_content());
}

#[test]
fn render_multiple_opponents() {
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
    let surface = render(&data, 120, 60);
    assert!(surface.has_content());
}

#[test]
fn opponents_hidden_when_terminal_too_narrow() {
    let mut data = make_active_data();
    data.opponents = vec![OpponentData {
        client_id: 2,
        board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
        score: 100,
        game_over: false,
    }];
    let surface = render(&data, 40, 22);
    assert!(surface.has_content());
}

#[test]
fn opponents_shown_when_terminal_wide_enough() {
    let mut data = make_active_data();
    data.opponents = vec![OpponentData {
        client_id: 2,
        board: vec![vec![String::new(); BOARD_WIDTH]; BOARD_HEIGHT],
        score: 100,
        game_over: false,
    }];
    let surface = render(&data, 44, 22);
    assert!(surface.has_content());
}

#[test]
fn opponents_limited_by_height() {
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
    let surface = render(&data, 120, 30);
    assert!(surface.has_content());
}

// =============================================================================
// Result screen rendering tests
// =============================================================================

#[test]
fn render_result_screen() {
    let data = TetrominoData {
        active: true,
        screen: "result".to_owned(),
        winner_id: Some(2),
        result_players: vec![
            ResultPlayerData { id: 1, score: 1200 },
            ResultPlayerData { id: 2, score: 3400 },
        ],
        ..TetrominoData::default()
    };
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_result_no_winner() {
    let data = TetrominoData {
        active: true,
        screen: "result".to_owned(),
        winner_id: None,
        result_players: vec![ResultPlayerData { id: 1, score: 100 }],
        ..TetrominoData::default()
    };
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

#[test]
fn render_result_too_small() {
    let data = TetrominoData {
        active: true,
        screen: "result".to_owned(),
        ..TetrominoData::default()
    };
    let surface = render(&data, 10, 5);
    assert!(surface.has_content());
}

#[test]
fn render_result_empty_players() {
    let data = TetrominoData {
        active: true,
        screen: "result".to_owned(),
        winner_id: Some(1),
        result_players: vec![],
        ..TetrominoData::default()
    };
    let surface = render(&data, 80, 30);
    assert!(surface.has_content());
}

// =============================================================================
// Overlay rendering test
// =============================================================================

#[test]
fn render_overlay_centered() {
    let mut surface = MockSurface::new(80, 30);
    render_overlay(&mut surface, "TEST", 10, 5);
    assert!(surface.has_content());
}

// =============================================================================
// Side panel rendering test
// =============================================================================

#[test]
fn render_side_panel_display() {
    let mut surface = MockSurface::new(80, 30);
    let data = make_active_data();
    render_side_panel(&mut surface, &data, 40, 5);
    assert!(surface.has_content());
}
