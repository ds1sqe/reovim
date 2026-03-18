//! Buffer operation benchmarks.
//!
//! Measures performance of core buffer operations at various sizes.
//!
//! # Note (#471)
//!
//! Buffer no longer has a cursor - cursor is per-window/per-client.
//! These benchmarks use explicit positions instead of cursor-based methods.

use {
    criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main},
    reovim_kernel::api::v1::*,
};

/// Generate a buffer with N lines.
fn generate_buffer(lines: usize) -> Buffer {
    let content = (0..lines)
        .map(|i| format!("This is line number {} with some content", i))
        .collect::<Vec<_>>()
        .join("\n");
    Buffer::from_string(&content)
}

/// Benchmark: Single character insertion at various buffer sizes.
fn bench_insert_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/insert_char");

    for &lines in &[10, 100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, &lines| {
            b.iter_batched_ref(
                || generate_buffer(lines),
                |buf| {
                    // Insert at middle of buffer using explicit position
                    let pos = Position::new(lines / 2, 10);
                    buf.insert_at(pos, "X");
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: Multi-character insertion (simulating a word).
fn bench_insert_word(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/insert_word");

    for &lines in &[10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, &lines| {
            b.iter_batched_ref(
                || generate_buffer(lines),
                |buf| {
                    // Insert at middle of buffer using explicit position
                    let pos = Position::new(lines / 2, 10);
                    buf.insert_at(pos, "benchmark");
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: Delete single character.
fn bench_delete_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/delete_char");

    for &lines in &[10, 100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, &lines| {
            b.iter_batched_ref(
                || generate_buffer(lines),
                |buf| {
                    // Delete at middle of buffer using explicit position
                    let pos = Position::new(lines / 2, 10);
                    let _ = buf.delete_at(pos, 1);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: Delete range (simulating line deletion).
fn bench_delete_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/delete_range");

    for &lines in &[100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, &lines| {
            b.iter_batched_ref(
                || generate_buffer(lines),
                |buf| {
                    // Delete a line in the middle
                    let mid = lines / 2;
                    let _ = buf.delete_range(Position::new(mid, 0), Position::new(mid + 1, 0));
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: Buffer creation from string.
fn bench_from_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/from_string");

    for &lines in &[10, 100, 1000, 10000] {
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

/// Benchmark: Line access by index.
fn bench_line_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/line_access");

    for &lines in &[100, 1000, 10000] {
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

/// Benchmark: Position to byte offset conversion.
fn bench_position_to_byte(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/position_to_byte");

    for &lines in &[100, 1000, 10000] {
        let buffer = generate_buffer(lines);
        let mid = lines / 2;

        group.bench_with_input(BenchmarkId::new("lines", lines), &buffer, |b, buffer| {
            b.iter(|| {
                let _ = buffer.position_to_byte(Position::new(mid, 10));
            });
        });
    }

    group.finish();
}

/// Benchmark: Buffer content join (lines.join("\n")).
///
/// This measures the cost of `buffer.content()` which is called on every edit
/// in `emit_syntax_updates()`. For a 5000-line file, this creates a ~200KB
/// string allocation per keystroke.
fn bench_content_join(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/content_join");

    for &lines in &[100, 500, 1000, 5000, 10000] {
        let buffer = generate_buffer(lines);

        group.bench_with_input(BenchmarkId::new("lines", lines), &buffer, |b, buffer| {
            b.iter(|| {
                let _ = std::hint::black_box(buffer.content());
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_char,
    bench_insert_word,
    bench_delete_char,
    bench_delete_range,
    bench_from_string,
    bench_line_access,
    bench_position_to_byte,
    bench_content_join,
);
criterion_main!(benches);
