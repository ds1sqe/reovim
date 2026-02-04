//! Indentation guide support
//!
//! Provides indentation guide computation for visual display of indent levels.
//! No treesitter needed - pure whitespace analysis.

/// An indentation guide at a specific column
#[derive(Debug, Clone)]
pub struct IndentGuide {
    /// Column position of the guide (0-indexed)
    pub column: u32,
    /// Whether this is an "active" guide (contains cursor indent level)
    pub active: bool,
}

/// Analyzes indentation and generates indent guides
#[derive(Debug, Clone)]
pub struct IndentAnalyzer {
    /// Number of spaces per tab
    pub tab_size: u32,
    /// Character to use for indent guides
    pub guide_char: char,
    /// Whether indent guides are enabled
    pub enabled: bool,
}

impl Default for IndentAnalyzer {
    fn default() -> Self {
        Self {
            tab_size: 4,
            guide_char: '\u{2502}', // Box drawing: │
            enabled: false,
        }
    }
}

impl IndentAnalyzer {
    /// Create a new indent analyzer with the given tab size
    #[must_use]
    pub const fn new(tab_size: u32) -> Self {
        Self {
            tab_size,
            guide_char: '\u{2502}',
            enabled: false,
        }
    }

    /// Enable or disable indent guides
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if indent guides are enabled
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the indent level of a line (number of indent units)
    #[must_use]
    pub fn indent_level(&self, line: &str) -> u32 {
        if self.tab_size == 0 {
            return 0;
        }

        let whitespace_count = self.leading_whitespace_count(line);
        whitespace_count / self.tab_size
    }

    /// Count leading whitespace columns (tabs expand to `tab_size`)
    #[must_use]
    pub fn leading_whitespace_count(&self, line: &str) -> u32 {
        let mut count = 0u32;
        for ch in line.chars() {
            match ch {
                ' ' => count += 1,
                '\t' => count += self.tab_size,
                _ => break,
            }
        }
        count
    }

    /// Compute indent guides for a single line
    ///
    /// # Arguments
    /// * `line` - The line content
    /// * `cursor_indent` - Optional cursor indent level for highlighting active guide
    ///
    /// # Returns
    /// Vector of indent guides for this line
    #[must_use]
    pub fn guides_for_line(&self, line: &str, cursor_indent: Option<u32>) -> Vec<IndentGuide> {
        if !self.enabled || self.tab_size == 0 {
            return Vec::new();
        }

        let indent_level = self.indent_level(line);
        let mut guides = Vec::new();

        // Generate guides at each indent level
        for level in 1..=indent_level {
            let column = (level - 1) * self.tab_size;
            let active = cursor_indent.is_some_and(|ci| level == ci);
            guides.push(IndentGuide { column, active });
        }

        guides
    }

    /// Inject indent guide characters into a line's leading whitespace
    ///
    /// Returns a new string with guide characters at appropriate positions.
    ///
    /// # Arguments
    /// * `line` - The original line content
    /// * `guides` - The indent guides for this line
    /// * `guide_style_start` - ANSI escape sequence to start guide styling
    /// * `guide_style_end` - ANSI escape sequence to end guide styling
    #[must_use]
    pub fn inject_guides(
        &self,
        line: &str,
        guides: &[IndentGuide],
        guide_style_start: &str,
        guide_style_end: &str,
    ) -> String {
        if guides.is_empty() || !self.enabled {
            return line.to_string();
        }

        let mut result = String::new();
        let chars: Vec<char> = line.chars().collect();
        let mut col = 0u32;
        let mut guide_idx = 0;

        // Process leading whitespace with guide injection
        for &ch in &chars {
            if ch != ' ' && ch != '\t' {
                // End of leading whitespace
                break;
            }

            // Check if we should insert a guide at this position
            if guide_idx < guides.len() && guides[guide_idx].column == col {
                result.push_str(guide_style_start);
                result.push(self.guide_char);
                result.push_str(guide_style_end);
                guide_idx += 1;
            } else {
                result.push(ch);
            }

            // Advance column position
            if ch == '\t' {
                col += self.tab_size;
            } else {
                col += 1;
            }
        }

        // Append the rest of the line (non-whitespace)
        let whitespace_chars = line.chars().take_while(|&c| c == ' ' || c == '\t').count();
        if whitespace_chars < chars.len() {
            result.push_str(
                &line[line
                    .char_indices()
                    .nth(whitespace_chars)
                    .map_or(0, |(i, _)| i)..],
            );
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indent_level() {
        let analyzer = IndentAnalyzer::new(4);

        assert_eq!(analyzer.indent_level(""), 0);
        assert_eq!(analyzer.indent_level("hello"), 0);
        assert_eq!(analyzer.indent_level("    hello"), 1);
        assert_eq!(analyzer.indent_level("        hello"), 2);
        assert_eq!(analyzer.indent_level("\thello"), 1);
        assert_eq!(analyzer.indent_level("\t\thello"), 2);
    }

    #[test]
    fn test_leading_whitespace_count() {
        let analyzer = IndentAnalyzer::new(4);

        assert_eq!(analyzer.leading_whitespace_count(""), 0);
        assert_eq!(analyzer.leading_whitespace_count("hello"), 0);
        assert_eq!(analyzer.leading_whitespace_count("    hello"), 4);
        assert_eq!(analyzer.leading_whitespace_count("\thello"), 4);
        assert_eq!(analyzer.leading_whitespace_count("  \thello"), 6); // 2 spaces + 4 for tab
    }

    #[test]
    fn test_guides_for_line() {
        let mut analyzer = IndentAnalyzer::new(4);
        analyzer.set_enabled(true);

        let guides = analyzer.guides_for_line("        hello", None);
        assert_eq!(guides.len(), 2);
        assert_eq!(guides[0].column, 0);
        assert_eq!(guides[1].column, 4);

        let guides = analyzer.guides_for_line("hello", None);
        assert!(guides.is_empty());
    }

    #[test]
    fn test_guides_disabled() {
        let analyzer = IndentAnalyzer::new(4);

        // Guides disabled by default
        let guides = analyzer.guides_for_line("        hello", None);
        assert!(guides.is_empty());
    }

    #[test]
    fn test_active_guide() {
        let mut analyzer = IndentAnalyzer::new(4);
        analyzer.set_enabled(true);

        let guides = analyzer.guides_for_line("        hello", Some(2));
        assert_eq!(guides.len(), 2);
        assert!(!guides[0].active);
        assert!(guides[1].active);
    }
}
