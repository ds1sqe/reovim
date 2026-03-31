use super::*;

#[test]
fn bnext_identity() {
    let cmd = BnextCommand;
    assert_eq!(cmd.names(), &["bn", "bnext"]);
    assert_eq!(cmd.id().name(), "bnext");
    assert!(!cmd.description().is_empty());
}

#[test]
fn bprevious_identity() {
    let cmd = BpreviousCommand;
    assert_eq!(cmd.names(), &["bp", "bprevious", "bN", "bNext"]);
    assert_eq!(cmd.id().name(), "bprevious");
    assert!(!cmd.description().is_empty());
}

#[test]
fn bdelete_identity() {
    let cmd = BdeleteCommand;
    assert_eq!(cmd.names(), &["bd", "bdelete"]);
    assert_eq!(cmd.id().name(), "bdelete");
    assert!(!cmd.description().is_empty());
}

#[test]
fn qall_identity() {
    let cmd = QuitAllCommand;
    assert_eq!(cmd.names(), &["qa", "qall"]);
    assert_eq!(cmd.id().name(), "qall");
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 4);
}
