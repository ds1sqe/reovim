//! Blame line formatting.

use reovim_subsys_git::types::BlameEntry;

/// Format a blame entry for display in the gutter.
///
/// Format: `{short_hash} {author} {summary}`
/// Truncates to `max_width` if specified.
pub fn format_blame(entry: &BlameEntry, max_width: Option<usize>) -> String {
    let formatted = format!("{} {} {}", entry.short_hash, entry.author, entry.summary);

    match max_width {
        Some(max) if formatted.len() > max && max > 3 => {
            format!("{}...", &formatted[..max - 3])
        }
        _ => formatted,
    }
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
