//! Per-client indent guide state.
//!
//! `IndentGuideState` is a `SessionExtension` that stores computed indent
//! guide data for the client's active buffer.

use reovim_driver_text_session::SessionExtension;

/// A single indent guide line at a specific column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentGuide {
    /// The line number (0-indexed).
    pub line: u32,
    /// The indent level (0-indexed, in units of `tab_size`).
    pub level: u32,
    /// Whether this guide is active (matches cursor indent depth).
    pub active: bool,
}

impl IndentGuide {
    /// Create a new indent guide.
    #[must_use]
    pub const fn new(line: u32, level: u32, active: bool) -> Self {
        Self {
            line,
            level,
            active,
        }
    }
}

/// User-configurable indent guide options.
#[derive(Debug, Clone)]
pub struct IndentGuideOptions {
    /// Whether indent guides are enabled.
    pub enabled: bool,
    /// The character used for guide rendering.
    pub guide_char: char,
    /// Tab size fallback (when no language-specific config exists).
    pub tab_size: u32,
}

impl Default for IndentGuideOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            guide_char: '\u{2502}', // BOX DRAWINGS LIGHT VERTICAL
            tab_size: 4,
        }
    }
}

/// Per-client indent guide state stored as a session extension.
pub struct IndentGuideState {
    /// Computed guides for visible lines.
    pub guides: Vec<IndentGuide>,
    /// User options.
    pub options: IndentGuideOptions,
}

impl SessionExtension for IndentGuideState {
    fn create() -> Self {
        Self {
            guides: Vec::new(),
            options: IndentGuideOptions::default(),
        }
    }
}

impl IndentGuideState {
    /// Compute indent guides from line content.
    ///
    /// For each line, calculates the indent level based on leading whitespace
    /// and `tab_size`. The cursor's indent level is used to mark the active guide.
    pub fn compute_guides(
        &mut self,
        lines: &[&str],
        first_line: u32,
        tab_size: u32,
        cursor_indent_level: u32,
    ) {
        self.guides.clear();
        if !self.options.enabled || tab_size == 0 {
            return;
        }
        for (i, line) in lines.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let line_num = first_line + i as u32;
            let indent_cols = count_leading_whitespace(line, tab_size);
            let levels = indent_cols / tab_size;
            for level in 0..levels {
                let active = level == cursor_indent_level;
                self.guides.push(IndentGuide::new(line_num, level, active));
            }
        }
    }
}

/// Count leading whitespace columns, accounting for tab size.
fn count_leading_whitespace(line: &str, tab_size: u32) -> u32 {
    let mut cols = 0u32;
    for ch in line.chars() {
        match ch {
            ' ' => cols += 1,
            '\t' => {
                cols = cols.saturating_add(tab_size - (cols % tab_size));
            }
            _ => break,
        }
    }
    cols
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
