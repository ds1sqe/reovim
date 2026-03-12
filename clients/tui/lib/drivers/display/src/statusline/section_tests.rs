use super::*;

#[test]
fn test_section_id_position() {
    assert_eq!(SectionId::A.position(), SectionPosition::Left);
    assert_eq!(SectionId::B.position(), SectionPosition::Left);
    assert_eq!(SectionId::C.position(), SectionPosition::Left);
    assert_eq!(SectionId::X.position(), SectionPosition::Right);
    assert_eq!(SectionId::Y.position(), SectionPosition::Right);
    assert_eq!(SectionId::Z.position(), SectionPosition::Right);
}

#[test]
fn test_section_new() {
    let section = Section::new(SectionId::A, " NORMAL ", Style::default());
    assert_eq!(section.id, SectionId::A);
    assert_eq!(section.text, " NORMAL ");
    assert_eq!(section.display_width(), 8);
}

#[test]
fn test_section_empty() {
    let section = Section::empty(SectionId::B);
    assert!(section.is_empty());
    assert_eq!(section.display_width(), 0);
}

#[test]
fn test_section_all_order() {
    assert_eq!(
        SectionId::ALL,
        &[
            SectionId::A,
            SectionId::B,
            SectionId::C,
            SectionId::X,
            SectionId::Y,
            SectionId::Z
        ]
    );
}

#[test]
fn test_section_priority_builder() {
    let section = Section::new(SectionId::A, "test", Style::default()).priority(200);
    assert_eq!(section.priority, 200);
    assert_eq!(section.text, "test");
    assert_eq!(section.id, SectionId::A);
}

#[test]
fn test_section_with_priority_constructor() {
    let section = Section::with_priority(SectionId::B, "hello", Style::default(), 42);
    assert_eq!(section.priority, 42);
    assert_eq!(section.text, "hello");
    assert_eq!(section.id, SectionId::B);
}

#[test]
fn test_section_position() {
    let left = Section::new(SectionId::A, "left", Style::default());
    assert_eq!(left.position(), SectionPosition::Left);

    let right = Section::new(SectionId::Z, "right", Style::default());
    assert_eq!(right.position(), SectionPosition::Right);
}

#[test]
fn test_section_is_empty() {
    let empty = Section::new(SectionId::A, "", Style::default());
    assert!(empty.is_empty());

    let not_empty = Section::new(SectionId::A, "x", Style::default());
    assert!(!not_empty.is_empty());
}
