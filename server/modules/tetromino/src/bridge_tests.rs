use {
    super::*,
    crate::{
        game::{ActivePiece, BlockColor, PieceType, Rotation},
        state::{ClientId, RoomId},
    },
    reovim_kernel::api::v1::ServiceRegistry,
};

/// Empty service registry for tick tests (tetromino doesn't use services).
fn services() -> ServiceRegistry {
    ServiceRegistry::new()
}

#[test]
fn bridge_kind() {
    assert_eq!(TetrominoBridge.kind(), "polyblocks");
}

#[test]
fn bridge_scope() {
    assert_eq!(TetrominoBridge.scope(), ExtensionScope::Client);
}

#[test]
fn snapshot_no_state_returns_none() {
    let map = ExtensionMap::new();
    assert!(TetrominoBridge.snapshot(&map).is_none());
}

#[test]
fn snapshot_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TetrominoState>();

    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], false);
    assert!(snap.get("board").is_none());
}

#[test]
fn snapshot_menu_screen() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Menu;

    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    assert_eq!(snap["screen"], "menu");
}

#[test]
fn snapshot_lobby_screen_fallback() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Lobby;

    // Without context, lobby falls back to menu
    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
}

#[test]
fn snapshot_active_no_game() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Game;

    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], false);
}

#[test]
fn snapshot_active_with_game() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.start_game();
    state.screen = TetrominoScreen::Game;

    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    assert_eq!(snap["screen"], "game");
    assert_eq!(snap["paused"], false);
    assert_eq!(snap["gameOver"], false);
    assert_eq!(snap["score"], 0);
    assert_eq!(snap["level"], 0);
    assert_eq!(snap["linesCleared"], 0);

    // Board should be 16 rows of 8 columns
    let board = snap["board"].as_array().unwrap();
    assert_eq!(board.len(), 16);
    assert_eq!(board[0].as_array().unwrap().len(), 8);

    // Active piece should exist
    assert!(snap["activePiece"].is_object());
    assert!(snap["activePiece"]["type"].is_string());
    assert!(snap["activePiece"]["color"].is_string());
    assert!(snap["activePiece"]["cells"].is_array());

    // Ghost piece should exist (same piece type, lower row)
    assert!(snap["ghostPiece"].is_object());
    assert!(snap["ghostPiece"]["cells"].is_array());

    // Held piece should be null initially
    assert!(snap["heldPiece"].is_null());

    // Next piece
    assert!(snap["nextPiece"].is_string());
    assert!(snap["nextPieceColor"].is_string());
}

#[test]
fn snapshot_with_held_piece() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.start_game();
    state.screen = TetrominoScreen::Game;
    state.hold_piece();

    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert!(snap["heldPiece"].is_object());
    assert!(snap["heldPiece"]["type"].is_string());
    assert!(snap["heldPiece"]["color"].is_string());
}

#[test]
fn snapshot_paused() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.start_game();
    state.screen = TetrominoScreen::Game;
    state.paused = true;

    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert_eq!(snap["paused"], true);
}

#[test]
fn snapshot_game_over() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.start_game();
    state.screen = TetrominoScreen::Game;
    state.game.as_mut().unwrap().game_over = true;
    state.game.as_mut().unwrap().active_piece = None;

    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert_eq!(snap["gameOver"], true);
    assert!(snap["activePiece"].is_null());
}

#[test]
fn snapshot_board_with_blocks() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.start_game();
    state.screen = TetrominoScreen::Game;
    state.game.as_mut().unwrap().board[15][0] = Some(BlockColor::Amber);
    state.game.as_mut().unwrap().board[15][1] = Some(BlockColor::Lime);

    let snap = TetrominoBridge.snapshot(&map).unwrap();
    let board = snap["board"].as_array().unwrap();
    let last_row = board[15].as_array().unwrap();
    assert_eq!(last_row[0], "amber");
    assert_eq!(last_row[1], "lime");
    assert_eq!(last_row[2], "");
}

#[test]
fn snapshot_board_with_grey_blocks() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.start_game();
    state.screen = TetrominoScreen::Game;
    state.game.as_mut().unwrap().board[15][0] = Some(BlockColor::Grey);

    let snap = TetrominoBridge.snapshot(&map).unwrap();
    let board = snap["board"].as_array().unwrap();
    let last_row = board[15].as_array().unwrap();
    assert_eq!(last_row[0], "grey");
}

#[test]
fn is_active_no_state() {
    let map = ExtensionMap::new();
    assert!(!TetrominoBridge.is_active(&map));
}

