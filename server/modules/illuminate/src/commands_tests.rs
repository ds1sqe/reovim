use {super::*, reovim_driver_command::Command};

#[test]
fn test_next_reference_id() {
    let cmd = NextReferenceCommand;
    assert_eq!(cmd.id(), ids::NEXT_REFERENCE);
}

#[test]
fn test_next_reference_description() {
    let cmd = NextReferenceCommand;
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_prev_reference_id() {
    let cmd = PrevReferenceCommand;
    assert_eq!(cmd.id(), ids::PREV_REFERENCE);
}

#[test]
fn test_prev_reference_description() {
    let cmd = PrevReferenceCommand;
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_all_commands_count() {
    let cmds = all_commands();
    assert_eq!(cmds.len(), 2);
}

#[test]
fn test_all_commands_unique_ids() {
    let cmds = all_commands();
    assert_ne!(cmds[0].id(), cmds[1].id());
}
