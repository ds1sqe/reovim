//! Syntax highlighting hot path benchmarks.
//!
//! Measures the operations that run on every keystroke when editing Rust files:
//! - Tree-sitter full parse vs incremental update
//! - Highlights query: full-file vs scoped byte range
//! - Token update construction (highlights + decorations + conversion)
//!
//! These benchmarks validate the performance bottlenecks identified in the
//! edit hot path (`emit_syntax_updates` in `input.rs`).

use std::sync::Arc;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

use {
    reovim_driver_syntax::{SyntaxDriver, SyntaxDriverFactory, SyntaxEdit},
    reovim_module_treesitter_rust::RustSyntaxFactory,
};

/// Generate realistic Rust source code with N lines.
///
/// Produces a mix of functions, structs, impls, and comments to exercise
/// tree-sitter's Rust grammar realistically.
fn generate_rust_source(lines: usize) -> String {
    let mut result = String::with_capacity(lines * 50);
    result.push_str("use std::collections::HashMap;\n");
    result.push_str("use std::sync::Arc;\n\n");

    let funcs_needed = lines / 12; // ~12 lines per function
    for i in 0..funcs_needed.max(1) {
        result.push_str(&format!("/// Documentation for function `func_{i}`.\n"));
        result.push_str(&format!(
            "pub fn func_{i}(input: &str, count: usize) -> Option<String> {{\n"
        ));
        result.push_str("    let mut result = String::new();\n");
        result.push_str("    for i in 0..count {\n");
        result.push_str("        if i > 0 {\n");
        result.push_str("            result.push_str(\", \");\n");
        result.push_str("        }\n");
        result.push_str(&format!(
            "        result.push_str(&format!(\"{{}}:{{}}\", input, i + {i}));\n"
        ));
        result.push_str("    }\n");
        result.push_str("    if result.is_empty() { None } else { Some(result) }\n");
        result.push_str("}\n\n");
    }

    // Pad to exact line count
    let current_lines = result.lines().count();
    if current_lines < lines {
        for i in 0..(lines - current_lines) {
            result.push_str(&format!("// padding line {i}\n"));
        }
    }

    result
}

/// Create a Rust syntax driver with the content already parsed.
fn create_parsed_driver(content: &str) -> Box<dyn SyntaxDriver> {
    let factory = RustSyntaxFactory::new();
    let mut driver = factory.create("rust").expect("Rust driver creation");
    driver.parse(content);
    driver
}

// ============================================================================
// Benchmark: Full parse vs incremental update
// ============================================================================