#[test]
fn is_active_inactive_state() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TetrominoState>();
    assert!(!TetrominoBridge.is_active(&map));
}

#[test]
fn is_active_active_state() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    assert!(TetrominoBridge.is_active(&map));
}

#[test]
fn serialize_board_empty() {
    let board = [[None; game::BOARD_WIDTH]; game::BOARD_HEIGHT];
    let result = serialize_board(&board);
    assert_eq!(result.len(), game::BOARD_HEIGHT);
    for row in &result {
        assert!(row.iter().all(|c| c.is_empty()));
    }
}

#[test]
fn serialize_board_all_colors() {
    let mut board = [[None; game::BOARD_WIDTH]; game::BOARD_HEIGHT];
    board[0][0] = Some(BlockColor::White);
    board[0][1] = Some(BlockColor::Amber);
    board[0][2] = Some(BlockColor::Teal);
    board[0][3] = Some(BlockColor::Sky);
    board[0][4] = Some(BlockColor::Lime);
    board[0][5] = Some(BlockColor::Rose);
    board[0][6] = Some(BlockColor::Violet);
    board[0][7] = Some(BlockColor::Coral);
    board[1][0] = Some(BlockColor::Blue);
    board[1][1] = Some(BlockColor::Gold);
    board[1][2] = Some(BlockColor::Grey);

    let result = serialize_board(&board);
    assert_eq!(result[0][0], "white");
    assert_eq!(result[0][1], "amber");
    assert_eq!(result[0][2], "teal");
    assert_eq!(result[0][3], "sky");
    assert_eq!(result[0][4], "lime");
    assert_eq!(result[0][5], "rose");
    assert_eq!(result[0][6], "violet");
    assert_eq!(result[0][7], "coral");
    assert_eq!(result[1][0], "blue");
    assert_eq!(result[1][1], "gold");
    assert_eq!(result[1][2], "grey");
}

#[test]
fn serialize_piece_format() {
    let piece = ActivePiece {
        piece_type: PieceType::Tee,
        rotation: Rotation::R0,
        row: 5,
        col: 3,
    };
    let json = serialize_piece(&piece);
    assert_eq!(json["type"], "Tee");
    assert_eq!(json["color"], "rose");
    let cells = json["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 4);
}

// =========================================================================
// snapshot_with_context tests
// =========================================================================

fn make_context<'a>(
    client_id: ClientId,
    shared: &'a ExtensionMap,
    opponents: &'a [(ClientId, &'a ExtensionMap)],
) -> BridgeContext<'a> {
    BridgeContext::new(client_id, shared, opponents)
}

#[test]
fn context_snapshot_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TetrominoState>();
    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["active"], false);
}

#[test]
fn context_snapshot_menu() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Menu;
    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["screen"], "menu");
}

#[test]
fn context_snapshot_lobby_with_rooms() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Lobby;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    lobby.create_room();
    let ctx = make_context(ClientId(1), &shared, &[]);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["screen"], "lobby");
    let rooms = snap["rooms"].as_array().unwrap();
    assert_eq!(rooms.len(), 1);
}

#[test]
fn context_snapshot_lobby_no_lobby_state() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Lobby;

    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["screen"], "lobby");
    let rooms = snap["rooms"].as_array().unwrap();
    assert!(rooms.is_empty());
}

#[test]
fn context_snapshot_room() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Room;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    lobby.toggle_ready(room_id, ClientId(1));

    state.current_room = Some(room_id);
    let ctx = make_context(ClientId(1), &shared, &[]);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["screen"], "room");
    assert_eq!(snap["selfReady"], true);
    let players = snap["players"].as_array().unwrap();
    assert_eq!(players.len(), 2);
}

#[test]
fn context_snapshot_room_no_room_id() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Room;
    // No current_room set
    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    assert!(TetrominoBridge.snapshot_with_context(&map, &ctx).is_none());
}

#[test]
fn context_snapshot_room_no_lobby() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Room;
    state.current_room = Some(RoomId(1));

    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    assert!(TetrominoBridge.snapshot_with_context(&map, &ctx).is_none());
}

#[test]
fn context_snapshot_game_single_player() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.start_game();
    state.screen = TetrominoScreen::Game;
    state.multiplayer = false;

    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["screen"], "game");
    assert!(snap.get("opponents").is_none());
}

