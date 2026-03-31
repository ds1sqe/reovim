use {super::*, reovim_driver_command::Command};

#[test]
fn open_menu_metadata() {
    let cmd = OpenMenu;
    assert_eq!(cmd.id(), ids::OPEN_MENU);
    assert!(!cmd.description().is_empty());
}

#[test]
fn start_metadata() {
    let cmd = Start;
    assert_eq!(cmd.id(), ids::START);
    assert!(!cmd.description().is_empty());
}

#[test]
fn start_single_metadata() {
    let cmd = StartSingle;
    assert_eq!(cmd.id(), ids::START_SINGLE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn quit_metadata() {
    let cmd = Quit;
    assert_eq!(cmd.id(), ids::QUIT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn restart_metadata() {
    let cmd = Restart;
    assert_eq!(cmd.id(), ids::RESTART);
    assert!(!cmd.description().is_empty());
}

#[test]
fn enter_lobby_metadata() {
    let cmd = EnterLobby;
    assert_eq!(cmd.id(), ids::ENTER_LOBBY);
    assert!(!cmd.description().is_empty());
}

#[test]
fn create_room_metadata() {
    let cmd = CreateRoom;
    assert_eq!(cmd.id(), ids::CREATE_ROOM);
    assert!(!cmd.description().is_empty());
}

#[test]
fn ready_toggle_metadata() {
    let cmd = ReadyToggle;
    assert_eq!(cmd.id(), ids::READY_TOGGLE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn leave_room_metadata() {
    let cmd = LeaveRoom;
    assert_eq!(cmd.id(), ids::LEAVE_ROOM);
    assert!(!cmd.description().is_empty());
}

#[test]
fn leave_lobby_metadata() {
    let cmd = LeaveLobby;
    assert_eq!(cmd.id(), ids::LEAVE_LOBBY);
    assert!(!cmd.description().is_empty());
}

#[test]
fn move_left_metadata() {
    let cmd = MoveLeft;
    assert_eq!(cmd.id(), ids::MOVE_LEFT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn move_right_metadata() {
    let cmd = MoveRight;
    assert_eq!(cmd.id(), ids::MOVE_RIGHT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn rotate_cw_metadata() {
    let cmd = RotateCw;
    assert_eq!(cmd.id(), ids::ROTATE_CW);
    assert!(!cmd.description().is_empty());
}

#[test]
fn rotate_ccw_metadata() {
    let cmd = RotateCcw;
    assert_eq!(cmd.id(), ids::ROTATE_CCW);
    assert!(!cmd.description().is_empty());
}

#[test]
fn soft_drop_metadata() {
    let cmd = SoftDrop;
    assert_eq!(cmd.id(), ids::SOFT_DROP);
    assert!(!cmd.description().is_empty());
}

#[test]
fn hard_drop_metadata() {
    let cmd = HardDrop;
    assert_eq!(cmd.id(), ids::HARD_DROP);
    assert!(!cmd.description().is_empty());
}

#[test]
fn hold_metadata() {
    let cmd = Hold;
    assert_eq!(cmd.id(), ids::HOLD);
    assert!(!cmd.description().is_empty());
}

#[test]
fn pause_metadata() {
    let cmd = Pause;
    assert_eq!(cmd.id(), ids::PAUSE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn tick_metadata() {
    let cmd = Tick;
    assert_eq!(cmd.id(), ids::TICK);
    assert!(!cmd.description().is_empty());
}

#[test]
fn join_room_by_index_metadata() {
    let cmd = JoinRoomByIndex;
    assert_eq!(cmd.id(), ids::JOIN_ROOM_BY_INDEX);
    assert!(!cmd.description().is_empty());
}

#[test]
fn start_match_metadata() {
    let cmd = StartMatch;
    assert_eq!(cmd.id(), ids::START_MATCH);
    assert!(!cmd.description().is_empty());
}

#[test]
fn enter_result_metadata() {
    let cmd = EnterResult;
    assert_eq!(cmd.id(), ids::ENTER_RESULT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 23);
}

#[test]
fn command_handlers_unique_ids() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    let mut deduped = ids.clone();
    deduped.sort_by_key(CommandId::name_owned);
    deduped.dedup_by_key(|id| id.name_owned());
    assert_eq!(ids.len(), deduped.len());
}

#[test]
fn all_debug() {
    let debug = format!("{OpenMenu:?}");
    assert!(debug.contains("OpenMenu"));
    let debug = format!("{Start:?}");
    assert!(debug.contains("Start"));
    let debug = format!("{StartSingle:?}");
    assert!(debug.contains("StartSingle"));
    let debug = format!("{Quit:?}");
    assert!(debug.contains("Quit"));
    let debug = format!("{MoveLeft:?}");
    assert!(debug.contains("MoveLeft"));
    let debug = format!("{EnterLobby:?}");
    assert!(debug.contains("EnterLobby"));
    let debug = format!("{CreateRoom:?}");
    assert!(debug.contains("CreateRoom"));
    let debug = format!("{ReadyToggle:?}");
    assert!(debug.contains("ReadyToggle"));
    let debug = format!("{LeaveRoom:?}");
    assert!(debug.contains("LeaveRoom"));
    let debug = format!("{LeaveLobby:?}");
    assert!(debug.contains("LeaveLobby"));
    let debug = format!("{JoinRoomByIndex:?}");
    assert!(debug.contains("JoinRoomByIndex"));
    let debug = format!("{StartMatch:?}");
    assert!(debug.contains("StartMatch"));
    let debug = format!("{EnterResult:?}");
    assert!(debug.contains("EnterResult"));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn all_default_constructable() {
    let _ = OpenMenu::default();
    let _ = Start::default();
    let _ = StartSingle::default();
    let _ = Quit::default();
    let _ = Restart::default();
    let _ = EnterLobby::default();
    let _ = CreateRoom::default();
    let _ = ReadyToggle::default();
    let _ = LeaveRoom::default();
    let _ = LeaveLobby::default();
    let _ = MoveLeft::default();
    let _ = MoveRight::default();
    let _ = RotateCw::default();
    let _ = RotateCcw::default();
    let _ = SoftDrop::default();
    let _ = HardDrop::default();
    let _ = Hold::default();
    let _ = Pause::default();
    let _ = Tick::default();
    let _ = JoinRoomByIndex::default();
    let _ = StartMatch::default();
    let _ = EnterResult::default();
}
