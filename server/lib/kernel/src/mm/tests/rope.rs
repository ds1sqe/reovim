use super::*;
use crate::mm::rope::Rope;

// ─── Construction ───────────────────────────────────────────────────────────

#[test]
fn empty_rope() {
    let r = Rope::new();
    assert!(r.is_empty());
    assert_eq!(r.byte_len(), 0);
    assert_eq!(r.char_len(), 0);
    assert_eq!(r.line_count(), 0);
    assert_eq!(r.line(0), None);
    assert_eq!(r.content(), "");
}

#[test]
fn from_empty_str() {
    let r = Rope::from_str("");
    assert!(r.is_empty());
    assert_eq!(r.line_count(), 0);
}

#[test]
fn from_single_line() {
    let r = Rope::from_str("hello");
    assert!(!r.is_empty());
    assert_eq!(r.byte_len(), 5);
    assert_eq!(r.char_len(), 5);
    assert_eq!(r.line_count(), 1);
    assert_eq!(r.line(0), Some("hello"));
    assert_eq!(r.line(1), None);
    assert_eq!(r.content(), "hello");
}

#[test]
fn from_two_lines() {
    let r = Rope::from_str("hello\nworld");
    assert_eq!(r.line_count(), 2);
    assert_eq!(r.line(0), Some("hello"));
    assert_eq!(r.line(1), Some("world"));
    assert_eq!(r.content(), "hello\nworld");
}

#[test]
fn from_trailing_newline() {
    // "abc\n" → 2 lines ("abc", ""), line-separator semantics
    let r = Rope::from_str("abc\n");
    assert_eq!(r.line_count(), 2);
    assert_eq!(r.line(0), Some("abc"));
    assert_eq!(r.line(1), Some(""));
}

#[test]
fn from_multiple_trailing_newlines() {
    let r = Rope::from_str("abc\n\n");
    assert_eq!(r.line_count(), 3);
    assert_eq!(r.line(0), Some("abc"));
    assert_eq!(r.line(1), Some(""));
    assert_eq!(r.line(2), Some(""));
}

#[test]
fn from_only_newlines() {
    let r = Rope::from_str("\n\n\n");
    assert_eq!(r.line_count(), 4);
    assert_eq!(r.line(0), Some(""));
    assert_eq!(r.line(1), Some(""));
    assert_eq!(r.line(2), Some(""));
    assert_eq!(r.line(3), Some(""));
}

#[test]
fn roundtrip_from_str_content() {
    let texts = [
        "hello",
        "hello\nworld",
        "hello\nworld\n",
        "\n",
        "\n\n",
        "abc\ndef\nghi\njkl",
        "",
    ];
    for text in texts {
        let r = Rope::from_str(text);
        assert_eq!(r.content(), text, "roundtrip failed for {text:?}");
    }
}

// ─── Large Rope (tests chunking) ───────────────────────────────────────────

#[test]
fn large_rope_roundtrip() {
    // Build a string larger than MAX_CHUNK_BYTES
    let line = "the quick brown fox jumps over the lazy dog\n";
    let text: String = line.repeat(100);
    let r = Rope::from_str(&text);

    assert_eq!(r.content(), text);
    // 100 lines of content + 1 trailing empty line (text ends with \n)
    assert_eq!(r.line_count(), 101);
    for i in 0..100 {
        assert_eq!(
            r.line(i),
            Some("the quick brown fox jumps over the lazy dog"),
            "line {i} mismatch"
        );
    }
}

#[test]
fn large_rope_line_count() {
    use std::fmt::Write;
    let mut text = String::new();
    for i in 0..1000 {
        writeln!(text, "line {i}").unwrap();
    }
    let r = Rope::from_str(&text);
    // 1000 lines of content + 1 trailing empty line (writeln! ends each with \n)
    assert_eq!(r.line_count(), 1001);
}

// ─── Unicode ────────────────────────────────────────────────────────────────

#[test]
fn unicode_multibyte() {
    let r = Rope::from_str("héllo\nwörld");
    assert_eq!(r.line_count(), 2);
    assert_eq!(r.char_len(), 11);
    assert_eq!(r.line(0), Some("héllo"));
    assert_eq!(r.line(1), Some("wörld"));
    assert_eq!(r.line_len(0), Some(5)); // chars, not bytes
}

#[test]
fn unicode_cjk() {
    let r = Rope::from_str("你好\n世界");
    assert_eq!(r.line_count(), 2);
    assert_eq!(r.char_len(), 5); // 2 + newline + 2
    assert_eq!(r.line(0), Some("你好"));
    assert_eq!(r.line(1), Some("世界"));
}

#[test]
fn unicode_emoji() {
    let r = Rope::from_str("🎉🎊\n🥳");
    assert_eq!(r.line_count(), 2);
    assert_eq!(r.line(0), Some("🎉🎊"));
    assert_eq!(r.line(1), Some("🥳"));
}

// ─── Position Conversion ────────────────────────────────────────────────────

#[test]
fn position_to_byte_simple() {
    let r = Rope::from_str("hello\nworld");
    assert_eq!(r.position_to_byte(0, 0), 0);
    assert_eq!(r.position_to_byte(0, 5), 5);
    assert_eq!(r.position_to_byte(1, 0), 6);
    assert_eq!(r.position_to_byte(1, 5), 11);
}

#[test]
fn position_to_byte_unicode() {
    let r = Rope::from_str("héllo\nworld");
    // 'é' is 2 bytes; "héllo" = 6 bytes
    assert_eq!(r.position_to_byte(0, 0), 0);
    assert_eq!(r.position_to_byte(0, 1), 1); // 'h' = 1 byte
    assert_eq!(r.position_to_byte(0, 2), 3); // 'é' = 2 bytes, so after 'h'+'é' = 3 bytes
    assert_eq!(r.position_to_byte(1, 0), 7); // "héllo\n" = 7 bytes
}

#[test]
fn byte_to_position_simple() {
    let r = Rope::from_str("hello\nworld");
    assert_eq!(r.byte_to_position(0), (0, 0));
    assert_eq!(r.byte_to_position(5), (0, 5));
    assert_eq!(r.byte_to_position(6), (1, 0));
    assert_eq!(r.byte_to_position(11), (1, 5));
}

#[test]
fn byte_to_position_unicode() {
    let r = Rope::from_str("héllo\nworld");
    assert_eq!(r.byte_to_position(0), (0, 0));
    assert_eq!(r.byte_to_position(1), (0, 1)); // after 'h'
    assert_eq!(r.byte_to_position(3), (0, 2)); // after 'é'
    assert_eq!(r.byte_to_position(7), (1, 0)); // start of "world"
}

#[test]
fn position_roundtrip() {
    let r = Rope::from_str("abc\ndef\nghi");
    for line in 0..3 {
        for col in 0..3 {
            let byte = r.position_to_byte(line, col);
            let (l, c) = r.byte_to_position(byte);
            assert_eq!((l, c), (line, col), "roundtrip failed for ({line}, {col})");
        }
    }
}

#[test]
fn char_byte_roundtrip() {
    let r = Rope::from_str("héllo wörld");
    for ci in 0..=r.char_len() {
        let byte = r.char_to_byte(ci);
        let back = r.byte_to_char(byte);
        assert_eq!(back, ci, "char/byte roundtrip failed for char {ci}");
    }
}

// ─── Clone (O(1)) ──────────────────────────────────────────────────────────

#[test]
fn clone_is_structural_sharing() {
    let r1 = Rope::from_str("hello\nworld");
    let r2 = r1.clone();
    assert_eq!(r1, r2);
    // Both should have the same Arc root pointer
    assert!(Arc::ptr_eq(&r1.root, &r2.root));
}

#[test]
fn clone_independence() {
    let r1 = Rope::from_str("hello\nworld");
    let r2 = r1.insert(5, " there");
    // r1 should be unchanged
    assert_eq!(r1.content(), "hello\nworld");
    assert_eq!(r2.content(), "hello there\nworld");
}

// ─── Insert ─────────────────────────────────────────────────────────────────

#[test]
fn insert_into_empty() {
    let r = Rope::new();
    let r2 = r.insert(0, "hello");
    assert_eq!(r2.content(), "hello");
}

#[test]
fn insert_at_beginning() {
    let r = Rope::from_str("world");
    let r2 = r.insert(0, "hello ");
    assert_eq!(r2.content(), "hello world");
}

#[test]
fn insert_at_end() {
    let r = Rope::from_str("hello");
    let r2 = r.insert(5, " world");
    assert_eq!(r2.content(), "hello world");
}

#[test]
fn insert_in_middle() {
    let r = Rope::from_str("hllo");
    let r2 = r.insert(1, "e");
    assert_eq!(r2.content(), "hello");
}

#[test]
fn insert_newline() {
    let r = Rope::from_str("helloworld");
    let r2 = r.insert(5, "\n");
    assert_eq!(r2.line_count(), 2);
    assert_eq!(r2.line(0), Some("hello"));
    assert_eq!(r2.line(1), Some("world"));
}

#[test]
fn insert_multiline() {
    let r = Rope::from_str("ac");
    let r2 = r.insert(1, "b\nd\ne");
    assert_eq!(r2.content(), "ab\nd\nec");
    assert_eq!(r2.line_count(), 3);
}