#[test]
fn context_snapshot_game_multiplayer_with_opponents() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.start_game();
    state.screen = TetrominoScreen::Game;
    state.multiplayer = true;

    // Set up an opponent
    let mut opp_map = ExtensionMap::new();
    let opp = opp_map.get_or_insert::<TetrominoState>();
    opp.start_game();
    opp.multiplayer = true;

    let shared = ExtensionMap::new();
    let opponents = vec![(ClientId(2), &opp_map as &ExtensionMap)];
    let ctx = make_context(ClientId(1), &shared, &opponents);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["screen"], "game");
    let opps = snap["opponents"].as_array().unwrap();
    assert_eq!(opps.len(), 1);
    assert!(opps[0]["board"].is_array());
    assert!(opps[0]["score"].is_number());
}

#[test]
fn context_snapshot_game_multiplayer_no_opponents() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.start_game();
    state.screen = TetrominoScreen::Game;
    state.multiplayer = true;

    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    let opps = snap["opponents"].as_array().unwrap();
    assert!(opps.is_empty());
}

#[test]
fn context_snapshot_no_state() {
    let map = ExtensionMap::new();
    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);
    assert!(TetrominoBridge.snapshot_with_context(&map, &ctx).is_none());
}

// =========================================================================
// tick() tests (#546)
// =========================================================================

#[test]
fn tick_no_state() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    // get_or_insert creates default (inactive) state — should return false
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

#[test]
fn tick_inactive() {
    let mut client = ExtensionMap::new();
    client.get_or_insert::<TetrominoState>();
    let mut shared = ExtensionMap::new();
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

#[test]
fn tick_paused() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.start_game();
    state.paused = true;
    let mut shared = ExtensionMap::new();
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

#[test]
fn tick_game_over() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.start_game();
    state.game.as_mut().unwrap().game_over = true;
    let mut shared = ExtensionMap::new();
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

#[test]
fn tick_no_game() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    // game is None
    let mut shared = ExtensionMap::new();
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

#[test]
fn tick_gravity_applies() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Game;
    state.start_game();
    // Force tick to be due by setting last_tick to distant past
    state.last_tick = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap();
    let mut shared = ExtensionMap::new();

    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));
    // last_tick should have been reset
    let state = client.get::<TetrominoState>().unwrap();
    assert!(state.last_tick.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn tick_no_gravity_when_not_due() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Game;
    state.start_game();
    // last_tick is now — not due yet
    let mut shared = ExtensionMap::new();
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

#[test]
fn tick_garbage_multiplayer() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Game;
    state.multiplayer = true;
    state.client_id = Some(ClientId(1));
    state.start_game();

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    lobby.broadcast_garbage(room_id, ClientId(2), 1);

    let state = client.get_or_insert::<TetrominoState>();
    state.current_room = Some(room_id);

    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));
    // Garbage should have been applied
    let game = client
        .get::<TetrominoState>()
        .unwrap()
        .game
        .as_ref()
        .unwrap();
    let bottom = &game.board[crate::game::BOARD_HEIGHT - 1];
    assert!(bottom.iter().any(Option::is_some));
}

#[test]
fn tick_multiplayer_no_garbage() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Game;
    state.multiplayer = true;
    state.client_id = Some(ClientId(1));
    state.start_game();

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));

    let state = client.get_or_insert::<TetrominoState>();
    state.current_room = Some(room_id);
    // No garbage queued, tick not due → false
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

#[test]
fn tick_multiplayer_no_room() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.start_game();
    state.multiplayer = true;
    state.client_id = Some(ClientId(1));
    // No current_room set
    let mut shared = ExtensionMap::new();
    // Should not panic, just skip garbage
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

#[test]
fn tick_multiplayer_no_client_id() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.start_game();
    state.multiplayer = true;
    state.current_room = Some(RoomId(1));
    // No client_id set
    let mut shared = ExtensionMap::new();
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

// =========================================================================
// Countdown snapshot tests (#544)
// =========================================================================

#[test]
fn snapshot_countdown_fallback() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Countdown;

    // Without context, countdown falls back to menu
    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    assert_eq!(snap["screen"], "menu");
}

#[test]
fn context_snapshot_countdown() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Countdown;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    lobby.start_countdown(room_id);

    state.current_room = Some(room_id);
    let ctx = make_context(ClientId(1), &shared, &[]);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["screen"], "countdown");
    assert_eq!(snap["roomId"], room_id.0);
    assert!(snap["remaining"].is_number());
}

#[test]
fn context_snapshot_countdown_no_room_id() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Countdown;
    // No current_room set
    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    assert!(TetrominoBridge.snapshot_with_context(&map, &ctx).is_none());
}

