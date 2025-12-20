//! Per-buffer decoration storage
//!
//! Stores decorations for each buffer with efficient line-based lookup.

use std::collections::{BTreeMap, HashMap};

use super::types::{Decoration, DecorationGroup};

/// Reference to a decoration with its group
#[derive(Debug, Clone)]
pub struct DecorationRef {
    pub group: DecorationGroup,
    pub index: usize,
}

/// Per-buffer decoration storage
#[derive(Debug, Default)]
pub struct BufferDecorations {
    /// Decorations organized by group for priority handling
    by_group: BTreeMap<DecorationGroup, Vec<Decoration>>,
    /// Cached line-indexed lookup for O(1) rendering
    by_line: HashMap<u32, Vec<DecorationRef>>,
    /// Whether decorations need regeneration
    stale: bool,
}

impl BufferDecorations {
    /// Create a new empty buffer decorations store
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set decorations for a specific group, rebuilding line index
    pub fn set_decorations(&mut self, group: DecorationGroup, decorations: Vec<Decoration>) {
        self.by_group.insert(group, decorations);
        self.rebuild_line_index();
    }

    /// Clear all decorations
    pub fn clear(&mut self) {
        self.by_group.clear();
        self.by_line.clear();
        self.stale = false;
    }

    /// Mark decorations as stale (need regeneration)
    pub const fn mark_stale(&mut self) {
        self.stale = true;
    }

    /// Check if decorations are stale
    #[must_use]
    pub const fn is_stale(&self) -> bool {
        self.stale
    }

    /// Clear the stale flag
    pub const fn clear_stale(&mut self) {
        self.stale = false;
    }

    /// Get all decorations affecting a specific line
    #[must_use]
    pub fn get_line_decorations(&self, line: u32) -> Vec<&Decoration> {
        let Some(refs) = self.by_line.get(&line) else {
            return Vec::new();
        };

        refs.iter()
            .filter_map(|r| self.by_group.get(&r.group).and_then(|v| v.get(r.index)))
            .collect()
    }

    /// Get decorations for a line filtered by group
    #[must_use]
    pub fn get_line_decorations_by_group(
        &self,
        line: u32,
        group: DecorationGroup,
    ) -> Vec<&Decoration> {
        let Some(refs) = self.by_line.get(&line) else {
            return Vec::new();
        };

        refs.iter()
            .filter(|r| r.group == group)
            .filter_map(|r| self.by_group.get(&r.group).and_then(|v| v.get(r.index)))
            .collect()
    }

    /// Get the line background for a line (if any)
    ///
    /// Returns the highest priority line background decoration
    #[must_use]
    pub fn get_line_background(&self, line: u32) -> Option<&Decoration> {
        self.get_line_decorations(line)
            .into_iter()
            .find(|d| matches!(d, Decoration::LineBackground { .. }))
    }

    /// Get all conceal decorations for a line
    #[must_use]
    pub fn get_conceals(&self, line: u32) -> Vec<&Decoration> {
        self.get_line_decorations(line)
            .into_iter()
            .filter(|d| matches!(d, Decoration::Conceal { .. } | Decoration::Hide { .. }))
            .collect()
    }

    /// Get all inline style decorations for a line
    #[must_use]
    pub fn get_inline_styles(&self, line: u32) -> Vec<&Decoration> {
        self.get_line_decorations(line)
            .into_iter()
            .filter(|d| matches!(d, Decoration::InlineStyle { .. }))
            .collect()
    }

    /// Rebuild the line index from decorations
    fn rebuild_line_index(&mut self) {
        self.by_line.clear();

        for (group, decorations) in &self.by_group {
            for (index, decoration) in decorations.iter().enumerate() {
                let start = decoration.start_line();
                let end = decoration.end_line();

                for line in start..=end {
                    self.by_line.entry(line).or_default().push(DecorationRef {
                        group: *group,
                        index,
                    });
                }
            }
        }

        // Sort each line's decorations by group priority (higher first)
        for refs in self.by_line.values_mut() {
            refs.sort_by(|a, b| b.group.cmp(&a.group));
        }
    }

    /// Check if there are any decorations
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_group.values().all(Vec::is_empty)
    }

    /// Get total decoration count
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_group.values().map(Vec::len).sum()
    }
}

/// Central store for all buffer decorations
#[derive(Debug, Default)]
pub struct DecorationStore {
    decorations: HashMap<usize, BufferDecorations>,
}

impl DecorationStore {
    /// Create a new decoration store
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get decorations for a buffer
    #[must_use]
    pub fn get(&self, buffer_id: usize) -> Option<&BufferDecorations> {
        self.decorations.get(&buffer_id)
    }