#[test]
fn insert_empty_text_is_noop() {
    let r = Rope::from_str("hello");
    let r2 = r.insert(3, "");
    assert!(Arc::ptr_eq(&r.root, &r2.root));
}

#[test]
fn insert_preserves_original() {
    let original = Rope::from_str("hello");
    let modified = original.insert(5, " world");
    assert_eq!(original.content(), "hello");
    assert_eq!(modified.content(), "hello world");
}

#[test]
fn insert_unicode() {
    let r = Rope::from_str("hllo");
    // Insert 'é' (2 bytes) at byte 1
    let r2 = r.insert(1, "é");
    assert_eq!(r2.content(), "héllo");
    assert_eq!(r2.char_len(), 5);
    assert_eq!(r2.byte_len(), 6);
}

// ─── Remove ─────────────────────────────────────────────────────────────────

#[test]
fn remove_from_empty() {
    let r = Rope::new();
    let r2 = r.remove(0..5);
    assert!(r2.is_empty());
}

#[test]
fn remove_entire_content() {
    let r = Rope::from_str("hello");
    let r2 = r.remove(0..5);
    assert!(r2.is_empty());
    assert_eq!(r2.content(), "");
}

#[test]
fn remove_beginning() {
    let r = Rope::from_str("hello world");
    let r2 = r.remove(0..6);
    assert_eq!(r2.content(), "world");
}

#[test]
fn remove_end() {
    let r = Rope::from_str("hello world");
    let r2 = r.remove(5..11);
    assert_eq!(r2.content(), "hello");
}

#[test]
fn remove_middle() {
    let r = Rope::from_str("hello world");
    let r2 = r.remove(5..6); // remove space
    assert_eq!(r2.content(), "helloworld");
}

#[test]
fn remove_newline() {
    let r = Rope::from_str("hello\nworld");
    let r2 = r.remove(5..6); // remove the '\n'
    assert_eq!(r2.content(), "helloworld");
    assert_eq!(r2.line_count(), 1);
}

#[test]
fn remove_empty_range() {
    let r = Rope::from_str("hello");
    let r2 = r.remove(2..2);
    assert!(Arc::ptr_eq(&r.root, &r2.root));
}

#[test]
fn remove_preserves_original() {
    let original = Rope::from_str("hello world");
    let modified = original.remove(5..11);
    assert_eq!(original.content(), "hello world");
    assert_eq!(modified.content(), "hello");
}

#[test]
fn remove_across_lines() {
    let r = Rope::from_str("abc\ndef\nghi");
    // Remove "c\ndef\ng" (bytes 2..9)
    let r2 = r.remove(2..9);
    assert_eq!(r2.content(), "abhi");
    assert_eq!(r2.line_count(), 1);
}

// ─── Insert + Remove roundtrip ──────────────────────────────────────────────

#[test]
fn insert_remove_roundtrip() {
    let r = Rope::from_str("hello world");
    let r2 = r.insert(5, " there");
    assert_eq!(r2.content(), "hello there world");
    let r3 = r2.remove(5..11); // remove " there"
    assert_eq!(r3.content(), "hello world");
}

// ─── Iterators ──────────────────────────────────────────────────────────────

#[test]
fn lines_iterator() {
    let r = Rope::from_str("abc\ndef\nghi");
    assert!(r.lines().eq(["abc", "def", "ghi"]));
}

#[test]
fn lines_iterator_empty() {
    let r = Rope::new();
    assert!(r.lines().next().is_none());
}

#[test]
fn lines_exact_size() {
    let r = Rope::from_str("a\nb\nc");
    let lines = r.lines();
    assert_eq!(lines.len(), 3);
}

#[test]
fn chunks_iterator() {
    let r = Rope::from_str("hello");
    let chunks: Vec<&str> = r.chunks().collect();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "hello");
}

#[test]
fn chunks_concatenate_to_content() {
    let line = "a long line for testing chunk splitting purposes\n";
    let text: String = line.repeat(100);
    let r = Rope::from_str(&text);
    let from_chunks: String = r.chunks().collect();
    assert_eq!(from_chunks, text);
}

// ─── PartialEq ──────────────────────────────────────────────────────────────

#[test]
fn equality_same_content() {
    let r1 = Rope::from_str("hello\nworld");
    let r2 = Rope::from_str("hello\nworld");
    assert_eq!(r1, r2);
}

#[test]
fn equality_different_content() {
    let r1 = Rope::from_str("hello");
    let r2 = Rope::from_str("world");
    assert_ne!(r1, r2);
}

#[test]
fn equality_different_structure_same_content() {
    // Build two ropes with same content but potentially different tree structure
    let r1 = Rope::from_str("abcdef");
    let r2 = {
        let r = Rope::from_str("abc");
        r.insert(3, "def")
    };
    assert_eq!(r1, r2);
}

// ─── Display ────────────────────────────────────────────────────────────────

#[test]
fn display_shows_content() {
    let r = Rope::from_str("hello\nworld");
    assert_eq!(format!("{r}"), "hello\nworld");
}

// ─── line_len ───────────────────────────────────────────────────────────────

#[test]
fn line_len_ascii() {
    let r = Rope::from_str("hello\nworld");
    assert_eq!(r.line_len(0), Some(5));
    assert_eq!(r.line_len(1), Some(5));
    assert_eq!(r.line_len(2), None);
}

#[test]
fn line_len_unicode() {
    let r = Rope::from_str("héllo\nwörld");
    assert_eq!(r.line_len(0), Some(5)); // chars, not bytes
    assert_eq!(r.line_len(1), Some(5));
}

// ─── Metrics ────────────────────────────────────────────────────────────────

#[test]
fn metrics_empty() {
    let m = Metrics::from_text("");
    assert_eq!(m, Metrics::default());
}

#[test]
fn metrics_simple() {
    let m = Metrics::from_text("hello");
    assert_eq!(m.byte_len, 5);
    assert_eq!(m.char_len, 5);
    assert_eq!(m.line_count, 1);
}

#[test]
fn metrics_multiline() {
    let m = Metrics::from_text("abc\ndef");
    assert_eq!(m.byte_len, 7);
    assert_eq!(m.char_len, 7);
    assert_eq!(m.line_count, 2);
}

#[test]
fn metrics_trailing_newline() {
    let m = Metrics::from_text("abc\n");
    assert_eq!(m.line_count, 1);
}

#[test]
fn metrics_unicode() {
    let m = Metrics::from_text("héllo");
    assert_eq!(m.byte_len, 6); // 'é' is 2 bytes
    assert_eq!(m.char_len, 5);
    assert_eq!(m.line_count, 1);
}

#[test]
fn metrics_sum() {
    let m1 = Metrics::from_text("abc\n");
    let m2 = Metrics::from_text("def");
    let sum = Metrics::sum([m1, m2].into_iter());
    assert_eq!(sum.byte_len, 7);
    assert_eq!(sum.char_len, 7);
    assert_eq!(sum.line_count, 2); // 1 + 1
}

// ─── Chunking ───────────────────────────────────────────────────────────────

#[test]
fn chunk_text_small() {
    let chunks = chunk_text("hello");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "hello");
}

#[test]
fn chunk_text_newline_aligned() {
    let line = "a line of text here!\n";
    let text: String = line.repeat(100);
    let chunks = chunk_text(&text);

    // Every chunk except the last should end with '\n'
    for (i, chunk) in chunks.iter().enumerate() {
        if i < chunks.len() - 1 {
            assert!(
                chunk.ends_with('\n'),
                "chunk {i} doesn't end with newline: {:?}",
                &chunk[chunk.len().saturating_sub(20)..]
            );
        }
        // Every chunk should be <= MAX_CHUNK_BYTES (unless it's a single long line)
        // Since our test lines are short, all should fit
        assert!(chunk.len() <= MAX_CHUNK_BYTES, "chunk {i} too large: {} bytes", chunk.len());
    }

    // Concatenation equals original
    let reconstructed: String = chunks.into_iter().collect();
    assert_eq!(reconstructed, text);
}

#[test]
fn chunk_text_oversized_line() {
    // A single line longer than MAX_CHUNK_BYTES
    let long_line: String = "x".repeat(MAX_CHUNK_BYTES + 500);
    let chunks = chunk_text(&long_line);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], long_line);
}

// ─── B-tree structure ───────────────────────────────────────────────────────

#[test]
fn build_tree_single_leaf() {
    let leaves = vec![RopeNode::new_leaf("hello".to_string())];
    let root = build_tree(leaves);
    assert!(root.is_leaf());
}

#[test]
fn build_tree_multiple_leaves() {
    let leaves: Vec<Arc<RopeNode>> = (0..20)
        .map(|i| RopeNode::new_leaf(format!("leaf {i}\n")))
        .collect();
    let root = build_tree(leaves);
    // Root should be internal
    assert!(!root.is_leaf());
    // Total line count should be 20
    assert_eq!(root.metrics.line_count, 20);
}

// ─── Alignment invariant ────────────────────────────────────────────────────

/// Check that no non-last leaf in the tree lacks a trailing newline.
fn check_alignment(node: &RopeNode, is_rightmost: bool) -> bool {
    match &node.kind {
        NodeKind::Leaf { text } => {
            if text.is_empty() {
                return true;
            }
            // Non-rightmost leaves must end with '\n'
            is_rightmost || text.ends_with('\n')
        }
        NodeKind::Internal { children } => {
            for (i, child) in children.iter().enumerate() {
                let child_rightmost = is_rightmost && i == children.len() - 1;
                if !check_alignment(child, child_rightmost) {
                    return false;
                }
            }
            true
        }
    }
}

