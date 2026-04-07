//! Search operation benchmarks — v0.14.3 baseline.
//!
//! Measures the cost of search-related operations to establish baselines
//! before the search engine rewrite (regex cache, line iteration, etc.).
//!
//! Run: `cargo bench -p reovim-bench --bench search_ops`

// Intentional: benchmarking regex compilation cost (the bug IS recompiling in loops).
#![allow(clippy::regex_creation_in_loops)]

use {
    criterion::{BenchmarkId, Criterion, criterion_group, criterion_main},
    reovim_provider_text::Buffer,
};

const SIZES: &[usize] = &[1_000, 10_000, 100_000];

fn generate_buffer(line_count: usize) -> Buffer {
    let lines: Vec<String> = (0..line_count)
        .map(|i| {
            if i % 100 == 0 {
                format!("fn handle_request(ctx: &Context) {{ // line {i}")
            } else if i % 50 == 0 {
                format!("let result = process_data(input); // line {i}")
            } else {
                format!("    let value = compute(x, y, z); // line {i}")
            }
        })
        .collect();
    Buffer::from_string(&lines.join("\n"))
}

// =========================================================================
// content() materialization — the dominant search cost
// =========================================================================

fn bench_content_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/content_materialization");
    for &lines in SIZES {
        let buf = generate_buffer(lines);
        group.bench_with_input(BenchmarkId::new("lines", lines), &lines, |b, _| {
            b.iter(|| {
                // Search calls content() 2-4 times per find_next/find_backward
                let _c1 = buf.content();
                let _c2 = buf.content();
            });
        });
    }
    group.finish();
}

// =========================================================================
// Regex compilation — recompiled on every search call
// =========================================================================

fn bench_regex_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/regex_compile");

    let patterns = [
        ("simple_word", "handle_request"),
        ("char_class", r"[a-z]+_[a-z]+"),
        ("complex", r"fn\s+\w+\s*\([^)]*\)"),
    ];

    for (name, pattern) in &patterns {
        group.bench_function(*name, |b| {
            b.iter(|| {
                let _re = regex::Regex::new(pattern).unwrap();
            });
        });
    }

    // Cached comparison
    let cached = regex::Regex::new(r"fn\s+\w+\s*\([^)]*\)").unwrap();
    group.bench_function("cached_complex_match", |b| {
        let text = "fn handle_request(ctx: &Context) {";
        b.iter(|| {
            let _m = cached.find(text);
        });
    });

    group.finish();
}

// =========================================================================
// Backward search: collect-all-then-filter vs early termination
// =========================================================================

fn bench_backward_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/backward");

    for &lines in SIZES {
        let buf = generate_buffer(lines);
        let content = buf.content();
        let re = regex::Regex::new("compute").unwrap();
        let cursor_byte = content.len() / 2;

        // Current: collect ALL matches, take last before cursor
        group.bench_with_input(BenchmarkId::new("collect_all", lines), &lines, |b, _| {
            b.iter(|| {
                let matches: Vec<_> = re.find_iter(&content).collect();
                let found = matches.iter().rev().find(|m| m.start() < cursor_byte);
                found.map(|m| m.start())
            });
        });

        // Optimal: iterate forward, stop at cursor
        group.bench_with_input(BenchmarkId::new("early_stop", lines), &lines, |b, _| {
            b.iter(|| {
                let mut last = None;
                for m in re.find_iter(&content) {
                    if m.start() >= cursor_byte {
                        break;
                    }
                    last = Some(m);
                }
                last
            });
        });
    }
    group.finish();
}

// =========================================================================
// Full search cost: current approach vs line-iteration approach
// =========================================================================

fn bench_full_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/full");

    for &lines in SIZES {
        let buf = generate_buffer(lines);

        // Current: content() + Regex::new() + match + content() again
        group.bench_with_input(BenchmarkId::new("current", lines), &lines, |b, _| {
            b.iter(|| {
                let content = buf.content();
                let re = regex::Regex::new("handle_request").unwrap();
                let _m = re.find(&content);
                let _content2 = buf.content();
            });
        });

        // Optimal: cached regex + line iteration
        group.bench_with_input(BenchmarkId::new("optimal", lines), &lines, |b, _| {
            let re = regex::Regex::new("handle_request").unwrap();
            b.iter(|| {
                for i in 0..buf.line_count() {
                    if let Some(line) = buf.line(i)
                        && re.is_match(line)
                    {
                        return Some((i, re.find(line)));
                    }
                }
                None::<(usize, Option<regex::Match<'_>>)>
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_content_materialization,
    bench_regex_compile,
    bench_backward_search,
    bench_full_search,
);
criterion_main!(benches);
