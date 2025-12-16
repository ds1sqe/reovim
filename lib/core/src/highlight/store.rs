use std::collections::{BTreeMap, HashMap};

use super::{Highlight, HighlightGroup, Style};

/// Stores highlights for all buffers
#[derive(Debug, Default)]
pub struct HighlightStore {
    /// `buffer_id` -> highlights organized by group
    highlights: HashMap<usize, BufferHighlights>,
}

/// Highlights for a single buffer, organized by group
#[derive(Debug, Default)]
pub struct BufferHighlights {
    /// group -> list of highlights
    by_group: BTreeMap<HighlightGroup, Vec<Highlight>>,
}

/// A computed highlight range for a single line
#[derive(Debug, Clone)]
pub struct LineHighlight {
    pub start_col: u32,
    pub end_col: u32,
    pub style: Style,
}

impl HighlightStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add highlights for a buffer
    pub fn add(&mut self, buffer_id: usize, highlights: Vec<Highlight>) {
        let buffer_hl = self.highlights.entry(buffer_id).or_default();
        for hl in highlights {
            buffer_hl.add(hl);
        }
    }

    /// Add a single highlight for a buffer
    pub fn add_one(&mut self, buffer_id: usize, highlight: Highlight) {
        let buffer_hl = self.highlights.entry(buffer_id).or_default();
        buffer_hl.add(highlight);
    }

    /// Clear all highlights of a specific group for a buffer
    pub fn clear_group(&mut self, buffer_id: usize, group: HighlightGroup) {
        if let Some(buffer_hl) = self.highlights.get_mut(&buffer_id) {
            buffer_hl.clear_group(group);
        }
    }

    /// Clear all highlights for a buffer
    pub fn clear_all(&mut self, buffer_id: usize) {
        self.highlights.remove(&buffer_id);
    }

    /// Get all highlight ranges for a specific line (for efficient rendering)
    /// Returns ranges sorted by start column with merged styles
    #[must_use]
    pub fn get_line_highlights(&self, buffer_id: usize, line: u32, line_len: u32) -> Vec<LineHighlight> {
        self.highlights
            .get(&buffer_id)
            .map(|bh| bh.get_line_highlights(line, line_len))
            .unwrap_or_default()
    }

    /// Get buffer highlights for direct access
    #[must_use]
    pub fn get_buffer(&self, buffer_id: usize) -> Option<&BufferHighlights> {
        self.highlights.get(&buffer_id)
    }

    /// Get or create buffer highlights
    pub fn get_or_create_buffer(&mut self, buffer_id: usize) -> &mut BufferHighlights {
        self.highlights.entry(buffer_id).or_default()
    }
}