#[test]
fn context_snapshot_countdown_no_lobby() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Countdown;
    state.current_room = Some(RoomId(1));

    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    assert!(TetrominoBridge.snapshot_with_context(&map, &ctx).is_none());
}

// =========================================================================
// Countdown tick tests (#544)
// =========================================================================

#[test]
fn tick_countdown_still_counting() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Countdown;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.start_countdown(room_id);

    let state = client.get_or_insert::<TetrominoState>();
    state.current_room = Some(room_id);

    // Countdown just started — should return true (notify for display update)
    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));
    // Screen should still be Countdown
    let state = client.get::<TetrominoState>().unwrap();
    assert_eq!(state.screen, TetrominoScreen::Countdown);
}

#[test]
fn tick_countdown_expired_starts_game() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Countdown;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.start_countdown(room_id);
    // Force countdown to have expired by backdating the start
    let room = lobby.rooms.get_mut(&room_id).unwrap();
    room.countdown_start = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .unwrap(),
    );

    let state = client.get_or_insert::<TetrominoState>();
    state.current_room = Some(room_id);

    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));
    // Screen should transition to Game
    let state = client.get::<TetrominoState>().unwrap();
    assert_eq!(state.screen, TetrominoScreen::Game);
    assert!(state.multiplayer);
    assert!(state.game.is_some());
    // Room should be InProgress
    let lobby = shared.get::<TetrominoLobbyState>().unwrap();
    assert!(lobby.is_in_progress(room_id));
}

#[test]
fn tick_countdown_no_room_id() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Countdown;
    // No current_room
    let mut shared = ExtensionMap::new();

    // Still returns true (countdown screen active, just no room to check)
    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

// =========================================================================
// Result screen snapshot tests
// =========================================================================

#[test]
fn snapshot_result_fallback() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Result;

    // Without context, result falls back to menu
    let snap = TetrominoBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    assert_eq!(snap["screen"], "menu");
}

#[test]
fn context_snapshot_result() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Result;
    state.start_game();
    state.multiplayer = true;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    lobby.start_match(room_id);
    lobby.mark_dead(room_id, ClientId(1));
    lobby.finish_match(room_id, ClientId(2));

    state.current_room = Some(room_id);

    // Set up opponent
    let mut opp_map = ExtensionMap::new();
    let opp = opp_map.get_or_insert::<TetrominoState>();
    opp.start_game();
    opp.multiplayer = true;
    opp.game.as_mut().unwrap().score = 500;

    let opponents = vec![(ClientId(2), &opp_map as &ExtensionMap)];
    let ctx = make_context(ClientId(1), &shared, &opponents);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["screen"], "result");
    assert_eq!(snap["winnerId"], 2);
    let players = snap["players"].as_array().unwrap();
    assert_eq!(players.len(), 2);
}

#[test]
fn context_snapshot_result_no_room_id() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Result;
    // No current_room
    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    assert!(TetrominoBridge.snapshot_with_context(&map, &ctx).is_none());
}

#[test]
fn context_snapshot_result_no_lobby() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Result;
    state.current_room = Some(RoomId(1));

    let shared = ExtensionMap::new();
    let ctx = make_context(ClientId(1), &shared, &[]);

    assert!(TetrominoBridge.snapshot_with_context(&map, &ctx).is_none());
}

#[test]
fn context_snapshot_result_no_winner() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Result;
    state.start_game();

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));

    state.current_room = Some(room_id);
    let ctx = make_context(ClientId(1), &shared, &[]);

    let snap = TetrominoBridge.snapshot_with_context(&map, &ctx).unwrap();
    assert_eq!(snap["screen"], "result");
    assert!(snap["winnerId"].is_null());
}

// =========================================================================
// Multiplayer death detection tick tests
// =========================================================================

#[test]
fn tick_multiplayer_death_marks_dead() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Game;
    state.multiplayer = true;
    state.client_id = Some(ClientId(1));
    state.start_game();
    state.game.as_mut().unwrap().game_over = true;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    lobby.start_match(room_id);

    let state = client.get_or_insert::<TetrominoState>();
    state.current_room = Some(room_id);

    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));

    // Player should be marked dead
    let lobby = shared.get::<TetrominoLobbyState>().unwrap();
    assert!(lobby.rooms[&room_id].dead_players.contains(&ClientId(1)));
    // With only 1 alive, match should be finished
    assert!(lobby.is_finished(room_id));
    assert_eq!(lobby.rooms[&room_id].winner, Some(ClientId(2)));
    // Screen should transition to Result
    let state = client.get::<TetrominoState>().unwrap();
    assert_eq!(state.screen, TetrominoScreen::Result);
}

