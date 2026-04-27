use {super::*, reovim_domain_text::Position, reovim_kernel::api::v1::BufferId};

fn test_buffer_id() -> BufferId {
    BufferId::new()
}

#[test]
fn test_mark_new() {
    let buf_id = test_buffer_id();
    let mark = Mark::new(Position::new(10, 5), buf_id);
    assert_eq!(mark.position, Position::new(10, 5));
    assert_eq!(mark.buffer_id, buf_id);
}

#[test]
fn test_local_marks() {
    let mut bank = MarkBank::new();

    assert!(bank.set_local('a', Position::new(10, 0)));
    assert_eq!(bank.get_local('a'), Some(Position::new(10, 0)));

    // Invalid mark name
    assert!(!bank.set_local('A', Position::new(0, 0)));
    assert!(bank.get_local('A').is_none());
}

#[test]
fn test_global_marks() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();

    assert!(bank.set_global('A', Mark::new(Position::new(5, 0), buf_id)));
    let mark = bank.get_global('A').unwrap();
    assert_eq!(mark.position, Position::new(5, 0));
    assert_eq!(mark.buffer_id, buf_id);

    // Invalid mark name
    assert!(!bank.set_global('a', Mark::new(Position::new(0, 0), buf_id)));
}

#[test]
fn test_special_marks() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();

    bank.set_special(SpecialMark::LastJump, Mark::new(Position::new(15, 3), buf_id));
    let mark = bank.get_special(SpecialMark::LastJump).unwrap();
    assert_eq!(mark.position, Position::new(15, 3));
}

#[test]
fn test_get_by_char() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();

    bank.set_local('x', Position::new(1, 0));
    bank.set_global('X', Mark::new(Position::new(2, 0), buf_id));
    bank.set_special(SpecialMark::LastJump, Mark::new(Position::new(3, 0), buf_id));

    // Local
    let result = bank.get_by_char('x').unwrap();
    assert_eq!(result.position(), Position::new(1, 0));
    assert!(result.buffer_id().is_none());

    // Global
    let result = bank.get_by_char('X').unwrap();
    assert_eq!(result.position(), Position::new(2, 0));
    assert!(result.buffer_id().is_some());

    // Special (last jump via ')
    let result = bank.get_by_char('\'').unwrap();
    assert_eq!(result.position(), Position::new(3, 0));
}

#[test]
fn test_list_marks() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();

    bank.set_local('c', Position::new(3, 0));
    bank.set_local('a', Position::new(1, 0));
    bank.set_local('b', Position::new(2, 0));

    let list = bank.list_local();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0], ('a', Position::new(1, 0)));
    assert_eq!(list[1], ('b', Position::new(2, 0)));
    assert_eq!(list[2], ('c', Position::new(3, 0)));

    bank.set_global('Z', Mark::new(Position::new(1, 0), buf_id));
    bank.set_global('A', Mark::new(Position::new(2, 0), buf_id));

    let list = bank.list_global();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].0, 'A');
    assert_eq!(list[1].0, 'Z');
}

#[test]
fn test_delete_marks() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();

    bank.set_local('a', Position::new(1, 0));
    assert!(bank.delete_local('a'));
    assert!(bank.get_local('a').is_none());
    assert!(!bank.delete_local('a')); // Already deleted

    bank.set_global('A', Mark::new(Position::new(1, 0), buf_id));
    assert!(bank.delete_global('A'));
    assert!(bank.get_global('A').is_none());
}

// === delete with invalid name ===

#[test]
fn test_delete_local_invalid_name() {
    let mut bank = MarkBank::new();
    assert!(!bank.delete_local('1'));
    assert!(!bank.delete_local('A'));
}

#[test]
fn test_delete_global_invalid_name() {
    let mut bank = MarkBank::new();
    assert!(!bank.delete_global('a'));
    assert!(!bank.delete_global('1'));
}

// === get_global invalid ===

