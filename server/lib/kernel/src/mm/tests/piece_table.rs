//! Tests for `PieceTree`.

use super::*;

fn orig_piece(byte_start: u64, byte_len: u64, chars: u64, lines: u64) -> Piece {
    Piece {
        source: PieceSource::Original { byte_start, byte_len },
        metrics: PieceMetrics {
            byte_len,
            char_count: chars,
            line_count: lines,
        },
    }
}

fn add_piece(offset: usize, len: usize, chars: u64, lines: u64) -> Piece {
    Piece {
        source: PieceSource::Add { offset, len },
        metrics: PieceMetrics {
            byte_len: len as u64,
            char_count: chars,
            line_count: lines,
        },
    }
}

// ── Construction ──────────────────────────────────────────────────────

#[test]
fn empty_tree() {
    let tree = PieceTree::new();
    assert!(tree.is_empty());
    assert_eq!(tree.byte_len(), 0);
    assert_eq!(tree.char_count(), 0);
    assert_eq!(tree.line_count(), 0);
    assert_eq!(tree.piece_count(), 0);
    assert!(tree.piece_at(0).is_none());
    assert_eq!(tree.iter_pieces().count(), 0);
    tree.validate().unwrap();
}

#[test]
fn single_piece() {
    let piece = orig_piece(0, 100, 100, 5);
    let tree = PieceTree::from_piece(piece);
    assert_eq!(tree.byte_len(), 100);
    assert_eq!(tree.char_count(), 100);
    assert_eq!(tree.line_count(), 5);
    assert_eq!(tree.piece_count(), 1);
    assert!(!tree.is_empty());
    tree.validate().unwrap();
}

// ── Insert ────────────────────────────────────────────────────────────

#[test]
fn insert_into_empty() {
    let tree = PieceTree::new();
    let piece = add_piece(0, 5, 5, 0);
    let tree = tree.insert(0, piece);
    assert_eq!(tree.byte_len(), 5);
    assert_eq!(tree.piece_count(), 1);
    tree.validate().unwrap();
}

#[test]
fn insert_at_start() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let tree = tree.insert(0, add_piece(0, 5, 5, 0));
    assert_eq!(tree.byte_len(), 15);
    assert_eq!(tree.piece_count(), 2);

    // First piece should be the add piece
    let pieces: Vec<_> = tree.iter_pieces().collect();
    assert_eq!(pieces.len(), 2);
    assert!(matches!(pieces[0].source, PieceSource::Add { offset: 0, len: 5 }));
    assert!(matches!(pieces[1].source, PieceSource::Original { byte_start: 0, byte_len: 10 }));
    tree.validate().unwrap();
}

#[test]
fn insert_at_end() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let tree = tree.insert(10, add_piece(0, 5, 5, 0));
    assert_eq!(tree.byte_len(), 15);
    assert_eq!(tree.piece_count(), 2);

    let pieces: Vec<_> = tree.iter_pieces().collect();
    assert!(matches!(pieces[0].source, PieceSource::Original { byte_start: 0, byte_len: 10 }));
    assert!(matches!(pieces[1].source, PieceSource::Add { offset: 0, len: 5 }));
    tree.validate().unwrap();
}

#[test]
fn insert_in_middle_splits_piece() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let tree = tree.insert(5, add_piece(0, 3, 3, 0));
    assert_eq!(tree.byte_len(), 13);
    assert_eq!(tree.piece_count(), 3);

    let pieces: Vec<_> = tree.iter_pieces().collect();
    assert_eq!(pieces.len(), 3);
    // Left half of original
    assert!(matches!(pieces[0].source, PieceSource::Original { byte_start: 0, byte_len: 5 }));
    // Inserted piece
    assert!(matches!(pieces[1].source, PieceSource::Add { offset: 0, len: 3 }));
    // Right half of original
    assert!(matches!(pieces[2].source, PieceSource::Original { byte_start: 5, byte_len: 5 }));
    tree.validate().unwrap();
}

// ── Delete ────────────────────────────────────────────────────────────

#[test]
fn delete_zero_length() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let tree = tree.delete(5, 0);
    assert_eq!(tree.byte_len(), 10);
    assert_eq!(tree.piece_count(), 1);
}

