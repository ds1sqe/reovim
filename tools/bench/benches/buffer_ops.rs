//! Buffer operation benchmarks — rope-backed (post #711 migration).
//!
//! Measures core buffer operations at 100-100K lines.
//! Key post-rope wins: clone is O(1) via Arc sharing, position conversion
//! is O(log n), insert/delete is O(log n).
//!
//! Run: `cargo bench -p reovim-bench --bench buffer_ops`

use {
    criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main},
    reovim_kernel::api::v1::*,
    reovim_types_text::Position,
};

/// Standard sizes: 100, 1K, 10K, 100K lines.
const SIZES: &[usize] = &[100, 1_000, 10_000, 100_000];

/// Generate a buffer with N lines (~40 chars each).
fn generate_buffer(lines: usize) -> Buffer {
    let content = (0..lines)
        .map(|i| format!("This is line number {} with some content", i))
        .collect::<Vec<_>>()
        .join("\n");
    Buffer::from_string(&content)
}

// =========================================================================
// Mutation benchmarks — expected O(1) with rope, O(n) with Vec<String>
// =========================================================================

fn bench_insert_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/insert_char");
    for &lines in SIZES {
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, &lines| {
            b.iter_batched_ref(
                || generate_buffer(lines),
                |buf| {
                    let pos = Position::new(lines / 2, 10);
                    buf.insert_at(pos, "X");
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_insert_word(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/insert_word");
    for &lines in SIZES {
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, &lines| {
            b.iter_batched_ref(
                || generate_buffer(lines),
                |buf| {
                    let pos = Position::new(lines / 2, 10);
                    buf.insert_at(pos, "benchmark");
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_insert_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/insert_line");
    for &lines in SIZES {
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, &lines| {
            b.iter_batched_ref(
                || generate_buffer(lines),
                |buf| {
                    let pos = Position::new(lines / 2, 0);
                    buf.insert_at(pos, "new line inserted here\n");
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_delete_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/delete_char");
    for &lines in SIZES {
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, &lines| {
            b.iter_batched_ref(
                || generate_buffer(lines),
                |buf| {
                    let pos = Position::new(lines / 2, 10);
                    let _ = buf.delete_at(pos, 1);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_delete_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/delete_range");
    for &lines in SIZES {
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, &lines| {
            b.iter_batched_ref(
                || generate_buffer(lines),
                |buf| {
                    let mid = lines / 2;
                    let _ = buf.delete_range(Position::new(mid, 0), Position::new(mid + 1, 0));
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// =========================================================================
// Read benchmarks — expected O(1) with Vec, O(log n) with rope
// =========================================================================

fn bench_line_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/line_access");
    for &lines in SIZES {
        let buffer = generate_buffer(lines);
        group.bench_with_input(BenchmarkId::new("lines", lines), &buffer, |b, buffer| {
            let mid = lines / 2;
            b.iter(|| {
                let _ = buffer.line(mid);
            });
        });
    }
    group.finish();
}

// =========================================================================
// Materialization benchmarks — the heavy hitters
// =========================================================================

fn bench_content(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/content");
    for &lines in SIZES {
        let buffer = generate_buffer(lines);
        group.bench_with_input(BenchmarkId::new("lines", lines), &buffer, |b, buffer| {
            b.iter(|| {
                let _ = std::hint::black_box(buffer.content());
            });
        });
    }
    group.finish();
}

fn bench_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/clone");
    for &lines in SIZES {
        let buffer = generate_buffer(lines);
        group.bench_with_input(BenchmarkId::new("lines", lines), &buffer, |b, buffer| {
            b.iter(|| {
                let _ = std::hint::black_box(buffer.clone());
            });
        });
    }
    group.finish();
}

fn bench_from_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/from_string");
    for &lines in SIZES {
        let content = (0..lines)
            .map(|i| format!("Line {} content", i))
            .collect::<Vec<_>>()
            .join("\n");
        group.bench_with_input(BenchmarkId::new("lines", lines), &content, |b, content| {
            b.iter(|| {
                let _ = Buffer::from_string(content);
            });
        });
    }
    group.finish();
}

// =========================================================================
// Position conversion — O(lines) with Vec, O(log n) with rope
// =========================================================================

fn bench_position_to_byte(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/position_to_byte");
    for &lines in SIZES {
        let buffer = generate_buffer(lines);
        let target = lines - 1; // worst case: near end of file
        group.bench_with_input(BenchmarkId::new("lines", lines), &buffer, |b, buffer| {
            b.iter(|| {
                let _ = buffer.position_to_byte(Position::new(target, 10));
            });
        });
    }
    group.finish();
}

fn bench_byte_to_position(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/byte_to_position");
    for &lines in SIZES {
        let buffer = generate_buffer(lines);
        let content = buffer.content();
        let target_byte = content.len() * 3 / 4; // 75% into the file
        group.bench_with_input(BenchmarkId::new("lines", lines), &buffer, |b, buffer| {
            b.iter(|| {
                let _ = buffer.byte_to_position(target_byte);
            });
        });
    }
    group.finish();
}

// =========================================================================
// Motion hot path — Vec<char> allocation cost
// =========================================================================

fn bench_vec_char_alloc(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/vec_char_alloc");
    let short = "hello world";
    let medium = "The quick brown fox jumps over the lazy dog. Hello world! More text here for a longer line padding.";
    let long: String = "x".repeat(1000);
    for (name, line) in [
        ("11_chars", short),
        ("99_chars", medium),
        ("1000_chars", &long),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                let _: Vec<char> = std::hint::black_box(line).chars().collect();
            });
        });
    }
    group.finish();
}

/// Simulate a word_forward-like motion: alloc Vec<char>, then scan forward
/// with 5-10 indexed lookups (the typical motion pattern).
fn bench_vec_char_motion_sim(c: &mut Criterion) {
    let mut group = c.benchmark_group("motion/vec_char_scan");
    let line_80 = "fn hello_world(foo: &str, bar: usize) -> Result<String, Error> { let x = 42; }";
    let line_450: String = (0..10)
        .map(|i| format!("let var_{i} = some_function(arg_{i}, other_{i});"))
        .collect::<Vec<_>>()
        .join(" ");
    // 2500 chars — long minified/generated line
    let line_2500: String = (0..55)
        .map(|i| format!("pub fn method_{i}(a: u32, b: &str) -> Option<Vec<u8>> {{ todo!() }}"))
        .collect::<Vec<_>>()
        .join(" ");
    // 10K chars — very long line (minified JS, log output)
    let line_10k: String = (0..220)
        .map(|i| format!("const field_{i} = getValue(config.section_{i}, default_{i});"))
        .collect::<Vec<_>>()
        .join(" ");
    // 50K chars — extreme case (binary-ish, data URIs, huge JSON)
    let line_50k: String = (0..1100)
        .map(|i| format!("data[{i}] = transform(input[{i}], matrix[{i}], offset);"))
        .collect::<Vec<_>>()
        .join(" ");

    let cases: Vec<(&str, &str)> = vec![
        ("80_chars", line_80),
        ("450_chars", &line_450),
        ("2500_chars", &line_2500),
        ("10k_chars", &line_10k),
        ("50k_chars", &line_50k),
    ];

    for (name, line) in &cases {
        // Approach 1: Vec<char> alloc + indexed scan (current)
        group.bench_function(format!("{name}/vec_char"), |b| {
            b.iter(|| {
                let chars: Vec<char> = std::hint::black_box(line).chars().collect();
                let len = chars.len();
                let mut col = len / 4;
                // Simulate word_forward: scan past word chars, then whitespace
                while col < len && chars[col].is_alphanumeric() {
                    col += 1;
                }
                while col < len && chars[col].is_whitespace() {
                    col += 1;
                }
                std::hint::black_box(col);
            });
        });

        // Approach 2: chars().nth() repeated (no alloc, O(n) per access)
        group.bench_function(format!("{name}/chars_nth"), |b| {
            b.iter(|| {
                let s = std::hint::black_box(line);
                let len = s.chars().count();
                let mut col = len / 4;
                while col < len && s.chars().nth(col).is_some_and(|c| c.is_alphanumeric()) {
                    col += 1;
                }
                while col < len && s.chars().nth(col).is_some_and(|c| c.is_whitespace()) {
                    col += 1;
                }
                std::hint::black_box(col);
            });
        });

        // Approach 3: byte offset table (Vec<u32>) + char lookup
        group.bench_function(format!("{name}/byte_offsets"), |b| {
            b.iter(|| {
                let s = std::hint::black_box(line);
                let offsets: Vec<u32> = s.char_indices().map(|(b, _)| b as u32).collect();
                let len = offsets.len();
                let mut col = len / 4;
                let char_at =
                    |idx: usize| -> char { s[offsets[idx] as usize..].chars().next().unwrap() };
                while col < len && char_at(col).is_alphanumeric() {
                    col += 1;
                }
                while col < len && char_at(col).is_whitespace() {
                    col += 1;
                }
                std::hint::black_box(col);
            });
        });

        // Approach 4: skip-to-offset + sequential scan (hybrid)
        group.bench_function(format!("{name}/skip_then_scan"), |b| {
            b.iter(|| {
                let s = std::hint::black_box(line);
                let len = s.chars().count();
                let start = len / 4;
                let mut col = start;
                // Skip to start position, then scan sequentially
                let byte_start = s.char_indices().nth(start).map_or(s.len(), |(b, _)| b);
                let mut iter = s[byte_start..].chars();
                while col < len {
                    match iter.next() {
                        Some(c) if c.is_alphanumeric() => col += 1,
                        _ => break,
                    }
                }
                let byte_ws = s.char_indices().nth(col).map_or(s.len(), |(b, _)| b);
                let mut iter = s[byte_ws..].chars();
                while col < len {
                    match iter.next() {
                        Some(c) if c.is_whitespace() => col += 1,
                        _ => break,
                    }
                }
                std::hint::black_box(col);
            });
        });
    }
    group.finish();
}

// =========================================================================
// Line hash — used for diff detection
// =========================================================================

fn bench_line_hashes(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/line_hashes");
    for &lines in SIZES {
        let buffer = generate_buffer(lines);
        group.bench_with_input(BenchmarkId::new("lines", lines), &buffer, |b, buffer| {
            b.iter(|| {
                let _ = std::hint::black_box(buffer.line_hashes());
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_insert_char,
    bench_insert_word,
    bench_insert_line,
    bench_delete_char,
    bench_delete_range,
    bench_line_access,
    bench_content,
    bench_clone,
    bench_from_string,
    bench_position_to_byte,
    bench_byte_to_position,
    bench_vec_char_alloc,
    bench_vec_char_motion_sim,
    bench_line_hashes,
);
criterion_main!(benches);
