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
    bench_line_hashes,
);
criterion_main!(benches);