    /// Get mutable decorations for a buffer
    pub fn get_mut(&mut self, buffer_id: usize) -> Option<&mut BufferDecorations> {
        self.decorations.get_mut(&buffer_id)
    }

    /// Get or create decorations for a buffer
    pub fn get_or_create(&mut self, buffer_id: usize) -> &mut BufferDecorations {
        self.decorations.entry(buffer_id).or_default()
    }

    /// Set decorations for a buffer
    pub fn set_decorations(
        &mut self,
        buffer_id: usize,
        group: DecorationGroup,
        decorations: Vec<Decoration>,
    ) {
        self.get_or_create(buffer_id)
            .set_decorations(group, decorations);
    }

    /// Mark buffer decorations as stale
    pub fn mark_stale(&mut self, buffer_id: usize) {
        if let Some(buf_dec) = self.decorations.get_mut(&buffer_id) {
            buf_dec.mark_stale();
        }
    }

    /// Clear decorations for a buffer
    pub fn clear(&mut self, buffer_id: usize) {
        if let Some(buf_dec) = self.decorations.get_mut(&buffer_id) {
            buf_dec.clear();
        }
    }

    /// Remove buffer from store
    pub fn remove(&mut self, buffer_id: usize) {
        self.decorations.remove(&buffer_id);
    }

    /// Get line decorations for a buffer
    #[must_use]
    pub fn get_line_decorations(&self, buffer_id: usize, line: u32) -> Vec<&Decoration> {
        self.decorations
            .get(&buffer_id)
            .map_or_else(Vec::new, |bd| bd.get_line_decorations(line))
    }

    /// Get line background for a buffer line
    #[must_use]
    pub fn get_line_background(&self, buffer_id: usize, line: u32) -> Option<&Decoration> {
        self.decorations
            .get(&buffer_id)
            .and_then(|bd| bd.get_line_background(line))
    }

    /// Get conceals for a buffer line
    #[must_use]
    pub fn get_conceals(&self, buffer_id: usize, line: u32) -> Vec<&Decoration> {
        self.decorations
            .get(&buffer_id)
            .map_or_else(Vec::new, |bd| bd.get_conceals(line))
    }

    /// Get inline styles for a buffer line
    #[must_use]
    pub fn get_inline_styles(&self, buffer_id: usize, line: u32) -> Vec<&Decoration> {
        self.decorations
            .get(&buffer_id)
            .map_or_else(Vec::new, |bd| bd.get_inline_styles(line))
    }

    /// Check if buffer has stale decorations
    #[must_use]
    pub fn is_stale(&self, buffer_id: usize) -> bool {
        self.decorations
            .get(&buffer_id)
            .is_some_and(BufferDecorations::is_stale)
    }
}

#[cfg(test)]
mod tests {
    use crate::highlight::Span;

    use super::*;

    #[test]
    fn test_buffer_decorations_line_lookup() {
        let mut bd = BufferDecorations::new();

        let decorations = vec![
            Decoration::single_line_background(
                0,
                crate::highlight::Style::new().bg(reovim_sys::style::Color::Blue),
            ),
            Decoration::single_line_background(
                2,
                crate::highlight::Style::new().bg(reovim_sys::style::Color::Green),
            ),
        ];

        bd.set_decorations(DecorationGroup::Language, decorations);

        assert_eq!(bd.get_line_decorations(0).len(), 1);
        assert!(bd.get_line_decorations(1).is_empty());
        assert_eq!(bd.get_line_decorations(2).len(), 1);
    }

    #[test]
    fn test_multiline_decoration() {
        let mut bd = BufferDecorations::new();

        let decorations = vec![Decoration::LineBackground {
            start_line: 5,
            end_line: 10,
            style: crate::highlight::Style::new().bg(reovim_sys::style::Color::Red),
        }];

        bd.set_decorations(DecorationGroup::Language, decorations);

        assert!(bd.get_line_decorations(4).is_empty());
        assert_eq!(bd.get_line_decorations(5).len(), 1);
        assert_eq!(bd.get_line_decorations(7).len(), 1);
        assert_eq!(bd.get_line_decorations(10).len(), 1);
        assert!(bd.get_line_decorations(11).is_empty());
    }

    #[test]
    fn test_conceal_lookup() {
        let mut bd = BufferDecorations::new();

        let decorations = vec![
            Decoration::conceal(Span::single_line(0, 0, 2), "󰉫 ", None),
            Decoration::single_line_background(
                0,
                crate::highlight::Style::new().bg(reovim_sys::style::Color::Blue),
            ),
        ];

        bd.set_decorations(DecorationGroup::Language, decorations);

        let conceals = bd.get_conceals(0);
        assert_eq!(conceals.len(), 1);
        assert!(matches!(conceals[0], Decoration::Conceal { .. }));
    }
}
