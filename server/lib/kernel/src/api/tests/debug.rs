use {
    super::*,
    crate::{
        api::module::ModuleId,
        core::{Mark, ModeId},
    },
    reovim_types_text::{Position, RegisterBank, RegisterContent, YankType},
};

#[test]
fn test_yank_type_snapshot_from() {
    assert_eq!(YankTypeSnapshot::from(YankType::Characterwise), YankTypeSnapshot::Characterwise);
    assert_eq!(YankTypeSnapshot::from(YankType::Linewise), YankTypeSnapshot::Linewise);
}

#[test]
fn test_snapshot_kernel_state() {
    let ctx = KernelContext::default();
    let snapshot = snapshot_kernel_state(&ctx);

    assert_eq!(snapshot.buffer_count, 0);
    assert!(snapshot.buffer_ids.is_empty());
    assert_eq!(snapshot.event_queue_len, 0);
}

#[test]
fn test_snapshot_registers_empty() {
    let bank = RegisterBank::new();
    let snapshot = snapshot_registers(&bank);

    assert_eq!(snapshot.unnamed.name, '"');
    assert!(snapshot.unnamed.text.is_empty());
    assert!(snapshot.named.is_empty());
}

#[test]
fn test_snapshot_registers_with_content() {
    let mut bank = RegisterBank::new();
    bank.set(RegisterContent::characterwise("hello"));
    bank.set_named('a', RegisterContent::linewise("world"));

    let snapshot = snapshot_registers(&bank);

    assert_eq!(snapshot.unnamed.text, "hello");
    assert_eq!(snapshot.unnamed.yank_type, YankTypeSnapshot::Characterwise);
    assert_eq!(snapshot.named.len(), 1);
    assert_eq!(snapshot.named[0].name, 'a');
    assert_eq!(snapshot.named[0].text, "world");
    assert_eq!(snapshot.named[0].yank_type, YankTypeSnapshot::Linewise);
}

#[test]
fn test_mark_bank_empty() {
    let bank = MarkBank::new();

    assert!(bank.list_local().is_empty());
    assert!(bank.list_global().is_empty());
    assert!(bank.get_special(SpecialMark::LastJump).is_none());
    assert!(bank.get_special(SpecialMark::LastEdit).is_none());
    assert!(bank.get_special(SpecialMark::LastInsert).is_none());
    assert!(bank.get_special(SpecialMark::VisualStart).is_none());
    assert!(bank.get_special(SpecialMark::VisualEnd).is_none());
    assert!(bank.get_special(SpecialMark::LastExitInsert).is_none());
}

#[test]
fn test_mark_bank_with_content() {
    let mut bank = MarkBank::new();
    let buffer_id = BufferId::new();

    bank.set_local('a', Position::new(10, 5));
    bank.set_global('A', Mark::new(Position::new(20, 0), buffer_id));
    bank.set_special(SpecialMark::LastJump, Mark::new(Position::new(5, 3), buffer_id));

    let local = bank.list_local();
    assert_eq!(local.len(), 1);
    assert_eq!(local[0], ('a', Position::new(10, 5)));

    let global = bank.list_global();
    assert_eq!(global.len(), 1);
    assert_eq!(global[0].0, 'A');
    assert_eq!(global[0].1.position, Position::new(20, 0));
    assert_eq!(global[0].1.buffer_id, buffer_id);

    let mark = bank
        .get_special(SpecialMark::LastJump)
        .expect("last jump mark");
    assert_eq!(mark.position, Position::new(5, 3));
    assert_eq!(mark.buffer_id, buffer_id);
}

#[test]
fn test_snapshot_mode_stack() {
    let mode_id = ModeId::new(ModuleId::new("editor"), "normal");
    let stack = ModeStack::new(mode_id);
    let snapshot = snapshot_mode_stack(&stack);

    assert_eq!(snapshot.current, "editor:normal");
    assert_eq!(snapshot.depth, 1);
    assert_eq!(snapshot.stack.len(), 1);
}
