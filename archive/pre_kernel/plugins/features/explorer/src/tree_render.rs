//! Tree structure rendering with box-drawing characters
//!
//! Provides visual hierarchy for file trees using Unicode box-drawing chars.

/// Information needed to render tree connection lines for a node
#[derive(Debug, Clone)]
pub struct TreeLineInfo {
    /// For each depth level (0..node.depth), whether there are more siblings below
    /// that need a vertical continuation line (│)
    pub vertical_lines: Vec<bool>,

    /// Whether this node is the last child of its parent
    pub is_last_child: bool,
}

/// Build the tree prefix string with box-drawing characters
///
/// # Examples
///
/// ```text
/// Depth 1, not last: "├─"
/// Depth 1, last:     "└─"
/// Depth 2, parent not last, not last: "│ ├─"
/// Depth 2, parent not last, last:     "│ └─"
/// Depth 2, parent last, not last:     "  ├─"
/// ```
#[must_use]
pub fn build_tree_prefix(info: &TreeLineInfo, depth: usize) -> String {
    if depth == 0 {
        return String::new(); // Root has no prefix
    }

    let mut prefix = String::new();

    // For each ancestor level (not including the node's own level)
    for level in 0..depth {
        if level == 0 {
            continue; // Skip root level
        }

        // Check if this ancestor level needs a vertical line
        if let Some(&has_more_siblings) = info.vertical_lines.get(level - 1) {
            if has_more_siblings {
                prefix.push('│');
                prefix.push(' ');
            } else {
                prefix.push_str("  ");
            }
        } else {
            // Shouldn't happen, but handle gracefully
            prefix.push_str("  ");
        }
    }

    // Add the connector for this node
    if info.is_last_child {
        prefix.push('└');
    } else {
        prefix.push('├');
    }
    prefix.push('─');

    prefix
}

/// Build a simple tree prefix with ASCII characters
#[must_use]
pub fn build_simple_prefix(depth: usize) -> String {
    if depth == 0 {
        String::new()
    } else {
        format!("{}> ", "  ".repeat(depth.saturating_sub(1)))
    }
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
        let prefix = build_tree_prefix(&info, 0);
        assert_eq!(prefix, "");
    }

    #[test]
    fn test_depth_1_not_last() {
        let info = TreeLineInfo {
            vertical_lines: vec![],
            is_last_child: false,
        };
        let prefix = build_tree_prefix(&info, 1);
        assert_eq!(prefix, "├─");
    }

    #[test]
    fn test_depth_1_last() {
        let info = TreeLineInfo {
            vertical_lines: vec![],
            is_last_child: true,
        };
        let prefix = build_tree_prefix(&info, 1);
        assert_eq!(prefix, "└─");
    }

    #[test]
    fn test_depth_2_parent_not_last_not_last() {
        let info = TreeLineInfo {
            vertical_lines: vec![true], // Parent has more siblings
            is_last_child: false,
        };
        let prefix = build_tree_prefix(&info, 2);
        assert_eq!(prefix, "│ ├─");
    }

    #[test]
    fn test_depth_2_parent_not_last_last() {
        let info = TreeLineInfo {
            vertical_lines: vec![true], // Parent has more siblings
            is_last_child: true,
        };
        let prefix = build_tree_prefix(&info, 2);
        assert_eq!(prefix, "│ └─");
    }

    #[test]
    fn test_depth_2_parent_last_not_last() {
        let info = TreeLineInfo {
            vertical_lines: vec![false], // Parent is last child
            is_last_child: false,
        };
        let prefix = build_tree_prefix(&info, 2);
        assert_eq!(prefix, "  ├─");
    }

    #[test]
    fn test_depth_2_parent_last_last() {
        let info = TreeLineInfo {
            vertical_lines: vec![false], // Parent is last child
            is_last_child: true,
        };
        let prefix = build_tree_prefix(&info, 2);
        assert_eq!(prefix, "  └─");
    }

    #[test]
    fn test_depth_3_complex() {
        let info = TreeLineInfo {
            vertical_lines: vec![true, false], // Grandparent has siblings, parent doesn't
            is_last_child: false,
        };
        let prefix = build_tree_prefix(&info, 3);
        assert_eq!(prefix, "│   ├─");
    }

    #[test]
    fn test_simple_prefix_depth_0() {
        let prefix = build_simple_prefix(0);
        assert_eq!(prefix, "");
    }

    #[test]
    fn test_simple_prefix_depth_1() {
        let prefix = build_simple_prefix(1);
        assert_eq!(prefix, "> ");
    }

    #[test]
    fn test_simple_prefix_depth_2() {
        let prefix = build_simple_prefix(2);
        assert_eq!(prefix, "  > ");
    }

    #[test]
    fn test_simple_prefix_depth_3() {
        let prefix = build_simple_prefix(3);
        assert_eq!(prefix, "    > ");
    }
}