#[test]
fn alignment_after_construction() {
    use std::fmt::Write;
    let mut text = String::new();
    for i in 0..50 {
        writeln!(text, "line {i}").unwrap();
    }
    let r = Rope::from_str(&text);
    assert!(check_alignment(&r.root, true), "alignment broken after construction");
}

#[test]
fn alignment_after_insert() {
    let r = Rope::from_str("abc\ndef\nghi");
    let r2 = r.insert(4, "XYZ\n");
    assert!(check_alignment(&r2.root, true), "alignment broken after insert");
    assert_eq!(r2.content(), "abc\nXYZ\ndef\nghi");
}

#[test]
fn alignment_after_remove_newline() {
    let r = Rope::from_str("abc\ndef\nghi");
    // Remove the first newline
    let r2 = r.remove(3..4);
    assert_eq!(r2.content(), "abcdef\nghi");
    assert!(check_alignment(&r2.root, true), "alignment broken after removing newline");
}

// ─── Stress tests ───────────────────────────────────────────────────────────

#[test]
fn stress_sequential_inserts() {
    let mut r = Rope::new();
    for i in 0..100 {
        let text = format!("line {i}\n");
        r = r.insert(r.byte_len(), &text);
    }
    // 100 lines + trailing empty line
    assert_eq!(r.line_count(), 101);
    for i in 0..100 {
        assert_eq!(r.line(i), Some(format!("line {i}").as_str()));
    }
}

#[test]
fn stress_sequential_removes() {
    use std::fmt::Write;
    let mut text = String::new();
    for i in 0..100 {
        writeln!(text, "line {i}").unwrap();
    }
    let mut r = Rope::from_str(&text);

    // Remove first line repeatedly
    for _ in 0..100 {
        if r.is_empty() {
            break;
        }
        let first_line_bytes = r.line(0).map_or(0, |l| l.len() + 1); // +1 for newline
        r = r.remove(0..first_line_bytes);
    }
    assert!(r.is_empty());
}

#[test]
fn stress_insert_remove_mixed() {
    let mut r = Rope::from_str("initial");
    for i in 0..50 {
        r = r.insert(0, &format!("prefix{i}\n"));
        r = r.insert(r.byte_len(), &format!("\nsuffix{i}"));
    }
    // Should still be valid
    let content = r.content();
    assert!(content.starts_with("prefix49\n"));
    assert!(content.ends_with("\nsuffix49"));
}

// ─── Edge cases ─────────────────────────────────────────────────────────────

#[test]
fn single_char() {
    let r = Rope::from_str("x");
    assert_eq!(r.line_count(), 1);
    assert_eq!(r.line(0), Some("x"));
    assert_eq!(r.byte_len(), 1);
    assert_eq!(r.char_len(), 1);
}

#[test]
fn single_newline() {
    let r = Rope::from_str("\n");
    assert_eq!(r.line_count(), 2);
    assert_eq!(r.line(0), Some(""));
    assert_eq!(r.line(1), Some(""));
}

#[test]
fn empty_lines() {
    let r = Rope::from_str("\n\n\n");
    assert_eq!(r.line_count(), 4);
    for i in 0..4 {
        assert_eq!(r.line(i), Some(""));
    }
}

#[test]
fn insert_past_end_clamps() {
    let r = Rope::from_str("hello");
    let r2 = r.insert(100, " world");
    assert_eq!(r2.content(), "hello world");
}

#[test]
fn remove_past_end_clamps() {
    let r = Rope::from_str("hello");
    let r2 = r.remove(3..100);
    assert_eq!(r2.content(), "hel");
}

#[test]
fn default_is_empty() {
    let r = Rope::default();
    assert!(r.is_empty());
}

#[test]
fn position_on_empty() {
    let r = Rope::new();
    assert_eq!(r.position_to_byte(0, 0), 0);
    assert_eq!(r.byte_to_position(0), (0, 0));
    assert_eq!(r.char_to_byte(0), 0);
    assert_eq!(r.byte_to_char(0), 0);
}

// ─── Property: line(i) matches str::lines().nth(i) ─────────────────────────

#[test]
fn lines_match_split_newline() {
    // Rope uses line-separator semantics: split('\n'), NOT str::lines().
    // "abc\n" → ["abc", ""], not ["abc"].
    let texts = [
        "hello\nworld",
        "abc",
        "abc\ndef\nghi\n",
        "\n\n\n",
        "single",
        "a\nb\nc\nd\ne\nf",
    ];
    for text in texts {
        let r = Rope::from_str(text);
        let expected: Vec<&str> = text.split('\n').collect();
        for (i, &exp) in expected.iter().enumerate() {
            assert_eq!(r.line(i), Some(exp), "line {i} mismatch for text {text:?}");
        }
        assert_eq!(r.line_count(), expected.len(), "line_count mismatch for text {text:?}");
    }
    // Empty rope: 0 lines (special case — split('\n') on "" gives [""])
    let r = Rope::from_str("");
    assert_eq!(r.line_count(), 0);
}

// ─── Big Rope Helpers ────────────────────────────────────────────────────────

/// Create a rope large enough to have internal nodes (3+ levels).
/// With `B_MAX`=8 and `MAX_CHUNK_BYTES`=1024, we need >8 chunks.
/// 2000 short lines (~14KB) produces ~14 chunks → forces 2 levels of
/// internal nodes. For deeper trees, use `make_huge_rope`.
fn make_big_rope() -> (Rope, String) {
    use std::fmt::Write;
    let mut text = String::new();
    for i in 0..2000 {
        writeln!(text, "line {i:04}").unwrap();
    }
    let r = Rope::from_str(&text);
    (r, text)
}

/// Create an even larger rope to force 3-4 levels of internal nodes.
/// With `B_MAX`=8, need >64 leaves for 3 levels and >512 for 4 levels.
/// Each chunk is ~1024 bytes, so ~600KB of text forces ~600 chunks → 4 levels.
fn make_huge_rope() -> (Rope, String) {
    use std::fmt::Write;
    let mut text = String::new();
    // 20_000 lines of ~30 bytes each → ~600KB → ~600 leaves → 4 levels
    for i in 0..20_000 {
        writeln!(text, "line number {i:05} data here").unwrap();
    }
    let r = Rope::from_str(&text);
    (r, text)
}

// ─── Debug impl ──────────────────────────────────────────────────────────────

#[test]
fn debug_short_content() {
    let r = Rope::from_str("hello\nworld");
    let dbg = format!("{r:?}");
    assert!(dbg.contains("Rope"), "should contain struct name");
    assert!(dbg.contains("byte_len"), "should contain byte_len field");
    assert!(dbg.contains("line_count"), "should contain line_count field");
    assert!(dbg.contains("hello\\nworld"), "should contain content preview");
}

#[test]
fn debug_long_content_truncated() {
    // Create content >80 bytes to trigger the truncation path
    let long_line = "a".repeat(120);
    let r = Rope::from_str(&long_line);
    let dbg = format!("{r:?}");
    assert!(dbg.contains("..."), "long content should be truncated with ...");
    assert!(dbg.contains("byte_len"), "should contain byte_len field");
}

// ─── PartialEq ───────────────────────────────────────────────────────────────

#[test]
fn eq_different_byte_len_returns_false() {
    let r1 = Rope::from_str("a");
    let r2 = Rope::from_str("ab");
    assert_ne!(r1, r2);
}

#[test]
fn eq_same_byte_len_different_content() {
    let r1 = Rope::from_str("abc");
    let r2 = Rope::from_str("xyz");
    assert_ne!(r1, r2);
}

#[test]
fn eq_identical_multi_chunk_ropes() {
    let (r1, text) = make_big_rope();
    let r2 = Rope::from_str(&text);
    assert_eq!(r1, r2);
}

#[test]
fn eq_multi_chunk_different_content() {
    use std::fmt::Write;
    let mut text1 = String::new();
    let mut text2 = String::new();
    for i in 0..2000 {
        writeln!(text1, "line {i:04}").unwrap();
        writeln!(text2, "LINE {i:04}").unwrap();
    }
    let r1 = Rope::from_str(&text1);
    let r2 = Rope::from_str(&text2);
    assert_ne!(r1, r2);
}

// ─── Remove edge case: range past content end ────────────────────────────────

#[test]
fn remove_range_fully_past_end() {
    let r = Rope::from_str("hello");
    // After clamping, s=5, e=5, so s >= e → returns clone
    let r2 = r.remove(5..100);
    assert_eq!(r2.content(), "hello");
    assert!(Arc::ptr_eq(&r.root, &r2.root));
}

#[test]
fn remove_range_start_past_end() {
    let r = Rope::from_str("hello");
    // range.start > byte_len, both clamp to 5, s >= e → clone
    let r2 = r.remove(50..100);
    assert_eq!(r2.content(), "hello");
    assert!(Arc::ptr_eq(&r.root, &r2.root));
}

// ─── chunk_text_aligned: oversized chunk + floor_char_boundary ───────────────

