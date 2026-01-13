//! Decoration storage with efficient line-based querying.
//!
//! Provides per-buffer decoration storage with priority-based layering
//! and efficient line lookups.

use std::collections::{BTreeMap, HashMap};

use super::types::{Decoration, DecorationGroup};

/// Reference to a decoration within a buffer's decoration storage.
#[derive(Debug, Clone, Copy)]
pub struct DecorationRef {
    /// The priority group
    pub group: DecorationGroup,
    /// Index within the group's decoration list
    pub index: usize,
}

/// Per-buffer decoration storage with efficient querying.
///
/// Organizes decorations by priority group and provides line-indexed lookups.
#[derive(Debug, Default)]
pub struct BufferDecorations {
    /// Decorations organized by priority group
    by_group: BTreeMap<DecorationGroup, Vec<Decoration>>,
    /// Line-indexed cache for fast lookup
    by_line: HashMap<u32, Vec<DecorationRef>>,
    /// Cache validity flag
    dirty: bool,
}

impl BufferDecorations {
    /// Create a new empty decoration storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_group: BTreeMap::new(),
            by_line: HashMap::new(),
            dirty: false,
        }
    }

    /// Add a decoration to the specified group.
    pub fn add(&mut self, group: DecorationGroup, decoration: Decoration) {
        let decorations = self.by_group.entry(group).or_default();
        decorations.push(decoration);
        self.dirty = true;
    }

    /// Add multiple decorations to the specified group.
    pub fn add_all(
        &mut self,
        group: DecorationGroup,
        decorations: impl IntoIterator<Item = Decoration>,
    ) {
        let group_decorations = self.by_group.entry(group).or_default();
        group_decorations.extend(decorations);
        self.dirty = true;
    }

    /// Clear all decorations in a specific group.
    pub fn clear_group(&mut self, group: DecorationGroup) {
        if self.by_group.remove(&group).is_some() {
            self.dirty = true;
        }
    }

    /// Clear all decorations.
    pub fn clear_all(&mut self) {
        self.by_group.clear();
        self.by_line.clear();
        self.dirty = false;
    }

    /// Get all decorations affecting a line, sorted by priority (lowest first).
    ///
    /// Returns decorations in priority order so they can be applied sequentially.
    #[must_use]
    pub fn for_line(&self, line: u32) -> Vec<&Decoration> {
        let mut result = Vec::new();

        // BTreeMap is already sorted by key (DecorationGroup)
        for decorations in self.by_group.values() {
            for decoration in decorations {
                if decoration.affects_line(line) {
                    result.push(decoration);
                }
            }
        }

        result
    }

    /// Get all decorations affecting a line, sorted by priority (highest first).
    ///
    /// Useful when you need to check which decoration should "win" at a position.
    #[must_use]
    pub fn for_line_reverse_priority(&self, line: u32) -> Vec<&Decoration> {
        let mut result = self.for_line(line);
        result.reverse();
        result
    }

    /// Get conceal decorations for a line.
    #[must_use]
    pub fn get_conceals(&self, line: u32) -> Vec<&Decoration> {
        self.for_line(line)
            .into_iter()
            .filter(|d| d.is_conceal())
            .collect()
    }

    /// Get hide decorations for a line.
    #[must_use]
    pub fn get_hides(&self, line: u32) -> Vec<&Decoration> {
        self.for_line(line)
            .into_iter()
            .filter(|d| d.is_hide())
            .collect()
    }

    /// Get line background decorations for a line.
    #[must_use]
    pub fn get_line_backgrounds(&self, line: u32) -> Vec<&Decoration> {
        self.for_line(line)
            .into_iter()
            .filter(|d| d.is_line_background())
            .collect()
    }

    /// Get inline style decorations for a line.
    #[must_use]
    pub fn get_inline_styles(&self, line: u32) -> Vec<&Decoration> {
        self.for_line(line)
            .into_iter()
            .filter(|d| d.is_inline_style())
            .collect()
    }

    /// Check if there are any decorations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_group.values().all(Vec::is_empty)
    }

    /// Get the total number of decorations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_group.values().map(Vec::len).sum()
    }

    /// Check if the line cache needs rebuilding.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Rebuild the line index cache.
    ///
    /// This is an optimization for repeated line queries.
    /// Call this after adding multiple decorations.
    pub fn rebuild_cache(&mut self) {
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

        self.dirty = false;
    }
}

/// Global decoration store mapping buffer IDs to their decorations.
#[derive(Debug, Default)]
pub struct DecorationStore {
    buffers: HashMap<usize, BufferDecorations>,
}