impl BufferHighlights {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, highlight: Highlight) {
        let group = highlight.group;
        self.by_group.entry(group).or_default().push(highlight);
    }

    pub fn clear_group(&mut self, group: HighlightGroup) {
        self.by_group.remove(&group);
    }

    pub fn clear_all(&mut self) {
        self.by_group.clear();
    }

    /// Get all highlight ranges for a specific line
    /// Returns non-overlapping ranges with properly merged styles
    #[must_use]
    #[allow(clippy::collapsible_if)]
    pub fn get_line_highlights(&self, line: u32, line_len: u32) -> Vec<LineHighlight> {
        // Collect all ranges that touch this line
        let mut ranges: Vec<(u32, u32, Style, HighlightGroup)> = Vec::new();

        for (group, highlights) in &self.by_group {
            for hl in highlights {
                if let Some((start, end)) = hl.span.cols_for_line(line, line_len) {
                    if start < end {
                        ranges.push((start, end, hl.style.clone(), *group));
                    }
                }
            }
        }

        if ranges.is_empty() {
            return Vec::new();
        }

        // Sort by start position
        ranges.sort_by_key(|(start, _, _, _)| *start);

        // Merge overlapping ranges using sweep line algorithm
        Self::merge_ranges(ranges)
    }

    /// Merge overlapping highlight ranges, respecting priority
    #[allow(clippy::collapsible_if)]
    fn merge_ranges(ranges: Vec<(u32, u32, Style, HighlightGroup)>) -> Vec<LineHighlight> {
        if ranges.is_empty() {
            return Vec::new();
        }

        // Collect all boundary points
        let mut events: Vec<(u32, bool, Style, HighlightGroup)> = Vec::new();
        for (start, end, style, group) in ranges {
            events.push((start, true, style.clone(), group)); // start event
            events.push((end, false, style, group)); // end event
        }

        // Sort events: by position, then end before start at same position
        events.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.cmp(&b.1)) // false (end) before true (start)
        });

        let mut result: Vec<LineHighlight> = Vec::new();
        let mut active: BTreeMap<HighlightGroup, Style> = BTreeMap::new();
        let mut prev_pos: Option<u32> = None;
        let mut prev_style: Option<Style> = None;

        for (pos, is_start, style, group) in events {
            // Emit segment from prev_pos to pos if we have active highlights
            if let Some(pp) = prev_pos {
                if pp < pos {
                    if let Some(merged) = &prev_style {
                        result.push(LineHighlight {
                            start_col: pp,
                            end_col: pos,
                            style: merged.clone(),
                        });
                    }
                }
            }

            // Update active set
            if is_start {
                active.insert(group, style);
            } else {
                active.remove(&group);
            }

            // Compute merged style from active highlights
            prev_style = if active.is_empty() {
                None
            } else {
                // Merge all active styles, lower priority first (BTreeMap is sorted by key)
                let mut merged = Style::default();
                for s in active.values() {
                    merged = merged.merge(s);
                }
                Some(merged)
            };

            prev_pos = Some(pos);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::Span;
    use reovim_sys::style::Color;

    #[test]
    fn test_single_highlight() {
        let mut store = HighlightStore::new();
        store.add_one(
            0,
            Highlight::new(
                Span::single_line(0, 5, 10),
                Style::new().bg(Color::Yellow),
                HighlightGroup::Search,
            ),
        );

        let highlights = store.get_line_highlights(0, 0, 20);
        assert_eq!(highlights.len(), 1);
        assert_eq!(highlights[0].start_col, 5);
        assert_eq!(highlights[0].end_col, 10);
    }

    #[test]
    fn test_overlapping_highlights() {
        let mut store = HighlightStore::new();
        // Lower priority: cols 0-15 with blue fg
        store.add_one(
            0,
            Highlight::new(
                Span::single_line(0, 0, 15),
                Style::new().fg(Color::Blue),
                HighlightGroup::Syntax,
            ),
        );
        // Higher priority: cols 5-10 with yellow bg
        store.add_one(
            0,
            Highlight::new(
                Span::single_line(0, 5, 10),
                Style::new().bg(Color::Yellow),
                HighlightGroup::Search,
            ),
        );

        let highlights = store.get_line_highlights(0, 0, 20);
        // Should have 3 segments: 0-5 (blue), 5-10 (blue+yellow), 10-15 (blue)
        assert_eq!(highlights.len(), 3);

        // First segment: just blue
        assert_eq!(highlights[0].start_col, 0);
        assert_eq!(highlights[0].end_col, 5);
        assert_eq!(highlights[0].style.fg, Some(Color::Blue));
        assert_eq!(highlights[0].style.bg, None);

        // Middle segment: blue + yellow bg
        assert_eq!(highlights[1].start_col, 5);
        assert_eq!(highlights[1].end_col, 10);
        assert_eq!(highlights[1].style.fg, Some(Color::Blue));
        assert_eq!(highlights[1].style.bg, Some(Color::Yellow));

        // Last segment: just blue
        assert_eq!(highlights[2].start_col, 10);
        assert_eq!(highlights[2].end_col, 15);
        assert_eq!(highlights[2].style.fg, Some(Color::Blue));
        assert_eq!(highlights[2].style.bg, None);
    }
}