#[test]
fn chunk_text_oversized_line_with_newline_after() {
    // A line longer than MAX_CHUNK_BYTES followed by a short line.
    // This forces the "no newline within limit — find the next newline" branch.
    let long_line = "x".repeat(MAX_CHUNK_BYTES + 500);
    let text = format!("{long_line}\nshort\n");
    let chunks = chunk_text(&text);
    // First chunk should be the long line + \n, second "short\n"
    let reconstructed: String = chunks.iter().map(String::as_str).collect();
    assert_eq!(reconstructed, text);
    assert!(chunks.len() >= 2, "should have at least 2 chunks");
    assert!(chunks[0].ends_with('\n'), "first chunk should end with newline");
}

#[test]
fn chunk_text_multibyte_at_boundary() {
    // Create text where multi-byte chars fall near MAX_CHUNK_BYTES boundary.
    // floor_char_boundary must back up past continuation bytes.
    // Each "é" is 2 bytes. Fill close to MAX_CHUNK_BYTES with 2-byte chars,
    // then add a newline, then more content.
    let two_byte_char = "é";
    // Fill ~MAX_CHUNK_BYTES with 2-byte chars (no newlines) → forces oversized chunk
    let count = MAX_CHUNK_BYTES / 2 + 10; // slightly over
    let long_segment: String = two_byte_char.repeat(count);
    let text = format!("{long_segment}\nshort\n");
    let chunks = chunk_text(&text);
    let reconstructed: String = chunks.iter().map(String::as_str).collect();
    assert_eq!(reconstructed, text, "multibyte roundtrip must preserve text");
    // Ensure all chunk boundaries are valid UTF-8 (the fact that we get here proves it)
    for (i, chunk) in chunks.iter().enumerate() {
        // std::str is always valid UTF-8
        assert!(!chunk.is_empty(), "chunk {i} should not be empty");
    }
}

#[test]
fn chunk_text_no_newline_oversized() {
    // No newlines at all, exceeds MAX_CHUNK_BYTES → single oversized chunk
    let text = "a".repeat(MAX_CHUNK_BYTES * 3);
    let chunks = chunk_text(&text);
    assert_eq!(chunks.len(), 1, "no-newline text should be one chunk");
    assert_eq!(chunks[0], text);
}

// ─── nodes_to_root branches ──────────────────────────────────────────────────

#[test]
fn nodes_to_root_zero_nodes() {
    // Removing everything from a rope exercises the `nodes.is_empty()` path
    // in `Rope::remove`, which calls `Rope::new()` (not nodes_to_root).
    // But `nodes_to_root` with 0 nodes returns an empty leaf.
    let r = Rope::from_str("hello");
    let r2 = r.remove(0..5);
    assert!(r2.is_empty());
    assert_eq!(r2.content(), "");
}

#[test]
fn nodes_to_root_single_node() {
    // A small insert that produces a single leaf replacement
    let r = Rope::from_str("ab");
    let r2 = r.insert(1, "X");
    assert_eq!(r2.content(), "aXb");
}

#[test]
fn nodes_to_root_few_nodes() {
    // Insert enough text to produce 2-B_MAX replacement nodes
    let r = Rope::from_str("hello");
    let insert_text = "x\n".repeat(500); // several chunks
    let r2 = r.insert(3, &insert_text);
    let expected = format!("hel{insert_text}lo");
    assert_eq!(r2.content(), expected);
}

#[test]
fn nodes_to_root_many_nodes() {
    // Insert a huge amount of text to exceed B_MAX replacement nodes
    let r = Rope::from_str("hello");
    let insert_text = "line\n".repeat(5000); // many chunks > B_MAX
    let r2 = r.insert(3, &insert_text);
    let expected = format!("hel{insert_text}lo");
    assert_eq!(r2.content(), expected);
}

// ─── Internal node navigation: line_at ───────────────────────────────────────

#[test]
fn line_at_internal_nodes() {
    let (r, text) = make_big_rope();
    let expected: Vec<&str> = text.split('\n').collect();
    // Check lines throughout the rope (beginning, middle, end)
    for &idx in &[0, 1, 100, 500, 999, 1500, 1999] {
        assert_eq!(r.line(idx), Some(expected[idx]), "line {idx} mismatch on big rope");
    }
    // Last line is empty (trailing \n)
    assert_eq!(r.line(2000), Some(""));
    // Past end
    assert_eq!(r.line(2001), None);
}

// ─── Internal node navigation: pos_to_byte ───────────────────────────────────

#[test]
fn pos_to_byte_internal_nodes() {
    let (r, text) = make_big_rope();
    let lines: Vec<&str> = text.split('\n').collect();
    // Verify several lines
    for &line_idx in &[0, 50, 500, 1000, 1999] {
        let byte_offset = r.position_to_byte(line_idx, 0);
        // Compute expected offset manually
        let expected: usize = lines[..line_idx].iter().map(|l| l.len() + 1).sum();
        assert_eq!(byte_offset, expected, "pos_to_byte start of line {line_idx}");
    }
    // Also check a non-zero column
    let byte_at_col3 = r.position_to_byte(100, 3);
    let line_start: usize = lines[..100].iter().map(|l| l.len() + 1).sum();
    assert_eq!(byte_at_col3, line_start + 3, "pos_to_byte line 100 col 3");
}

// ─── Internal node navigation: byte_to_pos ───────────────────────────────────

#[test]
fn byte_to_pos_internal_nodes() {
    let (r, text) = make_big_rope();
    let lines: Vec<&str> = text.split('\n').collect();
    // Test several byte offsets
    let mut byte_offset = 0usize;
    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx > 1999 {
            break;
        }
        if line_idx % 200 == 0 {
            let (l, c) = r.byte_to_position(byte_offset);
            assert_eq!((l, c), (line_idx, 0), "byte_to_pos at start of line {line_idx}");
            // Also test middle of line
            if !line.is_empty() {
                let mid = byte_offset + line.len() / 2;
                let (ml, mc) = r.byte_to_position(mid);
                assert_eq!(ml, line_idx, "byte_to_pos mid line {line_idx} wrong line");
                assert_eq!(mc, line.len() / 2, "byte_to_pos mid line {line_idx} wrong col");
            }
        }
        byte_offset += line.len() + 1; // +1 for \n
    }
    // Test byte offset at end of text
    let (l, c) = r.byte_to_position(text.len());
    assert_eq!((l, c), (2000, 0), "byte_to_pos at end of text");
}

// ─── Internal node navigation: char_to_byte ──────────────────────────────────

#[test]
fn char_to_byte_internal_nodes() {
    let (r, text) = make_big_rope();
    // For ASCII-only content, char offset == byte offset
    for &offset in &[0, 100, 500, 5000, 10000] {
        assert_eq!(r.char_to_byte(offset), offset, "char_to_byte({offset}) on ASCII big rope");
    }
    // Also test at the end
    assert_eq!(r.char_to_byte(text.len()), text.len());
}

#[test]
fn char_to_byte_internal_nodes_unicode() {
    use std::fmt::Write;
    // Build a big rope with multi-byte chars to test non-trivial conversion
    let mut text = String::new();
    for i in 0..2000 {
        writeln!(text, "línea {i:04}").unwrap(); // "í" is 2 bytes
    }
    let r = Rope::from_str(&text);
    // Verify roundtrip for several char offsets
    for &ci in &[0, 10, 100, 1000, 5000] {
        let byte = r.char_to_byte(ci);
        let back = r.byte_to_char(byte);
        assert_eq!(back, ci, "char/byte roundtrip failed for char {ci} on big unicode rope");
    }
}

// ─── Internal node navigation: byte_to_char ──────────────────────────────────

#[test]
fn byte_to_char_internal_nodes() {
    let (r, text) = make_big_rope();
    // ASCII: byte offset == char offset
    for &offset in &[0, 100, 500, 5000, 10000] {
        assert_eq!(r.byte_to_char(offset), offset, "byte_to_char({offset}) on ASCII big rope");
    }
    assert_eq!(r.byte_to_char(text.len()), text.len());
}

// ─── Insert on big rope (internal node splitting) ────────────────────────────

#[test]
fn insert_into_big_rope_beginning() {
    let (r, text) = make_big_rope();
    let r2 = r.insert(0, "PREFIX\n");
    assert_eq!(r2.content(), format!("PREFIX\n{text}"));
    assert!(check_alignment(&r2.root, true), "alignment broken after insert at beginning");
}

#[test]
fn insert_into_big_rope_middle() {
    let (r, text) = make_big_rope();
    let mid = text.len() / 2;
    // Find nearest newline to avoid splitting mid-line
    let insert_pos = text[..mid].rfind('\n').unwrap() + 1;
    let inserted = "INSERTED LINE\n";
    let r2 = r.insert(insert_pos, inserted);
    let expected = format!("{}{inserted}{}", &text[..insert_pos], &text[insert_pos..]);
    assert_eq!(r2.content(), expected);
    assert!(check_alignment(&r2.root, true), "alignment broken after insert in middle");
}

#[test]
fn insert_into_big_rope_end() {
    let (r, text) = make_big_rope();
    let r2 = r.insert(text.len(), "SUFFIX");
    assert_eq!(r2.content(), format!("{text}SUFFIX"));
}

