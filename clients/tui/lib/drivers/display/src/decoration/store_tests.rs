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
