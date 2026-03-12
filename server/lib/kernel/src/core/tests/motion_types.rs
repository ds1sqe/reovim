use super::*;

#[test]
fn test_motion_is_linewise() {
    assert!(Motion::Line(Direction::Forward).is_linewise());
    assert!(Motion::Line(Direction::Backward).is_linewise());
    assert!(Motion::JumpLine(None).is_linewise());
    assert!(Motion::Paragraph(Direction::Forward).is_linewise());

    assert!(!Motion::Char(Direction::Forward).is_linewise());
    assert!(
        !Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false
        }
        .is_linewise()
    );
}

#[test]
fn test_motion_is_inclusive() {
    assert!(Motion::LinePosition(LinePosition::End).is_inclusive());
    assert!(
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true
        }
        .is_inclusive()
    );
    assert!(Motion::MatchBracket.is_inclusive());

    assert!(
        !Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false
        }
        .is_inclusive()
    );
    assert!(!Motion::Char(Direction::Forward).is_inclusive());
}
