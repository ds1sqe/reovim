use super::*;

#[test]
fn module_id() {
    assert_eq!(MODULE.as_str(), "tetromino");
}

#[test]
fn command_ids_have_correct_module() {
    let commands = [
        START,
        QUIT,
        MOVE_LEFT,
        MOVE_RIGHT,
        ROTATE_CW,
        ROTATE_CCW,
        SOFT_DROP,
        HARD_DROP,
        PAUSE,
        TICK,
        RESTART,
        HOLD,
        OPEN_MENU,
        START_SINGLE,
        ENTER_LOBBY,
        CREATE_ROOM,
        READY_TOGGLE,
        LEAVE_ROOM,
        LEAVE_LOBBY,
        RETURN_LOBBY,
        JOIN_ROOM_BY_INDEX,
        START_MATCH,
        ENTER_RESULT,
    ];
    for cmd in &commands {
        assert_eq!(cmd.module(), &MODULE);
    }
}

#[test]
fn command_ids_are_unique() {
    let names: Vec<&str> = vec![
        START.name(),
        QUIT.name(),
        MOVE_LEFT.name(),
        MOVE_RIGHT.name(),
        ROTATE_CW.name(),
        ROTATE_CCW.name(),
        SOFT_DROP.name(),
        HARD_DROP.name(),
        PAUSE.name(),
        TICK.name(),
        RESTART.name(),
        HOLD.name(),
        OPEN_MENU.name(),
        START_SINGLE.name(),
        ENTER_LOBBY.name(),
        CREATE_ROOM.name(),
        READY_TOGGLE.name(),
        LEAVE_ROOM.name(),
        LEAVE_LOBBY.name(),
        RETURN_LOBBY.name(),
        JOIN_ROOM_BY_INDEX.name(),
        START_MATCH.name(),
        ENTER_RESULT.name(),
    ];
    let mut deduped = names.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(names.len(), deduped.len());
}
