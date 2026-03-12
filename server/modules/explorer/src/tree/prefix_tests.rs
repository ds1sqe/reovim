use super::*;

#[test]
fn test_root_has_no_prefix() {
    let info = TreeLineInfo {
        vertical_lines: vec![],
        is_last_child: false,
    };
    assert_eq!(build_tree_prefix(&info, 0), "");
}

#[test]
fn test_depth_1_not_last() {
    let info = TreeLineInfo {
        vertical_lines: vec![],
        is_last_child: false,
    };
    assert_eq!(build_tree_prefix(&info, 1), "├─");
}

#[test]
fn test_depth_1_last() {
    let info = TreeLineInfo {
        vertical_lines: vec![],
        is_last_child: true,
    };
    assert_eq!(build_tree_prefix(&info, 1), "└─");
}

#[test]
fn test_depth_2_parent_not_last_not_last() {
    let info = TreeLineInfo {
        vertical_lines: vec![true],
        is_last_child: false,
    };
    assert_eq!(build_tree_prefix(&info, 2), "│ ├─");
}

#[test]
fn test_depth_2_parent_not_last_last() {
    let info = TreeLineInfo {
        vertical_lines: vec![true],
        is_last_child: true,
    };
    assert_eq!(build_tree_prefix(&info, 2), "│ └─");
}

#[test]
fn test_depth_2_parent_last_not_last() {
    let info = TreeLineInfo {
        vertical_lines: vec![false],
        is_last_child: false,
    };
    assert_eq!(build_tree_prefix(&info, 2), "  ├─");
}

#[test]
fn test_depth_2_parent_last_last() {
    let info = TreeLineInfo {
        vertical_lines: vec![false],
        is_last_child: true,
    };
    assert_eq!(build_tree_prefix(&info, 2), "  └─");
}

#[test]
fn test_depth_3_complex() {
    let info = TreeLineInfo {
        vertical_lines: vec![true, false],
        is_last_child: false,
    };
    assert_eq!(build_tree_prefix(&info, 3), "│   ├─");
}

#[test]
fn test_depth_3_all_continuation() {
    let info = TreeLineInfo {
        vertical_lines: vec![true, true],
        is_last_child: true,
    };
    assert_eq!(build_tree_prefix(&info, 3), "│ │ └─");
}

#[test]
fn test_depth_3_no_continuation() {
    let info = TreeLineInfo {
        vertical_lines: vec![false, false],
        is_last_child: true,
    };
    assert_eq!(build_tree_prefix(&info, 3), "    └─");
}

#[test]
fn test_missing_vertical_lines_graceful() {
    // Handles case where vertical_lines is shorter than expected
    let info = TreeLineInfo {
        vertical_lines: vec![],
        is_last_child: false,
    };
    // Depth 2 but no vertical_lines data — should use fallback spaces
    assert_eq!(build_tree_prefix(&info, 2), "  ├─");
}
