use super::*;

#[test]
fn test_pure_addition() {
    let hunk = DiffHunk {
        old_start: 10,
        old_count: 0,
        new_start: 10,
        new_count: 3,
    };
    let signs = hunk_to_signs(&hunk);
    assert_eq!(signs.len(), 3);
    assert_eq!(
        signs[0],
        LineSigns {
            line: 9,
            kind: SignKind::Add
        }
    );
    assert_eq!(
        signs[1],
        LineSigns {
            line: 10,
            kind: SignKind::Add
        }
    );
    assert_eq!(
        signs[2],
        LineSigns {
            line: 11,
            kind: SignKind::Add
        }
    );
}

#[test]
fn test_pure_deletion() {
    let hunk = DiffHunk {
        old_start: 5,
        old_count: 2,
        new_start: 5,
        new_count: 0,
    };
    let signs = hunk_to_signs(&hunk);
    assert_eq!(signs.len(), 1);
    assert_eq!(
        signs[0],
        LineSigns {
            line: 4,
            kind: SignKind::Delete
        }
    );
}

#[test]
fn test_deletion_at_start() {
    let hunk = DiffHunk {
        old_start: 1,
        old_count: 3,
        new_start: 0,
        new_count: 0,
    };
    let signs = hunk_to_signs(&hunk);
    assert_eq!(signs.len(), 1);
    assert_eq!(
        signs[0],
        LineSigns {
            line: 0,
            kind: SignKind::Delete
        }
    );
}

#[test]
fn test_change() {
    let hunk = DiffHunk {
        old_start: 3,
        old_count: 2,
        new_start: 3,
        new_count: 4,
    };
    let signs = hunk_to_signs(&hunk);
    assert_eq!(signs.len(), 4);
    for (i, sign) in signs.iter().enumerate() {
        assert_eq!(sign.line, 2 + i);
        assert_eq!(sign.kind, SignKind::Change);
    }
}

#[test]
fn test_both_zero() {
    let hunk = DiffHunk {
        old_start: 1,
        old_count: 0,
        new_start: 1,
        new_count: 0,
    };
    let signs = hunk_to_signs(&hunk);
    assert!(signs.is_empty());
}

#[test]
fn test_single_line_addition() {
    let hunk = DiffHunk {
        old_start: 1,
        old_count: 0,
        new_start: 1,
        new_count: 1,
    };
    let signs = hunk_to_signs(&hunk);
    assert_eq!(signs.len(), 1);
    assert_eq!(
        signs[0],
        LineSigns {
            line: 0,
            kind: SignKind::Add
        }
    );
}

#[test]
fn test_single_line_change() {
    let hunk = DiffHunk {
        old_start: 5,
        old_count: 1,
        new_start: 5,
        new_count: 1,
    };
    let signs = hunk_to_signs(&hunk);
    assert_eq!(signs.len(), 1);
    assert_eq!(
        signs[0],
        LineSigns {
            line: 4,
            kind: SignKind::Change
        }
    );
}

#[test]
fn test_sign_kind_debug() {
    assert_eq!(format!("{:?}", SignKind::Add), "Add");
    assert_eq!(format!("{:?}", SignKind::Change), "Change");
    assert_eq!(format!("{:?}", SignKind::Delete), "Delete");
}

#[test]
fn test_sign_kind_clone() {
    let kind = SignKind::Add;
    let cloned = kind;
    assert_eq!(kind, cloned);
}

#[test]
fn test_line_signs_debug() {
    let sign = LineSigns {
        line: 5,
        kind: SignKind::Add,
    };
    let debug = format!("{sign:?}");
    assert!(debug.contains("LineSigns"));
}

#[test]
fn test_line_signs_clone() {
    let sign = LineSigns {
        line: 3,
        kind: SignKind::Change,
    };
    let cloned = sign;
    assert_eq!(sign, cloned);
}
