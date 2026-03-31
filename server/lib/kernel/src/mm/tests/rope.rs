use super::*;

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
