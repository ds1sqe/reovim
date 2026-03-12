use {
    super::*,
    crate::{
        api::module::ModuleId,
        core::{Mark, ModeId, RegisterContent},
    },
};

#[test]
fn test_yank_type_snapshot_from() {
    assert_eq!(
        YankTypeSnapshot::from(YankType::Characterwise),
        YankTypeSnapshot::Characterwise
    );
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
fn test_snapshot_marks_empty() {
    let bank = MarkBank::new();
    let snapshot = snapshot_marks(&bank);

    assert!(snapshot.local.is_empty());
    assert!(snapshot.global.is_empty());
    assert!(snapshot.special.is_empty());
}

#[test]
fn test_snapshot_marks_with_content() {
    let mut bank = MarkBank::new();
    let buffer_id = BufferId::new();

    bank.set_local('a', Position::new(10, 5));
    bank.set_global('A', Mark::new(Position::new(20, 0), buffer_id));

    let snapshot = snapshot_marks(&bank);

    assert_eq!(snapshot.local.len(), 1);
    assert_eq!(snapshot.local[0].name, "a");
    assert_eq!(snapshot.local[0].position, Position::new(10, 5));

    assert_eq!(snapshot.global.len(), 1);
    assert_eq!(snapshot.global[0].name, "A");
    assert_eq!(snapshot.global[0].position, Position::new(20, 0));
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

#[test]
fn test_snapshot_marks_with_special_marks() {
    let mut bank = MarkBank::new();
    let buffer_id = BufferId::new();

    bank.set_special(SpecialMark::LastJump, Mark::new(Position::new(5, 3), buffer_id));
    bank.set_special(SpecialMark::LastEdit, Mark::new(Position::new(10, 0), buffer_id));

    let snapshot = snapshot_marks(&bank);

    // Special marks should be included
    assert!(snapshot.special.len() >= 2);
    assert!(snapshot.special.iter().any(|m| m.name == "'"));
    assert!(snapshot.special.iter().any(|m| m.name == "."));
}
