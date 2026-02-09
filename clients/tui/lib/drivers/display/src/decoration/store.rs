//! Decoration storage with efficient line-based querying.
//!
//! Provides per-buffer decoration storage with priority-based layering
//! and efficient line lookups.

use std::collections::{BTreeMap, HashMap};

use reovim_kernel::api::v1::BufferId;

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
    buffers: HashMap<BufferId, BufferDecorations>,
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
    pub fn get_or_create(&mut self, buffer_id: BufferId) -> &mut BufferDecorations {
        self.buffers.entry(buffer_id).or_default()
    }

    /// Get decorations for a buffer (immutable).
    #[must_use]
    pub fn get(&self, buffer_id: BufferId) -> Option<&BufferDecorations> {
        self.buffers.get(&buffer_id)
    }

    /// Get decorations for a buffer (mutable).
    pub fn get_mut(&mut self, buffer_id: BufferId) -> Option<&mut BufferDecorations> {
        self.buffers.get_mut(&buffer_id)
    }

    /// Remove decorations for a buffer.
    pub fn remove(&mut self, buffer_id: BufferId) -> Option<BufferDecorations> {
        self.buffers.remove(&buffer_id)
    }

    /// Check if a buffer has decorations.
    #[must_use]
    pub fn contains(&self, buffer_id: BufferId) -> bool {
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
    use crate::highlight::Style;

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

        let buffer_id = BufferId::from_raw(1);
        let buffer_id_2 = BufferId::from_raw(2);

        let buffer1 = store.get_or_create(buffer_id);
        buffer1.add(
            DecorationGroup::Language,
            Decoration::conceal(Span::line(0, 0, 5), "test", None),
        );

        assert!(store.contains(buffer_id));
        assert!(!store.contains(buffer_id_2));
        assert_eq!(store.buffer_count(), 1);

        store.remove(buffer_id);
        assert!(!store.contains(buffer_id));
        assert_eq!(store.buffer_count(), 0);
    }

    // =========================================================================
    // BufferDecorations extended tests
    // =========================================================================

    #[test]
    fn test_buffer_decorations_add_all() {
        let mut decorations = BufferDecorations::new();

        let decos = vec![
            Decoration::conceal(Span::line(1, 0, 5), "a", None),
            Decoration::conceal(Span::line(2, 0, 5), "b", None),
            Decoration::conceal(Span::line(3, 0, 5), "c", None),
        ];

        decorations.add_all(DecorationGroup::Language, decos);
        assert_eq!(decorations.len(), 3);
        assert!(decorations.is_dirty());
    }

    #[test]
    fn test_buffer_decorations_clear_all() {
        let mut decorations = BufferDecorations::new();
        decorations
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(0, 0, 5), "a", None));
        decorations.add(
            DecorationGroup::Search,
            Decoration::inline_style(Span::line(0, 0, 5), Style::default()),
        );
        assert_eq!(decorations.len(), 2);

        decorations.clear_all();
        assert!(decorations.is_empty());
        assert_eq!(decorations.len(), 0);
    }

    #[test]
    fn test_buffer_decorations_clear_nonexistent_group() {
        let mut decorations = BufferDecorations::new();
        decorations
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(0, 0, 5), "a", None));

        // Clearing a group that doesn't exist should not mark dirty again
        let was_dirty = decorations.is_dirty();
        decorations.rebuild_cache();
        decorations.clear_group(DecorationGroup::Search);
        // Should not change dirty state since group was not present
        assert!(!decorations.is_dirty());
        let _ = was_dirty;
    }

    #[test]
    fn test_buffer_decorations_for_line_reverse_priority() {
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

        let reversed = decorations.for_line_reverse_priority(5);
        assert_eq!(reversed.len(), 3);
        // Should be in reverse priority order (highest first)
        assert!(reversed[0].is_line_background()); // Visual (40)
        assert!(reversed[1].is_inline_style()); // Search (20)
        assert!(reversed[2].is_conceal()); // Language (0)
    }

    #[test]
    fn test_buffer_decorations_get_conceals() {
        let mut decorations = BufferDecorations::new();
        decorations.add(
            DecorationGroup::Language,
            Decoration::conceal(Span::line(5, 0, 10), "conceal", None),
        );
        decorations.add(
            DecorationGroup::Search,
            Decoration::inline_style(Span::line(5, 0, 5), Style::default()),
        );

        let conceals = decorations.get_conceals(5);
        assert_eq!(conceals.len(), 1);
        assert!(conceals[0].is_conceal());

        // No conceals on other lines
        assert!(decorations.get_conceals(0).is_empty());
    }

    #[test]
    fn test_buffer_decorations_get_hides() {
        let mut decorations = BufferDecorations::new();
        decorations.add(DecorationGroup::Language, Decoration::hide(Span::line(3, 2, 8)));
        decorations
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(3, 0, 10), "x", None));

        let hides = decorations.get_hides(3);
        assert_eq!(hides.len(), 1);
        assert!(hides[0].is_hide());
    }

    #[test]
    fn test_buffer_decorations_get_line_backgrounds() {
        let mut decorations = BufferDecorations::new();
        decorations
            .add(DecorationGroup::Visual, Decoration::line_background(1, 5, Style::default()));
        decorations
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(3, 0, 10), "x", None));

        let line_bgs = decorations.get_line_backgrounds(3);
        assert_eq!(line_bgs.len(), 1);
        assert!(line_bgs[0].is_line_background());

        // Line 0 has no backgrounds
        assert!(decorations.get_line_backgrounds(0).is_empty());
    }

    #[test]
    fn test_buffer_decorations_get_inline_styles() {
        let mut decorations = BufferDecorations::new();
        decorations.add(
            DecorationGroup::Search,
            Decoration::inline_style(Span::line(7, 0, 5), Style::default()),
        );
        decorations
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(7, 0, 10), "x", None));

        let styles = decorations.get_inline_styles(7);
        assert_eq!(styles.len(), 1);
        assert!(styles[0].is_inline_style());
    }

    #[test]
    fn test_buffer_decorations_dirty_flag() {
        let mut decorations = BufferDecorations::new();
        assert!(!decorations.is_dirty());

        decorations
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(0, 0, 5), "x", None));
        assert!(decorations.is_dirty());

        decorations.rebuild_cache();
        assert!(!decorations.is_dirty());

        decorations.add_all(
            DecorationGroup::Search,
            vec![Decoration::inline_style(
                Span::line(1, 0, 3),
                Style::default(),
            )],
        );
        assert!(decorations.is_dirty());
    }

    #[test]
    fn test_buffer_decorations_rebuild_cache() {
        let mut decorations = BufferDecorations::new();
        decorations
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(0, 0, 5), "x", None));
        decorations.add(
            DecorationGroup::Search,
            Decoration::inline_style(Span::new(0, 0, 2, 5), Style::default()),
        );

        decorations.rebuild_cache();
        assert!(!decorations.is_dirty());
    }

    #[test]
    fn test_buffer_decorations_multiline_span() {
        let mut decorations = BufferDecorations::new();
        decorations.add(
            DecorationGroup::Language,
            Decoration::conceal(Span::new(2, 0, 5, 10), "multi", None),
        );

        // Should affect lines 2, 3, 4, 5
        assert!(decorations.for_line(2).len() == 1);
        assert!(decorations.for_line(3).len() == 1);
        assert!(decorations.for_line(4).len() == 1);
        assert!(decorations.for_line(5).len() == 1);
        assert!(decorations.for_line(1).is_empty());
        assert!(decorations.for_line(6).is_empty());
    }

    // =========================================================================
    // DecorationStore extended tests
    // =========================================================================

    #[test]
    fn test_decoration_store_get() {
        let mut store = DecorationStore::new();
        let buffer_id = BufferId::from_raw(1);

        // get on non-existent buffer
        assert!(store.get(buffer_id).is_none());

        store.get_or_create(buffer_id);
        assert!(store.get(buffer_id).is_some());
    }

    #[test]
    fn test_decoration_store_get_mut() {
        let mut store = DecorationStore::new();
        let buffer_id = BufferId::from_raw(1);

        // get_mut on non-existent buffer
        assert!(store.get_mut(buffer_id).is_none());

        store.get_or_create(buffer_id);
        let buf = store.get_mut(buffer_id);
        assert!(buf.is_some());
        buf.unwrap()
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(0, 0, 5), "x", None));

        assert_eq!(store.get(buffer_id).unwrap().len(), 1);
    }

    #[test]
    fn test_decoration_store_remove_returns_decorations() {
        let mut store = DecorationStore::new();
        let buffer_id = BufferId::from_raw(1);

        store
            .get_or_create(buffer_id)
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(0, 0, 5), "x", None));

        let removed = store.remove(buffer_id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().len(), 1);

        // Remove again returns None
        assert!(store.remove(buffer_id).is_none());
    }

    #[test]
    fn test_decoration_store_clear_all() {
        let mut store = DecorationStore::new();
        store.get_or_create(BufferId::from_raw(1));
        store.get_or_create(BufferId::from_raw(2));
        store.get_or_create(BufferId::from_raw(3));
        assert_eq!(store.buffer_count(), 3);

        store.clear_all();
        assert_eq!(store.buffer_count(), 0);
    }

    #[test]
    fn test_decoration_store_multiple_buffers() {
        let mut store = DecorationStore::new();
        let b1 = BufferId::from_raw(1);
        let b2 = BufferId::from_raw(2);

        store
            .get_or_create(b1)
            .add(DecorationGroup::Language, Decoration::conceal(Span::line(0, 0, 5), "a", None));
        store.get_or_create(b2).add(
            DecorationGroup::Search,
            Decoration::inline_style(Span::line(0, 0, 5), Style::default()),
        );

        assert!(store.contains(b1));
        assert!(store.contains(b2));
        assert_eq!(store.buffer_count(), 2);
        assert_eq!(store.get(b1).unwrap().len(), 1);
        assert_eq!(store.get(b2).unwrap().len(), 1);
    }
}