#[test]
fn insert_large_text_into_big_rope() {
    use std::fmt::Write;
    // Insert enough text to cause internal node splitting (> B_MAX children)
    let (r, text) = make_big_rope();
    let mut large_insert = String::new();
    for i in 0..500 {
        writeln!(large_insert, "inserted {i:04}").unwrap();
    }
    let insert_pos = 100;
    let r2 = r.insert(insert_pos, &large_insert);
    let expected = format!("{}{large_insert}{}", &text[..insert_pos], &text[insert_pos..]);
    assert_eq!(r2.content(), expected);
    assert!(check_alignment(&r2.root, true), "alignment broken after large insert");
}

// ─── Remove on big rope (internal node mutation) ─────────────────────────────

#[test]
fn remove_from_big_rope_beginning() {
    let (r, text) = make_big_rope();
    // Remove first 500 bytes
    let r2 = r.remove(0..500);
    assert_eq!(r2.content(), &text[500..]);
    assert!(check_alignment(&r2.root, true), "alignment broken after remove from beginning");
}

#[test]
fn remove_from_big_rope_middle() {
    let (r, text) = make_big_rope();
    let start = 3000;
    let end = 6000;
    let r2 = r.remove(start..end);
    let expected = format!("{}{}", &text[..start], &text[end..]);
    assert_eq!(r2.content(), expected);
    assert!(check_alignment(&r2.root, true), "alignment broken after remove from middle");
}

#[test]
fn remove_from_big_rope_end() {
    let (r, text) = make_big_rope();
    let start = text.len() - 500;
    let r2 = r.remove(start..text.len());
    assert_eq!(r2.content(), &text[..start]);
    assert!(check_alignment(&r2.root, true), "alignment broken after remove from end");
}

#[test]
fn remove_entire_big_rope() {
    let (r, text) = make_big_rope();
    let r2 = r.remove(0..text.len());
    assert!(r2.is_empty());
    assert_eq!(r2.content(), "");
}

#[test]
fn remove_across_many_children() {
    // Remove a large range that spans multiple internal node children
    let (r, text) = make_big_rope();
    let start = 1000;
    let end = text.len() - 1000;
    let r2 = r.remove(start..end);
    let expected = format!("{}{}", &text[..start], &text[end..]);
    assert_eq!(r2.content(), expected);
    assert!(
        check_alignment(&r2.root, true),
        "alignment broken after removing across many children"
    );
}

#[test]
fn remove_single_child_entirely() {
    // Remove a range that exactly covers one internal child's range.
    // This exercises the "entirely within range — remove" branch.
    let (r, _text) = make_big_rope();
    // Remove a chunk-sized range from the middle
    let r2 = r.remove(1024..2048);
    assert!(check_alignment(&r2.root, true));
    assert_eq!(r2.byte_len(), r.byte_len() - 1024);
}

// ─── fixup_alignment ─────────────────────────────────────────────────────────

#[test]
fn fixup_alignment_after_newline_removal() {
    // Removing a newline from the middle of a big rope breaks the
    // newline-alignment invariant, which fixup_alignment must repair.
    let (r, text) = make_big_rope();
    // Find a newline somewhere in the middle
    let mid = text.len() / 2;
    let nl_pos = text[..mid].rfind('\n').unwrap();
    let r2 = r.remove(nl_pos..nl_pos + 1);
    // The content should have that newline removed
    let expected = format!("{}{}", &text[..nl_pos], &text[nl_pos + 1..]);
    assert_eq!(r2.content(), expected);
    // Alignment invariant must hold
    assert!(
        check_alignment(&r2.root, true),
        "alignment broken after removing newline in big rope"
    );
}

#[test]
fn fixup_alignment_multiple_newline_removals() {
    // Remove multiple newlines to stress fixup_alignment
    let (mut r, text) = make_big_rope();
    let mut expected = text;
    // Remove 10 newlines from various positions
    for _ in 0..10 {
        if let Some(nl_pos) = expected[..expected.len() / 2].rfind('\n') {
            r = r.remove(nl_pos..nl_pos + 1);
            expected = format!("{}{}", &expected[..nl_pos], &expected[nl_pos + 1..]);
            assert_eq!(r.content(), expected, "content mismatch after newline removal");
            assert!(
                check_alignment(&r.root, true),
                "alignment broken during serial newline removals"
            );
        }
    }
}

// ─── Huge rope: deep internal nodes ──────────────────────────────────────────

#[test]
fn huge_rope_line_access() {
    let (r, text) = make_huge_rope();
    let lines: Vec<&str> = text.split('\n').collect();
    // Spot-check lines at various positions
    for &idx in &[0, 1, 1000, 5000, 10000, 15000, 19999] {
        assert_eq!(r.line(idx), Some(lines[idx]), "line {idx} mismatch on huge rope");
    }
}

#[test]
fn huge_rope_position_roundtrip() {
    let (r, text) = make_huge_rope();
    let lines: Vec<&str> = text.split('\n').collect();
    // Roundtrip several positions
    let mut byte_offset = 0usize;
    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx >= 20000 {
            break;
        }
        if line_idx % 2000 == 0 {
            let byte = r.position_to_byte(line_idx, 0);
            assert_eq!(byte, byte_offset, "pos_to_byte line {line_idx} on huge rope");
            let (l, c) = r.byte_to_position(byte);
            assert_eq!((l, c), (line_idx, 0), "byte_to_pos line {line_idx} on huge rope");
        }
        byte_offset += line.len() + 1;
    }
}

#[test]
fn huge_rope_char_byte_roundtrip() {
    let (r, text) = make_huge_rope();
    // Test at several char offsets (ASCII so char==byte)
    for &ci in &[0, 1000, 50_000, 200_000, text.len()] {
        let byte = r.char_to_byte(ci);
        let back = r.byte_to_char(byte);
        assert_eq!(back, ci, "char/byte roundtrip failed at {ci} on huge rope");
    }
}

#[test]
fn huge_rope_insert_and_remove() {
    let (r, text) = make_huge_rope();
    // Insert in the middle
    let mid = text.len() / 2;
    let insert_pos = text[..mid].rfind('\n').unwrap() + 1;
    let r2 = r.insert(insert_pos, "HUGE INSERT\n");
    let expected = format!("{}{}{}", &text[..insert_pos], "HUGE INSERT\n", &text[insert_pos..]);
    assert_eq!(r2.content(), expected);

    // Remove the inserted text from the result
    let inserted_len = "HUGE INSERT\n".len();
    let r3 = r2.remove(insert_pos..insert_pos + inserted_len);
    assert_eq!(r3.content(), text, "insert/remove roundtrip on huge rope");
}

// ─── ChunksIter: empty chunk skip ────────────────────────────────────────────

#[test]
fn chunks_skip_empty_leaves() {
    // After removing all content from a leaf, the resulting empty leaf
    // should be skipped by chunks iterator. We can verify this indirectly:
    // build a rope, remove some content, and verify chunks concatenation.
    let (r, text) = make_big_rope();
    let r2 = r.remove(0..500);
    let from_chunks: String = r2.chunks().collect();
    assert_eq!(from_chunks, &text[500..]);
    // Verify no empty chunks in the iteration
    for (i, chunk) in r2.chunks().enumerate() {
        assert!(!chunk.is_empty(), "chunk {i} should not be empty");
    }
}

// ─── find_child_for_byte edge cases ──────────────────────────────────────────

#[test]
fn insert_at_every_line_boundary_of_big_rope() {
    // This exercises find_child_for_byte at various offsets including
    // exact child boundaries
    let (r, text) = make_big_rope();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut offset = 0usize;
    for (i, line) in lines.iter().enumerate().take(100) {
        let r2 = r.insert(offset, "X");
        let expected_start = &text[..offset];
        let expected_end = &text[offset..];
        assert_eq!(
            r2.content(),
            format!("{expected_start}X{expected_end}"),
            "insert at line {i} boundary failed"
        );
        offset += line.len() + 1;
    }
}

// ─── split_children ──────────────────────────────────────────────────────────

#[test]
fn repeated_inserts_force_splits() {
    // Start with a big rope and keep inserting to force repeated splits
    let (mut r, _) = make_big_rope();
    for i in 0..200 {
        let text = format!("ins{i:04}\n");
        r = r.insert(0, &text);
    }
    // Verify content integrity
    let content = r.content();
    assert!(content.starts_with("ins0199\n"));
    assert!(check_alignment(&r.root, true));
}

// ─── Interaction: insert + remove on internal nodes ──────────────────────────

#[test]
fn insert_remove_roundtrip_big_rope() {
    let (r, text) = make_big_rope();
    let insert_text = "INSERTED CONTENT\n";
    let pos = 5000;
    let r2 = r.insert(pos, insert_text);
    let r3 = r2.remove(pos..pos + insert_text.len());
    assert_eq!(r3.content(), text, "insert/remove roundtrip on big rope");
}

// ─── last_byte_of and fixup_alignment with internal children ─────────────────

#[test]
fn remove_spanning_internal_boundaries() {
    // Remove a range that spans from the middle of one internal child
    // to the middle of another, forcing fixup_alignment on internal nodes
    let (r, text) = make_huge_rope();
    // Remove a large range spanning multiple internal children
    let start = 10_000;
    let end = 100_000;
    let r2 = r.remove(start..end);
    let expected = format!("{}{}", &text[..start], &text[end..]);
    assert_eq!(r2.content(), expected);
    assert!(
        check_alignment(&r2.root, true),
        "alignment broken after removing across internal boundaries"
    );
}

