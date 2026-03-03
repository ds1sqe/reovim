//! Tree structure rendering with box-drawing characters.
//!
//! Port of `archive/pre_kernel/plugins/features/explorer/src/tree_render.rs`.
//! Provides visual hierarchy for file trees using Unicode box-drawing chars.

/// Information needed to render tree connection lines for a node.
#[derive(Debug, Clone)]
pub struct TreeLineInfo {
    /// For each depth level (0..node.depth), whether there are more siblings below
    /// that need a vertical continuation line (`│`).
    pub vertical_lines: Vec<bool>,

    /// Whether this node is the last child of its parent.
    pub is_last_child: bool,
}

/// Build the tree prefix string with box-drawing characters.
///
/// # Examples
///
/// ```text
/// Depth 0:                             ""
/// Depth 1, not last:                   "├─"
/// Depth 1, last:                       "└─"
/// Depth 2, parent not last, not last:  "│ ├─"
/// Depth 2, parent not last, last:      "│ └─"
/// Depth 2, parent last, not last:      "  ├─"
/// ```
#[must_use]
pub fn build_tree_prefix(info: &TreeLineInfo, depth: usize) -> String {
    if depth == 0 {
        return String::new();
    }

    let mut prefix = String::new();

    // For each ancestor level (not including the node's own level)
    for level in 0..depth {
        if level == 0 {
            continue; // Skip root level
        }

        if let Some(&has_more_siblings) = info.vertical_lines.get(level - 1) {
            if has_more_siblings {
                prefix.push('\u{2502}'); // │
                prefix.push(' ');
            } else {
                prefix.push_str("  ");
            }
        } else {
            prefix.push_str("  ");
        }
    }

    // Add the connector for this node
    if info.is_last_child {
        prefix.push('\u{2514}'); // └
    } else {
        prefix.push('\u{251C}'); // ├
    }
    prefix.push('\u{2500}'); // ─

    prefix
}

#[cfg(test)]
mod tests {
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
}