impl DecorationStore {
    /// Create a new empty decoration store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    /// Get or create decorations for a buffer.
    pub fn get_or_create(&mut self, buffer_id: usize) -> &mut BufferDecorations {
        self.buffers.entry(buffer_id).or_default()
    }

    /// Get decorations for a buffer (immutable).
    #[must_use]
    pub fn get(&self, buffer_id: usize) -> Option<&BufferDecorations> {
        self.buffers.get(&buffer_id)
    }

    /// Get decorations for a buffer (mutable).
    pub fn get_mut(&mut self, buffer_id: usize) -> Option<&mut BufferDecorations> {
        self.buffers.get_mut(&buffer_id)
    }

    /// Remove decorations for a buffer.
    pub fn remove(&mut self, buffer_id: usize) -> Option<BufferDecorations> {
        self.buffers.remove(&buffer_id)
    }

    /// Check if a buffer has decorations.
    #[must_use]
    pub fn contains(&self, buffer_id: usize) -> bool {
        self.buffers.contains_key(&buffer_id)
    }

    /// Get the number of buffers with decorations.
    #[must_use]
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Clear all decorations for all buffers.
    pub fn clear_all(&mut self) {
        self.buffers.clear();
    }
}

#[cfg(test)]
mod tests {
    use reovim_core::highlight::Style;

    use super::{super::types::Span, *};

    #[test]
    fn test_buffer_decorations_empty() {
        let decorations = BufferDecorations::new();
        assert!(decorations.is_empty());
        assert_eq!(decorations.len(), 0);
    }

    #[test]
    fn test_buffer_decorations_add() {
        let mut decorations = BufferDecorations::new();

        decorations.add(
            DecorationGroup::Language,
            Decoration::conceal(Span::line(5, 0, 10), "test", None),
        );

        assert!(!decorations.is_empty());
        assert_eq!(decorations.len(), 1);
    }

    #[test]
    fn test_buffer_decorations_for_line() {
        let mut decorations = BufferDecorations::new();

        decorations.add(
            DecorationGroup::Language,
            Decoration::conceal(Span::line(5, 0, 10), "lang", None),
        );
        decorations.add(
            DecorationGroup::Search,
            Decoration::inline_style(Span::line(5, 0, 5), Style::default()),
        );
        decorations
            .add(DecorationGroup::Visual, Decoration::line_background(5, 5, Style::default()));

        // Line 5 should have all three
        let line5 = decorations.for_line(5);
        assert_eq!(line5.len(), 3);

        // Line 0 should have none
        let line0 = decorations.for_line(0);
        assert_eq!(line0.len(), 0);
    }

    #[test]
    fn test_buffer_decorations_priority_order() {
        let mut decorations = BufferDecorations::new();

        // Add in reverse priority order
        decorations
            .add(DecorationGroup::Visual, Decoration::line_background(5, 5, Style::default()));
        decorations.add(
            DecorationGroup::Language,
            Decoration::conceal(Span::line(5, 0, 10), "lang", None),
        );
        decorations.add(
            DecorationGroup::Search,
            Decoration::inline_style(Span::line(5, 0, 5), Style::default()),
        );

        // Should be returned in priority order (lowest first)
        let line5 = decorations.for_line(5);
        assert!(line5[0].is_conceal()); // Language (0)
        assert!(line5[1].is_inline_style()); // Search (20)
        assert!(line5[2].is_line_background()); // Visual (40)
    }

    #[test]
    fn test_buffer_decorations_clear_group() {
        let mut decorations = BufferDecorations::new();

        decorations
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(5, 0, 10), "a", None));
        decorations.add(
            DecorationGroup::Search,
            Decoration::inline_style(Span::line(5, 0, 5), Style::default()),
        );

        assert_eq!(decorations.len(), 2);

        decorations.clear_group(DecorationGroup::Language);
        assert_eq!(decorations.len(), 1);

        // Should still have search decoration
        let line5 = decorations.for_line(5);
        assert_eq!(line5.len(), 1);
        assert!(line5[0].is_inline_style());
    }

    #[test]
    fn test_decoration_store_basic() {
        let mut store = DecorationStore::new();

        let buffer1 = store.get_or_create(1);
        buffer1.add(
            DecorationGroup::Language,
            Decoration::conceal(Span::line(0, 0, 5), "test", None),
        );

        assert!(store.contains(1));
        assert!(!store.contains(2));
        assert_eq!(store.buffer_count(), 1);

        store.remove(1);
        assert!(!store.contains(1));
        assert_eq!(store.buffer_count(), 0);
    }
}