// ─── Coverage: ChunksIter::next() empty-leaf continue (line 408) ─────────────

#[test]
fn chunks_iter_skips_empty_leaf_node() {
    // Construct a rope with an empty leaf directly using internal APIs.
    // An internal node holding [empty_leaf, real_leaf] should yield
    // only the real leaf's text from the chunks iterator.
    let empty_leaf = RopeNode::new_leaf(String::new());
    let real_leaf = RopeNode::new_leaf("hello".to_string());
    let root = RopeNode::new_internal(vec![empty_leaf, real_leaf]);
    let rope = Rope { root };

    let chunks: Vec<&str> = rope.chunks().collect();
    assert_eq!(chunks, vec!["hello"], "empty leaf should be skipped");
}

#[test]
fn chunks_iter_multiple_empty_leaves() {
    // Multiple empty leaves interspersed with real content
    let leaves = vec![
        RopeNode::new_leaf(String::new()),
        RopeNode::new_leaf("aaa\n".to_string()),
        RopeNode::new_leaf(String::new()),
        RopeNode::new_leaf("bbb".to_string()),
        RopeNode::new_leaf(String::new()),
    ];
    let root = RopeNode::new_internal(leaves);
    let rope = Rope { root };

    let chunks: Vec<&str> = rope.chunks().collect();
    assert_eq!(chunks, vec!["aaa\n", "bbb"]);
}

// ─── PartialEq with different internal structure ────────────────────────────

#[test]
fn eq_multi_chunk_different_boundaries_same_content() {
    // Two ropes with identical content but different tree structure.
    // Rope 1: from_str (balanced tree).
    // Rope 2: built by repeated inserts (different chunk boundaries).
    use std::fmt::Write;
    let mut text = String::new();
    for i in 0..200 {
        writeln!(text, "line {i:04}").unwrap();
    }
    let r1 = Rope::from_str(&text);
    // Build r2 by inserting line by line
    let mut r2 = Rope::new();
    for i in 0..200 {
        let line = format!("line {i:04}\n");
        r2 = r2.insert(r2.byte_len(), &line);
    }
    assert_eq!(r1.content(), r2.content(), "content should match");
    assert_eq!(r1, r2, "PartialEq should succeed despite different tree structure");
}

#[test]
fn eq_left_exhausted_right_has_more() {
    // Left has fewer chunks than right (same content, different structure).
    let left = Rope {
        root: RopeNode::new_internal(vec![RopeNode::new_leaf("abc".to_string())]),
    };
    let right = Rope {
        root: RopeNode::new_internal(vec![
            RopeNode::new_leaf("ab".to_string()),
            RopeNode::new_leaf("c".to_string()),
        ]),
    };
    // Same byte_len (3), same content — should be equal
    assert_eq!(left, right);
}

#[test]
fn eq_right_exhausted_left_has_remaining() {
    // Right has fewer chunks than left (same content, different structure).
    let left = Rope {
        root: RopeNode::new_internal(vec![
            RopeNode::new_leaf("ab".to_string()),
            RopeNode::new_leaf("c".to_string()),
        ]),
    };
    let right = Rope {
        root: RopeNode::new_internal(vec![RopeNode::new_leaf("abc".to_string())]),
    };
    assert_eq!(left, right);
}

#[test]
fn eq_left_exhausted_right_not_empty() {
    // left exhausted, r_remaining not empty → should return false
    // Need same byte_len but different content. Both 3 bytes.
    let left = Rope {
        root: RopeNode::new_leaf("abc".to_string()),
    };
    // Right: "ab" + "d" = 3 bytes but "abd" != "abc"
    let right = Rope {
        root: RopeNode::new_internal(vec![
            RopeNode::new_leaf("ab".to_string()),
            RopeNode::new_leaf("d".to_string()),
        ]),
    };
    // byte_len is same (3), but content differs
    assert_ne!(left, right);
}

// ─── Coverage: floor_char_boundary backtrack (lines 540, 546, 547) ────────────

#[test]
fn floor_char_boundary_at_multibyte_boundary() {
    // 4-byte char (emoji) placed so MAX_CHUNK_BYTES falls in the middle of it.
    // floor_char_boundary must backtrack past continuation bytes.
    let emoji = "\u{1F600}"; // 4 bytes
    assert_eq!(emoji.len(), 4);

    // Create a string where a multi-byte char straddles a position.
    // "a".repeat(1021) + emoji (4 bytes) = 1025 bytes.
    // floor_char_boundary(s, 1024) should land at 1021 (start of emoji).
    let s = format!("{}{emoji}rest", "a".repeat(1021));
    let result = floor_char_boundary(&s, 1024);
    assert_eq!(result, 1021, "should backtrack to start of 4-byte char");

    // Also test 2-byte char at boundary
    let s2 = format!("{}{}tail", "b".repeat(1023), "\u{00E9}"); // 1023 + 2 = 1025
    let result2 = floor_char_boundary(&s2, 1024);
    assert_eq!(result2, 1023, "should backtrack to start of 2-byte char");
}

#[test]
fn floor_char_boundary_past_end() {
    // Exercise the `byte_idx >= s.len()` early return (line 540)
    let s = "hello";
    let result = floor_char_boundary(s, 100);
    assert_eq!(result, 5, "should return s.len() when byte_idx >= len");
}

#[test]
fn floor_char_boundary_at_ascii() {
    // When byte_idx lands on an ASCII char, no backtracking needed
    let s = "hello\u{00E9}world";
    let result = floor_char_boundary(s, 3);
    assert_eq!(result, 3, "ASCII boundary needs no backtracking");
}

// ─── Coverage: build_tree single-node exit (line 586) ─────────────────────────

#[test]
fn build_tree_exactly_bmax_leaves() {
    // B_MAX = 8 leaves → build_tree should create one internal node,
    // then on the next loop iteration nodes.len() == 1 → line 585-586
    let leaves: Vec<Arc<RopeNode>> = (0..B_MAX)
        .map(|i| RopeNode::new_leaf(format!("leaf{i}\n")))
        .collect();
    let root = build_tree(leaves);
    assert!(!root.is_leaf(), "B_MAX leaves should produce internal node");
    if let NodeKind::Internal { children } = &root.kind {
        assert_eq!(children.len(), B_MAX);
    }
}

#[test]
fn build_tree_bmax_plus_one_leaves() {
    // B_MAX + 1 = 9 leaves → must split into two groups.
    // After grouping: 2 internal nodes → loop again → nodes.len() == 2 <= B_MAX
    // → wraps in final internal node (line 588).
    let leaves: Vec<Arc<RopeNode>> = (0..=B_MAX)
        .map(|i| RopeNode::new_leaf(format!("leaf{i}\n")))
        .collect();
    let root = build_tree(leaves);
    assert!(!root.is_leaf());
    // Total line count should be B_MAX + 1
    assert_eq!(root.metrics.line_count, B_MAX + 1);
}

// ─── Coverage: nodes_to_root 0-node branch (line 595) ─────────────────────────

#[test]
fn nodes_to_root_empty_vec() {
    // Directly test nodes_to_root with empty vec
    let root = nodes_to_root(vec![]);
    assert!(root.is_leaf());
    assert_eq!(root.metrics.byte_len, 0);
}

// ─── Coverage: line_at Internal node (lines 630-631, 642) ─────────────────────

#[test]
fn line_at_past_last_line_in_internal_node() {
    // Create a rope with internal nodes and query a line past the last child.
    // This exercises the `None` return (line 642) in the Internal branch.
    let (r, _) = make_big_rope();
    // line_count is 2001 (2000 lines + trailing empty). line(2001) is past end.
    assert_eq!(r.line(2001), None);
}

#[test]
fn line_at_leaf_past_lines_returns_none() {
    // Exercise the `None` return (line 631) in the Leaf branch of line_at.
    // A leaf with "hello" has line_count=1, so requesting line 1 should return None.
    let node = RopeNode::new_leaf("hello".to_string());
    assert_eq!(line_at(&node, 1), None);
}

#[test]
fn line_at_leaf_no_trailing_newline_last_segment() {
    // Exercise lines 628-630: the last segment (no trailing newline) path.
    // A leaf "abc\ndef" has 2 lines. Requesting line 1 should return "def".
    let node = RopeNode::new_leaf("abc\ndef".to_string());
    assert_eq!(line_at(&node, 0), Some("abc"));
    assert_eq!(line_at(&node, 1), Some("def")); // lines 628-630
    assert_eq!(line_at(&node, 2), None); // line 631
}

// ─── Coverage: pos_to_byte Internal node (lines 679-680, 692) ─────────────────

#[test]
fn pos_to_byte_past_last_line_in_internal() {
    // Exercise the fallthrough `offset` return (line 692) in Internal branch.
    // Query a line past all children → falls through and returns total byte_len.
    let (r, text) = make_big_rope();
    // Line 99999 doesn't exist, so pos_to_byte should clamp/fall through.
    let result = r.position_to_byte(99999, 0);
    assert_eq!(result, text.len(), "past-end line should return total byte_len");
}