#[test]
fn delete_entire_piece() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let tree = tree.delete(0, 10);
    assert!(tree.is_empty());
    assert_eq!(tree.byte_len(), 0);
    tree.validate().unwrap();
}

#[test]
fn delete_from_start() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let tree = tree.delete(0, 5);
    assert_eq!(tree.byte_len(), 5);
    assert_eq!(tree.piece_count(), 1);

    let pieces: Vec<_> = tree.iter_pieces().collect();
    assert!(matches!(pieces[0].source, PieceSource::Original { byte_start: 5, byte_len: 5 }));
    tree.validate().unwrap();
}

#[test]
fn delete_from_end() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let tree = tree.delete(5, 5);
    assert_eq!(tree.byte_len(), 5);
    assert_eq!(tree.piece_count(), 1);

    let pieces: Vec<_> = tree.iter_pieces().collect();
    assert!(matches!(pieces[0].source, PieceSource::Original { byte_start: 0, byte_len: 5 }));
    tree.validate().unwrap();
}

#[test]
fn delete_middle_creates_two_pieces() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let tree = tree.delete(3, 4);
    assert_eq!(tree.byte_len(), 6);
    assert_eq!(tree.piece_count(), 2);

    let pieces: Vec<_> = tree.iter_pieces().collect();
    assert!(matches!(pieces[0].source, PieceSource::Original { byte_start: 0, byte_len: 3 }));
    assert!(matches!(pieces[1].source, PieceSource::Original { byte_start: 7, byte_len: 3 }));
    tree.validate().unwrap();
}

#[test]
fn delete_across_pieces() {
    // Two pieces: [0..5] [5..10]
    let tree = PieceTree::from_piece(orig_piece(0, 5, 5, 0));
    let tree = tree.insert(5, orig_piece(5, 5, 5, 0));
    assert_eq!(tree.byte_len(), 10);

    // Delete bytes 3..7 (crosses piece boundary)
    let tree = tree.delete(3, 4);
    assert_eq!(tree.byte_len(), 6);
    tree.validate().unwrap();
}

#[test]
fn delete_from_empty() {
    let tree = PieceTree::new();
    let tree = tree.delete(0, 10);
    assert!(tree.is_empty());
}

// ── Clone (O(1) via Arc) ─────────────────────────────────────────────

#[test]
fn clone_is_cheap() {
    let tree = PieceTree::from_piece(orig_piece(0, 100, 100, 5));
    let strong_before = tree.root_strong_count();

    let cloned = tree.clone();
    let strong_after = tree.root_strong_count();

    assert_eq!(strong_after, strong_before + 1);
    assert_eq!(cloned.byte_len(), tree.byte_len());
}

// ── piece_at ─────────────────────────────────────────────────────────

#[test]
fn piece_at_single() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let (piece, offset) = tree.piece_at(5).unwrap();
    assert_eq!(offset, 5);
    assert!(matches!(piece.source, PieceSource::Original { byte_start: 0, byte_len: 10 }));
}

#[test]
fn piece_at_boundary() {
    let tree = PieceTree::from_piece(orig_piece(0, 5, 5, 0));
    let tree = tree.insert(5, add_piece(0, 5, 5, 0));

    // Offset 4 is in first piece
    let (p, off) = tree.piece_at(4).unwrap();
    assert_eq!(off, 4);
    assert!(matches!(p.source, PieceSource::Original { .. }));

    // Offset 5 is in second piece
    let (p, off) = tree.piece_at(5).unwrap();
    assert_eq!(off, 0);
    assert!(matches!(p.source, PieceSource::Add { .. }));
}

#[test]
fn piece_at_empty() {
    let tree = PieceTree::new();
    assert!(tree.piece_at(0).is_none());
}

// ── Iteration ────────────────────────────────────────────────────────