#[test]
fn test_get_global_invalid_name() {
    let bank = MarkBank::new();
    assert!(bank.get_global('a').is_none());
    assert!(bank.get_global('1').is_none());
}

// === get_by_char special marks ===

#[test]
fn test_get_by_char_last_edit() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();
    bank.set_special(SpecialMark::LastEdit, Mark::new(Position::new(5, 3), buf_id));

    let result = bank.get_by_char('.').unwrap();
    assert_eq!(result.position(), Position::new(5, 3));
}

#[test]
fn test_get_by_char_last_insert() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();
    bank.set_special(SpecialMark::LastInsert, Mark::new(Position::new(7, 2), buf_id));

    let result = bank.get_by_char('^').unwrap();
    assert_eq!(result.position(), Position::new(7, 2));
}

#[test]
fn test_get_by_char_visual_start() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();
    bank.set_special(SpecialMark::VisualStart, Mark::new(Position::new(1, 0), buf_id));

    let result = bank.get_by_char('<').unwrap();
    assert_eq!(result.position(), Position::new(1, 0));
}

#[test]
fn test_get_by_char_visual_end() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();
    bank.set_special(SpecialMark::VisualEnd, Mark::new(Position::new(3, 10), buf_id));

    let result = bank.get_by_char('>').unwrap();
    assert_eq!(result.position(), Position::new(3, 10));
}

#[test]
fn test_get_by_char_backtick() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();
    bank.set_special(SpecialMark::LastJump, Mark::new(Position::new(9, 0), buf_id));

    let result = bank.get_by_char('`').unwrap();
    assert_eq!(result.position(), Position::new(9, 0));
}

#[test]
fn test_get_by_char_invalid() {
    let bank = MarkBank::new();
    assert!(bank.get_by_char('1').is_none());
    assert!(bank.get_by_char('!').is_none());
}

#[test]
fn test_get_by_char_missing_special() {
    let bank = MarkBank::new();
    // Special marks not set should return None
    assert!(bank.get_by_char('.').is_none());
    assert!(bank.get_by_char('^').is_none());
    assert!(bank.get_by_char('<').is_none());
    assert!(bank.get_by_char('>').is_none());
}

// === clear operations ===

#[test]
fn test_clear_local() {
    let mut bank = MarkBank::new();
    bank.set_local('a', Position::new(1, 0));
    bank.set_local('b', Position::new(2, 0));
    bank.clear_local();
    assert!(bank.get_local('a').is_none());
    assert!(bank.get_local('b').is_none());
    assert!(bank.list_local().is_empty());
}

#[test]
fn test_clear_all() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();
    bank.set_local('a', Position::new(1, 0));
    bank.set_global('A', Mark::new(Position::new(2, 0), buf_id));
    bank.set_special(SpecialMark::LastJump, Mark::new(Position::new(3, 0), buf_id));

    bank.clear_all();
    assert!(bank.get_local('a').is_none());
    assert!(bank.get_global('A').is_none());
    assert!(bank.get_special(SpecialMark::LastJump).is_none());
}

#[test]
fn test_clear_special() {
    let mut bank = MarkBank::new();
    let buf_id = test_buffer_id();
    bank.set_special(SpecialMark::LastEdit, Mark::new(Position::new(1, 0), buf_id));
    assert!(bank.get_special(SpecialMark::LastEdit).is_some());

    bank.clear_special(SpecialMark::LastEdit);
    assert!(bank.get_special(SpecialMark::LastEdit).is_none());
}

// === MarkResult buffer_id ===

#[test]
fn test_mark_result_local_buffer_id() {
    let result = MarkResult::Local(Position::new(0, 0));
    assert!(result.buffer_id().is_none());
}

#[test]
fn test_mark_result_global_buffer_id() {
    let buf_id = test_buffer_id();
    let mark = Mark::new(Position::new(0, 0), buf_id);
    let result = MarkResult::Global(&mark);
    assert_eq!(result.buffer_id(), Some(buf_id));
}