#[test]
fn tick_multiplayer_death_no_finish_when_multiple_alive() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Game;
    state.multiplayer = true;
    state.client_id = Some(ClientId(1));
    state.start_game();
    state.game.as_mut().unwrap().game_over = true;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    lobby.join_room(room_id, ClientId(3));
    lobby.start_match(room_id);

    let state = client.get_or_insert::<TetrominoState>();
    state.current_room = Some(room_id);

    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));

    let lobby = shared.get::<TetrominoLobbyState>().unwrap();
    assert!(lobby.rooms[&room_id].dead_players.contains(&ClientId(1)));
    // 2 alive still — not finished
    assert!(!lobby.is_finished(room_id));
    assert_eq!(lobby.alive_count(room_id), 2);
}

#[test]
fn tick_single_player_game_over_no_death_detection() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Game;
    state.multiplayer = false;
    state.start_game();
    state.game.as_mut().unwrap().game_over = true;
    let mut shared = ExtensionMap::new();

    // Single player game over — tick returns false, no death detection
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
    let state = client.get::<TetrominoState>().unwrap();
    assert_eq!(state.screen, TetrominoScreen::Game);
}

// =========================================================================
// Room-screen catch-up tick tests (#544)
// =========================================================================

#[test]
fn tick_room_screen_detects_countdown() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Room;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    lobby.start_countdown(room_id);

    let state = client.get_or_insert::<TetrominoState>();
    state.current_room = Some(room_id);

    // Room screen + lobby Countdown -> screen becomes Countdown
    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));
    let state = client.get::<TetrominoState>().unwrap();
    assert_eq!(state.screen, TetrominoScreen::Countdown);
}

#[test]
fn tick_room_screen_detects_in_progress() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Room;

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let room_id = lobby.create_room();
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    lobby.start_match(room_id);

    let state = client.get_or_insert::<TetrominoState>();
    state.current_room = Some(room_id);

    // Room screen + lobby InProgress -> screen becomes Game, game started
    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));
    let state = client.get::<TetrominoState>().unwrap();
    assert_eq!(state.screen, TetrominoScreen::Game);
    assert!(state.multiplayer);
    assert!(state.game.is_some());
}

#[test]
fn tick_room_screen_no_room_id() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Room;
    // No current_room
    let mut shared = ExtensionMap::new();

    // Should not panic, just skip catch-up
    assert!(!TetrominoBridge.tick(&mut client, &mut shared, &services()));
}

#[test]
fn tick_countdown_screen_catches_up_when_already_in_progress() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Countdown; // tick caught up from Room
    let room_id = RoomId(1);
    state.current_room = Some(room_id);

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let created_room = lobby.create_room();
    assert_eq!(created_room, room_id);
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    // Another client's tick already called start_match()
    lobby.start_countdown(room_id);
    lobby.start_match(room_id);

    // Countdown screen + lobby InProgress -> game starts
    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));
    let state = client.get::<TetrominoState>().unwrap();
    assert_eq!(state.screen, TetrominoScreen::Game);
    assert!(state.multiplayer);
    assert!(state.game.is_some());
}

#[test]
fn tick_surviving_player_transitions_to_result_when_match_finished() {
    let mut client = ExtensionMap::new();
    let state = client.get_or_insert::<TetrominoState>();
    state.active = true;
    state.screen = TetrominoScreen::Game;
    state.multiplayer = true;
    state.client_id = Some(ClientId(2));
    let room_id = RoomId(1);
    state.current_room = Some(room_id);
    state.start_game();

    let mut shared = ExtensionMap::new();
    let lobby = shared.get_or_insert::<TetrominoLobbyState>();
    let created_room = lobby.create_room();
    assert_eq!(created_room, room_id);
    lobby.join_room(room_id, ClientId(1));
    lobby.join_room(room_id, ClientId(2));
    // Player 1 already dead, match finished with player 2 as winner
    lobby.mark_dead(room_id, ClientId(1));
    lobby.finish_match(room_id, ClientId(2));

    // Surviving player's tick detects match finished → Result screen
    assert!(TetrominoBridge.tick(&mut client, &mut shared, &services()));
    let state = client.get::<TetrominoState>().unwrap();
    assert_eq!(state.screen, TetrominoScreen::Result);
}