#[test]
fn iter_pieces_order() {
    let tree = PieceTree::from_piece(orig_piece(0, 5, 5, 0));
    let tree = tree.insert(5, add_piece(0, 3, 3, 0));
    let tree = tree.insert(8, orig_piece(5, 5, 5, 0));

    let pieces: Vec<_> = tree.iter_pieces().collect();
    assert_eq!(pieces.len(), 3);

    let mut cumulative = 0u64;
    for p in &pieces {
        cumulative += p.metrics.byte_len;
    }
    assert_eq!(cumulative, tree.byte_len());
}

// ── Metrics consistency ──────────────────────────────────────────────

#[test]
fn metrics_after_insert() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 2));
    let tree = tree.insert(5, add_piece(0, 5, 5, 1));
    assert_eq!(tree.byte_len(), 15);
    assert_eq!(tree.char_count(), 15);
    assert_eq!(tree.line_count(), 3);
    tree.validate().unwrap();
}

#[test]
fn metrics_after_delete() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 2));
    let tree = tree.delete(0, 5);
    assert_eq!(tree.byte_len(), 5);
    tree.validate().unwrap();
}

// ── PieceMetrics ────────────────────────────────────────────────────

#[test]
fn piece_metrics_from_str() {
    let m = PieceMetrics::compute("hello\nworld");
    assert_eq!(m.byte_len, 11);
    assert_eq!(m.char_count, 11);
    assert_eq!(m.line_count, 1);
}

#[test]
fn piece_metrics_from_str_unicode() {
    let m = PieceMetrics::compute("héllo");
    assert_eq!(m.byte_len, 6); // é is 2 bytes
    assert_eq!(m.char_count, 5);
    assert_eq!(m.line_count, 0);
}

#[test]
fn piece_metrics_from_str_empty() {
    let m = PieceMetrics::compute("");
    assert_eq!(m.byte_len, 0);
    assert_eq!(m.char_count, 0);
    assert_eq!(m.line_count, 0);
}

#[test]
fn piece_metrics_add() {
    let a = PieceMetrics { byte_len: 5, char_count: 5, line_count: 1 };
    let b = PieceMetrics { byte_len: 3, char_count: 3, line_count: 0 };
    let sum = a.add(b);
    assert_eq!(sum.byte_len, 8);
    assert_eq!(sum.char_count, 8);
    assert_eq!(sum.line_count, 1);
}

// ── Edge cases ──────────────────────────────────────────────────────

#[test]
fn single_byte_pieces() {
    let mut tree = PieceTree::new();
    for i in 0..10u8 {
        tree = tree.insert(u64::from(i), add_piece(usize::from(i), 1, 1, 0));
    }
    assert_eq!(tree.byte_len(), 10);
    assert_eq!(tree.piece_count(), 10);
    tree.validate().unwrap();
}

#[test]
fn many_pieces_triggers_tree_building() {
    // Insert enough pieces to trigger multi-level tree
    let mut tree = PieceTree::new();
    for i in 0..50u64 {
        tree = tree.insert(i * 10, orig_piece(i * 10, 10, 10, 0));
    }
    assert_eq!(tree.byte_len(), 500);
    assert_eq!(tree.piece_count(), 50);
    tree.validate().unwrap();
}

#[test]
fn insert_then_delete_all() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let tree = tree.insert(5, add_piece(0, 5, 5, 0));
    assert_eq!(tree.byte_len(), 15);
    let tree = tree.delete(0, 15);
    assert!(tree.is_empty());
    tree.validate().unwrap();
}

#[test]
fn default_is_empty() {
    let tree = PieceTree::default();
    assert!(tree.is_empty());
}

// ── PieceSource ─────────────────────────────────────────────────────

#[test]
fn piece_source_equality() {
    let a = PieceSource::Original { byte_start: 0, byte_len: 10 };
    let b = PieceSource::Original { byte_start: 0, byte_len: 10 };
    assert_eq!(a, b);

    let c = PieceSource::Add { offset: 0, len: 5 };
    let d = PieceSource::Add { offset: 0, len: 5 };
    assert_eq!(c, d);

    assert_ne!(a, c);
}

// ── Debug formatting ─────────────────────────────────────────────────

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn debug_formatting() {
    let tree = PieceTree::from_piece(orig_piece(0, 10, 10, 0));
    let debug = format!("{tree:?}");
    assert!(debug.contains("PieceTree"));
}
