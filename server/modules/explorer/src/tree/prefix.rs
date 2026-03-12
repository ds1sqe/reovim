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
#[path = "prefix_tests.rs"]
mod tests;
