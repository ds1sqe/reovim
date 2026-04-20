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
    let names: Vec<std::borrow::Cow<'static, str>> = vec![
        START.name_owned(),
        QUIT.name_owned(),
        MOVE_LEFT.name_owned(),
        MOVE_RIGHT.name_owned(),
        ROTATE_CW.name_owned(),
        ROTATE_CCW.name_owned(),
        SOFT_DROP.name_owned(),
        HARD_DROP.name_owned(),
        PAUSE.name_owned(),
        TICK.name_owned(),
        RESTART.name_owned(),
        HOLD.name_owned(),
        OPEN_MENU.name_owned(),
        START_SINGLE.name_owned(),
        ENTER_LOBBY.name_owned(),
        CREATE_ROOM.name_owned(),
        READY_TOGGLE.name_owned(),
        LEAVE_ROOM.name_owned(),
        LEAVE_LOBBY.name_owned(),
        RETURN_LOBBY.name_owned(),
        JOIN_ROOM_BY_INDEX.name_owned(),
        START_MATCH.name_owned(),
        ENTER_RESULT.name_owned(),
    ];
    let mut deduped = names.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(names.len(), deduped.len());
}
