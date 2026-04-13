//! `DiffHunk` to sign kind conversion.

use reovim_subsys_git::types::DiffHunk;

/// The kind of gutter sign derived from a diff hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignKind {
    /// Lines were added.
    Add,
    /// Lines were changed (modified).
    Change,
    /// Lines were deleted.
    Delete,
}

/// A resolved sign for a specific line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSigns {
    /// The 0-indexed line number.
    pub line: usize,
    /// The kind of sign.
    pub kind: SignKind,
}

/// Convert a `DiffHunk` into line-level signs.
///
/// Mapping rules:
/// - `old_count == 0 && new_count > 0`: Addition (lines `new_start..new_start+new_count`)
/// - `new_count == 0 && old_count > 0`: Deletion (single marker at `new_start`, or `new_start-1` if `new_start > 0`)
/// - `old_count > 0 && new_count > 0`: Change (lines `new_start..new_start+new_count`)
///
/// Line numbers are converted from 1-indexed (git output) to 0-indexed.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn hunk_to_signs(hunk: &DiffHunk) -> Vec<LineSigns> {
    // Convert from 1-indexed to 0-indexed
    let start = hunk.new_start.saturating_sub(1);

    if hunk.old_count == 0 && hunk.new_count > 0 {
        // Pure addition
        (0..hunk.new_count)
            .map(|i| LineSigns {
                line: start + i,
                kind: SignKind::Add,
            })
            .collect()
    } else if hunk.new_count == 0 && hunk.old_count > 0 {
        // Pure deletion — single marker
        let marker_line = if hunk.new_start > 0 { start } else { 0 };
        vec![LineSigns {
            line: marker_line,
            kind: SignKind::Delete,
        }]
    } else if hunk.old_count > 0 && hunk.new_count > 0 {
        // Change
        (0..hunk.new_count)
            .map(|i| LineSigns {
                line: start + i,
                kind: SignKind::Change,
            })
            .collect()
    } else {
        // Both zero — no-op hunk
        vec![]
    }
}

#[cfg(test)]
#[path = "hunk_tests.rs"]
mod tests;