#[test]
fn pos_to_byte_leaf_past_lines_returns_text_len() {
    // Exercise line 680: leaf case where current_line never matches target_line.
    let node = RopeNode::new_leaf("hello".to_string());
    // target_line=5, but leaf only has line 0 → falls through to text.len()
    let result = pos_to_byte(&node, 5, 0);
    assert_eq!(result, 5);
}

// ─── Coverage: remove_range empty range on internal (lines 831-837) ───────────

#[test]
fn remove_range_empty_on_internal_node() {
    // Calling remove_range with an empty range on an internal node should
    // clone the node (lines 831-837).
    let (r, _text) = make_big_rope();
    // Rope::remove short-circuits empty ranges before calling remove_range,
    // so we need to call remove_range directly on an internal node.
    let cloned = remove_range(&r.root, 100..100);
    assert_eq!(cloned.len(), 1);
    // Verify the cloned node has the same metrics
    assert_eq!(cloned[0].metrics.byte_len, r.byte_len());
    // Verify it's an internal node (line 835-836)
    assert!(!cloned[0].is_leaf());
    // Also test on a leaf
    let leaf_node = RopeNode::new_leaf("hello".to_string());
    let cloned_leaf = remove_range(&leaf_node, 2..2);
    assert_eq!(cloned_leaf.len(), 1);
    assert!(cloned_leaf[0].is_leaf());
    if let NodeKind::Leaf { text } = &cloned_leaf[0].kind {
        assert_eq!(text, "hello");
    }
}

// ─── remove_range: full leaf removal ────────────────────────────────────────

#[test]
fn remove_range_leaf_full_removal() {
    // Removing all content from a leaf (s=0, e>=len) returns empty vec.
    let leaf = RopeNode::new_leaf("x".to_string());
    let result = remove_range(&leaf, 0..1);
    assert!(result.is_empty(), "removing all content from leaf should return empty vec");
}

// ─── Coverage: find_child_for_byte past-end branch (lines 918-919) ────────────

#[test]
fn find_child_for_byte_past_end() {
    // Create children and query with byte offset past the total size.
    let children = vec![
        RopeNode::new_leaf("aaa\n".to_string()),
        RopeNode::new_leaf("bbb\n".to_string()),
    ];
    // Total bytes = 8. Query at byte 100 (past end).
    let (idx, local) = find_child_for_byte(&children, 100);
    assert_eq!(idx, 1, "should return last child index");
    assert_eq!(local, 4, "should return last child's byte_len");
}

#[test]
fn find_child_for_byte_exact_boundary() {
    // Query at exact boundary between children.
    let children = vec![
        RopeNode::new_leaf("aaa\n".to_string()), // 4 bytes
        RopeNode::new_leaf("bbb\n".to_string()), // 4 bytes
    ];
    // Byte 4 is the start of child 1, but find_child_for_byte uses <=,
    // so byte 4 should be in child 0 (offset 4 <= 0 + 4).
    let (idx, local) = find_child_for_byte(&children, 4);
    assert_eq!(idx, 0, "exact end of child 0 should stay in child 0");
    assert_eq!(local, 4);
}

// ─── Coverage: split_children B_MAX group branch (line 935) ───────────────────

#[test]
fn split_children_many_children() {
    // Create > 2*B_MAX children to trigger the B_MAX group branch (line 935).
    let children: Vec<Arc<RopeNode>> = (0..B_MAX * 3)
        .map(|i| RopeNode::new_leaf(format!("c{i}\n")))
        .collect();
    let result = split_children(&children);
    // Should produce multiple internal nodes
    assert!(result.len() > 1);
    // Total metrics should be preserved
    let total_bytes: usize = result.iter().map(|n| n.metrics.byte_len).sum();
    let expected_bytes: usize = children.iter().map(|n| n.metrics.byte_len).sum();
    assert_eq!(total_bytes, expected_bytes);
}

#[test]
fn split_children_just_over_bmax() {
    // B_MAX + 1 children to trigger the `remaining <= 2 * B_MAX` split (line 932-933).
    let children: Vec<Arc<RopeNode>> = (0..=B_MAX)
        .map(|i| RopeNode::new_leaf(format!("c{i}\n")))
        .collect();
    let result = split_children(&children);
    assert_eq!(result.len(), 2, "should split into 2 groups");
}

// ─── Coverage: fixup_alignment branches (lines 972-975) ──────────────────────

#[test]
fn fixup_alignment_both_leaves() {
    // Test fixup_alignment where both children are leaves and the left
    // doesn't end with newline. This exercises lines 972-973.
    let left = RopeNode::new_leaf("hello".to_string()); // no trailing \n
    let right = RopeNode::new_leaf(" world\n".to_string());
    let mut children = vec![left, right];
    fixup_alignment(&mut children);
    // After fixup, the merged text "hello world\n" should be re-chunked.
    // Since it's small, it stays as one or more leaves.
    let total: String = children
        .iter()
        .map(|n| {
            let mut s = String::new();
            collect_text(n, &mut s);
            s
        })
        .collect();
    assert_eq!(total, "hello world\n");
}

#[test]
fn fixup_alignment_with_internal_child() {
    // Test fixup_alignment where one child is internal. This exercises
    // the `else` branch at line 974-975: wraps in new_internal.
    // Left: internal node (doesn't end with \n).
    let left = RopeNode::new_internal(vec![
        RopeNode::new_leaf("part1\n".to_string()),
        RopeNode::new_leaf("part2".to_string()), // no trailing \n
    ]);
    let right = RopeNode::new_leaf(" more\n".to_string());
    let mut children = vec![left, right];
    fixup_alignment(&mut children);
    let total: String = children
        .iter()
        .map(|n| {
            let mut s = String::new();
            collect_text(n, &mut s);
            s
        })
        .collect();
    assert_eq!(total, "part1\npart2 more\n");
}

#[test]
fn fixup_alignment_many_leaves_from_merge() {
    // Test fixup_alignment where merging two children produces > B_MAX leaves,
    // exercising line 968: `new_leaves.len() > B_MAX → build_tree`.
    // Create two children whose merged text produces many chunks.
    use std::fmt::Write;
    let mut left_text = String::new();
    for i in 0..100 {
        writeln!(left_text, "left line {i:04}").unwrap();
    }
    // Remove trailing newline so left doesn't end with \n
    left_text.pop();
    let mut right_text = String::new();
    for i in 0..100 {
        writeln!(right_text, "right line {i:04}").unwrap();
    }
    let left = RopeNode::new_leaf(left_text.clone());
    let right = RopeNode::new_leaf(right_text.clone());
    let mut children = vec![left, right];
    fixup_alignment(&mut children);
    let total: String = children
        .iter()
        .map(|n| {
            let mut s = String::new();
            collect_text(n, &mut s);
            s
        })
        .collect();
    assert_eq!(total, format!("{left_text}{right_text}"));
}

// ─── Coverage: deep mutation then query ──────────────────────────────────────

#[test]
fn mutated_rope_line_at_deep_child() {
    // Create a big rope, mutate it, then query lines.
    // The mutation forces re-balancing so queries go through internal nodes.
    let (mut r, _) = make_huge_rope();
    // Insert at various positions to restructure the tree
    for i in 0..20 {
        let pos = i * 1000;
        r = r.insert(pos.min(r.byte_len()), &format!("INSERT{i}\n"));
    }
    // Now query lines throughout — exercises internal node line_at
    for i in (0..r.line_count()).step_by(500) {
        assert!(r.line(i).is_some(), "line {i} should exist after mutations");
    }
}

#[test]
fn mutated_rope_pos_to_byte_deep_child() {
    // After mutations, query pos_to_byte through internal nodes.
    let (mut r, _) = make_huge_rope();
    for i in 0..10 {
        r = r.insert(i * 500, "X\n");
    }
    // Roundtrip several positions
    for line_idx in (0..r.line_count()).step_by(1000) {
        let byte = r.position_to_byte(line_idx, 0);
        let (l, c) = r.byte_to_position(byte);
        assert_eq!((l, c), (line_idx, 0), "roundtrip failed for line {line_idx}");
    }
}

// ─── Coverage: remove_range on internal node → empty range (line 830) ─────────

#[test]
fn remove_range_empty_on_leaf() {
    // Exercise line 830-839 clone path on a leaf node
    let leaf = RopeNode::new_leaf("abc\ndef\n".to_string());
    let result = remove_range(&leaf, 3..3);
    assert_eq!(result.len(), 1);
    if let NodeKind::Leaf { text } = &result[0].kind {
        assert_eq!(text, "abc\ndef\n");
    }
}

// ─── Coverage: insert forcing find_child_for_byte past-end (lines 918-919) ────

#[test]
fn insert_past_end_of_big_rope() {
    // Insert at a byte offset way past the rope's byte_len.
    // Rope::insert clamps to byte_len, but this tests the path.
    let (r, text) = make_big_rope();
    let r2 = r.insert(text.len() + 10000, "PAST_END");
    // Should be clamped to insert at the end
    assert!(r2.content().ends_with("PAST_END"));
    assert_eq!(r2.byte_len(), text.len() + 8);
}

// ─── Coverage: build_tree with > B_MAX leaves after inner loop (line 585) ─────