/// Benchmark: Tree-sitter full parse (cold, no old tree).
///
/// This is what happens on every keystroke when `InsertChar` bypasses
/// incremental parsing (the current bug).
fn bench_treesitter_full_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax/full_parse");
    let factory = RustSyntaxFactory::new();

    for &lines in &[100, 500, 1000, 5000] {
        let content = generate_rust_source(lines);

        group.bench_with_input(BenchmarkId::new("lines", lines), &content, |b, content| {
            b.iter_batched(
                || factory.create("rust").unwrap(),
                |mut driver| {
                    driver.parse(content);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: Tree-sitter incremental update (edit + reparse with old tree).
///
/// This is what SHOULD happen on every keystroke: O(edit) instead of O(file).
fn bench_treesitter_incremental_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax/incremental_update");

    for &lines in &[100, 500, 1000, 5000] {
        let content = generate_rust_source(lines);
        let mid_line = lines / 2;

        // Compute byte offset for the insertion point (middle of file)
        let insert_byte: usize = content
            .lines()
            .take(mid_line)
            .map(|l| l.len() + 1) // +1 for newline
            .sum();
        let insert_col = 4u32; // indent level
        let insert_byte_with_col = insert_byte + insert_col as usize;

        // Content after inserting a single character "X"
        let mut new_content = content.clone();
        new_content.insert(insert_byte_with_col, 'X');

        let edit = SyntaxEdit::insert(
            insert_byte_with_col,
            mid_line as u32,
            insert_col,
            insert_byte_with_col + 1,
            mid_line as u32,
            insert_col + 1,
        );

        group.bench_with_input(
            BenchmarkId::new("lines", lines),
            &(content, new_content, edit),
            |b, (original, modified, edit)| {
                b.iter_batched(
                    || create_parsed_driver(original),
                    |mut driver| {
                        driver.update(modified, edit);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Highlights query scope
// ============================================================================

/// Benchmark: Highlights query over entire file (0..usize::MAX).
///
/// This is what `build_token_update` currently does after every edit.
fn bench_highlights_full_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax/highlights_full_file");

    for &lines in &[100, 500, 1000, 5000] {
        let content = generate_rust_source(lines);
        let driver = create_parsed_driver(&content);

        group.bench_with_input(BenchmarkId::new("lines", lines), &driver, |b, driver| {
            b.iter(|| {
                let _ = std::hint::black_box(driver.highlights(0..usize::MAX));
            });
        });
    }

    group.finish();
}

/// Benchmark: Highlights query over a viewport-sized byte range.
///
/// This measures what the cost WOULD be if we scoped the query to the
/// visible viewport (~50 lines) instead of the entire file.
fn bench_highlights_viewport(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax/highlights_viewport");

    for &lines in &[100, 500, 1000, 5000] {
        let content = generate_rust_source(lines);
        let driver = create_parsed_driver(&content);

        // Compute byte range for ~50 lines around the middle
        let mid_line = lines / 2;
        let viewport_start_line = mid_line.saturating_sub(25);
        let viewport_end_line = (mid_line + 25).min(lines);

        let start_byte: usize = content
            .lines()
            .take(viewport_start_line)
            .map(|l| l.len() + 1)
            .sum();
        let end_byte: usize = content
            .lines()
            .take(viewport_end_line)
            .map(|l| l.len() + 1)
            .sum();

        group.bench_with_input(
            BenchmarkId::new("lines", lines),
            &(driver, start_byte, end_byte),
            |b, (driver, start, end)| {
                b.iter(|| {
                    let _ = std::hint::black_box(driver.highlights(*start..*end));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Highlights query for edit region only (~200 bytes around edit).
///
/// This is what `SyntaxStreamState::notify_edit` uses (scoped range with padding).
fn bench_highlights_edit_region(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax/highlights_edit_region");

    for &lines in &[100, 500, 1000, 5000] {
        let content = generate_rust_source(lines);
        let driver = create_parsed_driver(&content);

        // 200-byte range around middle of file
        let mid_byte = content.len() / 2;
        let start_byte = mid_byte.saturating_sub(100);
        let end_byte = (mid_byte + 100).min(content.len());

        group.bench_with_input(
            BenchmarkId::new("lines", lines),
            &(driver, start_byte, end_byte),
            |b, (driver, start, end)| {
                b.iter(|| {
                    let _ = std::hint::black_box(driver.highlights(*start..*end));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: End-to-end edit cycle
// ============================================================================

/// Benchmark: Full edit cycle — content join + full parse + full highlights.
///
/// This represents the CURRENT per-keystroke cost when InsertChar is used
/// (the worst-case path that hits all bottlenecks).
fn bench_edit_cycle_current(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax/edit_cycle_current");
    let factory = Arc::new(RustSyntaxFactory::new());

    for &lines in &[100, 500, 1000, 5000] {
        let content = generate_rust_source(lines);
        // Simulate: buffer stores lines separately, content() joins them
        let buffer_lines: Vec<String> = content.lines().map(String::from).collect();

        group.bench_with_input(
            BenchmarkId::new("lines", lines),
            &(buffer_lines, factory.clone()),
            |b, (lines_vec, factory)| {
                b.iter_batched(
                    || {
                        let content = lines_vec.join("\n");
                        let mut driver = factory.create("rust").unwrap();
                        driver.parse(&content);
                        (driver, lines_vec.clone())
                    },
                    |(mut driver, buf_lines)| {
                        // Step 1: Join lines (content() call)
                        let content = buf_lines.join("\n");
                        // Step 2: Full parse (no edit info)
                        driver.parse(&content);
                        // Step 3: Full highlights query
                        let _ = std::hint::black_box(driver.highlights(0..usize::MAX));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark: Optimized edit cycle — incremental update + scoped highlights.
///
/// This represents the IDEAL per-keystroke cost after fixes:
/// - Incremental tree-sitter update (O(edit) not O(file))
/// - Scoped highlights query (viewport, not full file)
fn bench_edit_cycle_optimized(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax/edit_cycle_optimized");

    for &lines in &[100, 500, 1000, 5000] {
        let content = generate_rust_source(lines);
        let mid_line = lines / 2;

        let insert_byte: usize = content.lines().take(mid_line).map(|l| l.len() + 1).sum();
        let insert_byte_with_col = insert_byte + 4;

        let mut new_content = content.clone();
        new_content.insert(insert_byte_with_col, 'X');

        let edit = SyntaxEdit::insert(
            insert_byte_with_col,
            mid_line as u32,
            4,
            insert_byte_with_col + 1,
            mid_line as u32,
            5,
        );

        // Scoped highlights: 200 bytes around edit
        let start_byte = insert_byte_with_col.saturating_sub(100);
        let end_byte = (insert_byte_with_col + 100).min(new_content.len());

        group.bench_with_input(
            BenchmarkId::new("lines", lines),
            &(content, new_content, edit, start_byte, end_byte),
            |b, (original, modified, edit, hl_start, hl_end)| {
                b.iter_batched(
                    || create_parsed_driver(original),
                    |mut driver| {
                        // Step 1: Incremental update (no content join needed if we pass ref)
                        driver.update(modified, edit);
                        // Step 2: Scoped highlights (edit region only)
                        let _ = std::hint::black_box(driver.highlights(*hl_start..*hl_end));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_treesitter_full_parse,
    bench_treesitter_incremental_update,
    bench_highlights_full_file,
    bench_highlights_viewport,
    bench_highlights_edit_region,
    bench_edit_cycle_current,
    bench_edit_cycle_optimized,
);
criterion_main!(benches);