#[test]
fn build_tree_large_leaf_count() {
    // Create enough leaves to require multiple rounds of grouping.
    // With B_MAX=8, 100 leaves → first pass: ~13 internal nodes → second pass: 2 → root.
    let leaves: Vec<Arc<RopeNode>> = (0..100)
        .map(|i| RopeNode::new_leaf(format!("L{i:03}\n")))
        .collect();
    let root = build_tree(leaves);
    assert!(!root.is_leaf());
    assert_eq!(root.metrics.line_count, 100);
    // Verify content roundtrip
    let mut content = String::new();
    collect_text(&root, &mut content);
    for i in 0..100 {
        assert!(content.contains(&format!("L{i:03}\n")));
    }
}

// ─── Coverage: line_at internal node fallthrough None (line 642) ──────────────

#[test]
fn line_at_internal_node_past_all_children() {
    // Call line_at directly on an internal node with an index beyond
    // all children's line counts. This exercises the `None` return at
    // line 642 (internal node falls through the for loop without matching).
    let internal = RopeNode::new_internal(vec![
        RopeNode::new_leaf("hello\n".to_string()), // line_count = 1
        RopeNode::new_leaf("world".to_string()),   // line_count = 1
    ]);
    // Total line_count = 2. Requesting line 5 should fall through.
    assert_eq!(line_at(&internal, 5), None);
    // Also test just past the end (line 2)
    assert_eq!(line_at(&internal, 2), None);
}

// ─── Coverage: chunk_text empty string (L509:br1 false branch) ────────────────

#[test]
fn chunk_text_empty_string() {
    // chunk_text("") should return an empty vec because the while loop
    // condition `!remaining.is_empty()` is false on the first check.
    let chunks = chunk_text("");
    assert!(chunks.is_empty(), "chunk_text of empty string should return empty vec");
}

// ─── Coverage: fixup_alignment skip when left ends with newline (L959:br1) ────

#[test]
fn fixup_alignment_skips_when_left_ends_with_newline() {
    // When left child ends with '\n', fixup_alignment should NOT merge.
    // This exercises the false branch of line 959:
    // `left_last.is_some() && left_last != Some(b'\n')` → false when left ends '\n'
    let left = RopeNode::new_leaf("hello\n".to_string());
    let right = RopeNode::new_leaf("world".to_string());
    let mut children = vec![left, right];
    fixup_alignment(&mut children);
    // Should remain 2 children (no merge needed)
    assert_eq!(children.len(), 2, "aligned children should not be merged");
    if let NodeKind::Leaf { text } = &children[0].kind {
        assert_eq!(text, "hello\n");
    }
    if let NodeKind::Leaf { text } = &children[1].kind {
        assert_eq!(text, "world");
    }
}

#[test]
fn fixup_alignment_skips_when_left_is_empty() {
    // When left child is empty, left_last is None, so the condition
    // `left_last.is_some() && ...` is false → skip.
    let left = RopeNode::new_leaf(String::new());
    let right = RopeNode::new_leaf("world".to_string());
    let mut children = vec![left, right];
    fixup_alignment(&mut children);
    assert_eq!(children.len(), 2, "empty left should not trigger merge");
}

// ─── PartialEq mismatch paths ───────────────────────────────────────────────

#[test]
fn eq_unequal_ropes_same_bytelen_different_chunk_layout() {
    // Same byte_len, different content, different chunk layout → not equal.
    let left = Rope {
        root: RopeNode::new_internal(vec![
            RopeNode::new_leaf("ab".to_string()),
            RopeNode::new_leaf("cd".to_string()),
        ]),
    };
    let right = Rope {
        root: RopeNode::new_internal(vec![
            RopeNode::new_leaf("ax".to_string()),
            RopeNode::new_leaf("cd".to_string()),
        ]),
    };
    assert_ne!(left, right, "different content should not be equal");
}

// ─── Coverage: fixup_alignment L972:br1/br3 — both-leaf and mixed paths ───────

#[test]
fn fixup_alignment_left_internal_right_internal() {
    // Both children are internal → line 972 condition is false → line 975.
    let left = RopeNode::new_internal(vec![
        RopeNode::new_leaf("aaa\n".to_string()),
        RopeNode::new_leaf("bbb".to_string()), // no trailing \n
    ]);
    let right = RopeNode::new_internal(vec![
        RopeNode::new_leaf("ccc\n".to_string()),
        RopeNode::new_leaf("ddd\n".to_string()),
    ]);
    let mut children = vec![left, right];
    fixup_alignment(&mut children);
    // Left doesn't end with '\n' → merge triggered. Since neither is a leaf,
    // result wraps in new_internal (line 975).
    let total: String = children
        .iter()
        .map(|n| {
            let mut s = String::new();
            collect_text(n, &mut s);
            s
        })
        .collect();
    assert_eq!(total, "aaa\nbbbccc\nddd\n");
}

#[test]
fn fixup_alignment_left_leaf_right_internal() {
    // Left is leaf (no \n), right is internal → line 972: children[i].is_leaf()
    // is true but children[i+1].is_leaf() is false → line 975.
    let left = RopeNode::new_leaf("hello".to_string());
    let right = RopeNode::new_internal(vec![
        RopeNode::new_leaf(" world\n".to_string()),
        RopeNode::new_leaf("more\n".to_string()),
    ]);
    let mut children = vec![left, right];
    fixup_alignment(&mut children);
    let total: String = children
        .iter()
        .map(|n| {
            let mut s = String::new();
            collect_text(n, &mut s);
            s
        })
        .collect();
    assert_eq!(total, "hello world\nmore\n");
}

// ─── Coverage: remove_range internal → no overlap (L889:br1 false) ────────────

#[test]
fn remove_range_internal_no_overlap_with_any_child() {
    // Call remove_range on an internal node where the range is entirely
    // AFTER all children. This exercises the `if modified` false branch (L889).
    // With proper clamping in Rope::remove, this path is unreachable from
    // the public API, but we can test it directly.
    let internal = RopeNode::new_internal(vec![
        RopeNode::new_leaf("aaa\n".to_string()), // 4 bytes, offset 0..4
        RopeNode::new_leaf("bbb\n".to_string()), // 4 bytes, offset 4..8
    ]);
    // Range 8..10 is entirely past both children. The loop condition
    // `range.end <= cs || range.start >= ce` catches every child.
    let result = remove_range(&internal, 8..10);
    // No child overlaps → modified stays false → line 889 false branch.
    // Returns the original children wrapped in a single internal node.
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].metrics.byte_len, 8, "no bytes should be removed");
}

// ─── Coverage: floor_char_boundary with idx=0 (L519:br1) ───────────────────

#[test]
fn floor_char_boundary_at_zero() {
    // byte_idx=0 on a non-empty string: idx starts at 0, while loop
    // condition `idx > 0` is immediately false (L519:br1).
    assert_eq!(floor_char_boundary("abc", 0), 0);
}

// ─── Coverage: fixup_alignment both-non-leaf merge (L942, L945) ─────────────

#[test]
fn fixup_alignment_two_internals_large_merge() {
    // Two adjacent internal nodes whose merged text exceeds MAX_CHUNK_BYTES,
    // producing > 1 leaf. Both children[i] are internal → is_leaf() false
    // at L942 → br1 taken → falls to L945.
    let left_text = "a".repeat(600); // no trailing '\n'
    let right_text = format!("\n{}", "b".repeat(600));
    let left = RopeNode::new_internal(vec![RopeNode::new_leaf(left_text)]);
    let right = RopeNode::new_internal(vec![RopeNode::new_leaf(right_text)]);
    let mut children = vec![left, right];
    fixup_alignment(&mut children);
    // Merged text is "aaa...\nbbb..." (1201 bytes) → chunk_text splits at '\n'
    // → 2 leaves → wrapped in new_internal (L945)
    let mut text = String::new();
    for c in &children {
        collect_text(c, &mut text);
    }
    assert_eq!(text.len(), 1201);
    assert!(text.starts_with("aaa"));
    assert!(text.contains('\n'));
}

#[test]
fn fixup_alignment_leaf_and_internal_large_merge() {
    // Left is leaf, right is internal → children[i].is_leaf() true but
    // children[i+1].is_leaf() false → L942:br3 taken → L945.
    let left_text = "x".repeat(600);
    let right_text = format!("\n{}", "y".repeat(600));
    let left = RopeNode::new_leaf(left_text);
    let right = RopeNode::new_internal(vec![RopeNode::new_leaf(right_text)]);
    let mut children = vec![left, right];
    fixup_alignment(&mut children);
    let mut text = String::new();
    for c in &children {
        collect_text(c, &mut text);
    }
    assert_eq!(text.len(), 1201);
}

// ─── Coverage: remove_range debug_assert branches (L819) ────────────────────

#[test]
#[should_panic(expected = "remove start not on char boundary")]
fn remove_range_panics_on_non_char_start_boundary() {
    // "α" is 2 bytes (0xCE 0xB1). Byte 1 is a continuation byte.
    let leaf = RopeNode::new_leaf("α".to_string());
    remove_range(&leaf, 1..2);
}

#[test]
#[should_panic(expected = "remove end not on char boundary")]
fn remove_range_panics_on_non_char_end_boundary() {
    // "αβ" is 4 bytes. Byte 3 (0xB2) is a continuation byte.
    // Start=0 is valid, end=3 is not on a char boundary.
    let leaf = RopeNode::new_leaf("αβ".to_string());
    remove_range(&leaf, 0..3);
}
