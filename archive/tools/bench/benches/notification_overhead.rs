//! Notification and bridge overhead benchmarks.
//!
//! Measures the per-keystroke overhead of the extension bridge system:
//! - JSON serialization of bridge snapshots
//! - Bridge is_active() iteration cost (simulated)
//! - Token span conversion (Annotation -> TokenSpan equivalent)
//!
//! These benchmarks validate the notification bottlenecks identified in
//! `detect_bridge_changes` and `build_extension_notification`.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

// ============================================================================
// Benchmark: JSON serialization overhead (bridge snapshots)
// ============================================================================

/// Simulate a bufferline snapshot with N buffer entries.
fn make_bufferline_json(buffer_count: usize) -> serde_json::Value {
    let buffers: Vec<serde_json::Value> = (0..buffer_count)
        .map(|i| {
            serde_json::json!({
                "id": i,
                "name": format!("file_{i}.rs"),
                "path": format!("/home/user/project/src/file_{i}.rs"),
                "modified": i % 3 == 0,
                "pinned": i < 2,
                "filetype": "rust",
                "errorCount": i % 5,
                "warningCount": i % 3,
            })
        })
        .collect();

    serde_json::json!({
        "active": true,
        "buffers": buffers,
    })
}

/// Simulate a diagnostic panel snapshot.
fn make_diagnostic_json(diag_count: usize) -> serde_json::Value {
    let items: Vec<serde_json::Value> = (0..diag_count)
        .map(|i| {
            serde_json::json!({
                "severity": if i % 4 == 0 { "error" } else { "warning" },
                "message": format!("unused variable `var_{i}` in function body"),
                "file": format!("src/module_{}.rs", i / 10),
                "line": i * 3 + 1,
                "col": (i * 7) % 80,
            })
        })
        .collect();

    serde_json::json!({
        "active": true,
        "mode": "diagnostics",
        "items": items,
        "errorCount": diag_count / 4,
        "warningCount": diag_count * 3 / 4,
    })
}

/// Simulate an illuminate highlight snapshot.
fn make_illuminate_json(range_count: usize) -> serde_json::Value {
    let ranges: Vec<serde_json::Value> = (0..range_count)
        .map(|i| {
            serde_json::json!({
                "startLine": i * 10,
                "startCol": 4,
                "endLine": i * 10,
                "endCol": 12,
                "kind": if i % 3 == 0 { "write" } else { "read" },
            })
        })
        .collect();

    serde_json::json!({
        "active": true,
        "bufferId": 1,
        "word": "some_variable",
        "ranges": ranges,
        "sequence": 42,
    })
}

/// Benchmark: JSON serialization of bridge snapshots to string.
///
/// This is what happens in `build_extension_notification` for each active
/// bridge on every keystroke (due to the `|| is_active` bug).
fn bench_bridge_json_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("bridge/json_serialize");

    // Bufferline with varying buffer counts
    for &count in &[3, 10, 30] {
        let json = make_bufferline_json(count);
        group.bench_with_input(BenchmarkId::new("bufferline_buffers", count), &json, |b, json| {
            b.iter(|| {
                let _ = std::hint::black_box(json.to_string());
            });
        });
    }

    // Diagnostic panel with varying diagnostic counts
    for &count in &[10, 50, 200] {
        let json = make_diagnostic_json(count);
        group.bench_with_input(BenchmarkId::new("diagnostics_items", count), &json, |b, json| {
            b.iter(|| {
                let _ = std::hint::black_box(json.to_string());
            });
        });
    }

    // Illuminate with varying range counts
    for &count in &[5, 20, 50] {
        let json = make_illuminate_json(count);
        group.bench_with_input(BenchmarkId::new("illuminate_ranges", count), &json, |b, json| {
            b.iter(|| {
                let _ = std::hint::black_box(json.to_string());
            });
        });
    }

    group.finish();
}

/// Benchmark: JSON parse of bridge data on client side.
///
/// TUI receives the JSON string and must parse it in `on_notification`.
fn bench_bridge_json_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("bridge/json_parse");

    for &count in &[3, 10, 30] {
        let json_str = make_bufferline_json(count).to_string();
        group.bench_with_input(
            BenchmarkId::new("bufferline_buffers", count),
            &json_str,
            |b, json_str| {
                b.iter(|| {
                    let _: serde_json::Value =
                        std::hint::black_box(serde_json::from_str(json_str).unwrap());
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Per-keystroke bridge detection overhead (simulated)
// ============================================================================

/// Simulate the `detect_bridge_changes` loop with N bridges.
///
/// The real implementation iterates all bridges twice per keystroke
/// (once to snapshot before, once to check after). This measures the
/// overhead of the iteration + comparison logic itself.
fn bench_bridge_detection_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("bridge/detection_loop");

    // Simulate bridge active states (bool check per bridge)
    for &bridge_count in &[10, 16, 22, 30] {
        // Before state: some bridges active
        let before: Vec<(&str, bool)> = (0..bridge_count)
            .map(|i| {
                let name: &str = Box::leak(format!("bridge_{i}").into_boxed_str());
                // ~30% of bridges are "active" (typical editing session)
                (name, i % 3 == 0)
            })
            .collect();

        // After state: same as before (nothing actually changed)
        let after_active: Vec<bool> = before.iter().map(|(_, active)| *active).collect();

        group.bench_with_input(
            BenchmarkId::new("bridges", bridge_count),
            &(before, after_active),
            |b, (before, after)| {
                b.iter(|| {
                    let mut changed_count = 0u32;
                    for (i, &(_, was_active)) in before.iter().enumerate() {
                        let is_active = after[i];
                        // Current (buggy) logic: || is_active
                        if was_active != is_active || is_active {
                            changed_count += 1;
                        }
                    }
                    std::hint::black_box(changed_count)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Fixed bridge detection (only on toggle).
///
/// Measures the overhead with the correct logic: only record change
/// when bridge active state actually toggles.
fn bench_bridge_detection_fixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("bridge/detection_fixed");

    for &bridge_count in &[10, 16, 22, 30] {
        let before: Vec<(&str, bool)> = (0..bridge_count)
            .map(|i| {
                let name: &str = Box::leak(format!("bridge_f_{i}").into_boxed_str());
                (name, i % 3 == 0)
            })
            .collect();

        let after_active: Vec<bool> = before.iter().map(|(_, active)| *active).collect();

        group.bench_with_input(
            BenchmarkId::new("bridges", bridge_count),
            &(before, after_active),
            |b, (before, after)| {
                b.iter(|| {
                    let mut changed_count = 0u32;
                    for (i, &(_, was_active)) in before.iter().enumerate() {
                        let is_active = after[i];
                        // Fixed logic: only on toggle
                        if was_active != is_active {
                            changed_count += 1;
                        }
                    }
                    std::hint::black_box(changed_count)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Per-bridge serialize+emit cost (the real penalty)
// ============================================================================

/// Benchmark: Total cost of unnecessary bridge notifications per keystroke.
///
/// Simulates what happens when `detect_bridge_changes` triggers for N active
/// bridges: snapshot JSON creation + string serialization for each.
fn bench_unnecessary_bridge_notifications(c: &mut Criterion) {
    let mut group = c.benchmark_group("bridge/unnecessary_per_keystroke");

    // Typical editing session: bufferline + indent-guide + pair active
    // Each fires a full JSON snapshot on every keystroke
    let bufferline_snap = make_bufferline_json(5);
    let illuminate_snap = make_illuminate_json(0); // empty but still serialized
    let pair_snap = serde_json::json!({"active": true, "pairs": []});

    // 1 active bridge
    group.bench_function("active_1", |b| {
        b.iter(|| {
            let _ = std::hint::black_box(bufferline_snap.to_string());
        });
    });

    // 3 active bridges (typical)
    group.bench_function("active_3", |b| {
        b.iter(|| {
            let _ = std::hint::black_box(bufferline_snap.to_string());
            let _ = std::hint::black_box(illuminate_snap.to_string());
            let _ = std::hint::black_box(pair_snap.to_string());
        });
    });

    // 5 active bridges (worst case with completion + cmdline)
    let completion_snap = serde_json::json!({"active": true, "items": [], "selected": 0});
    let cmdline_snap = serde_json::json!({"active": true, "prompt": ":", "input": "", "cursor": 0});

    group.bench_function("active_5", |b| {
        b.iter(|| {
            let _ = std::hint::black_box(bufferline_snap.to_string());
            let _ = std::hint::black_box(illuminate_snap.to_string());
            let _ = std::hint::black_box(pair_snap.to_string());
            let _ = std::hint::black_box(completion_snap.to_string());
            let _ = std::hint::black_box(cmdline_snap.to_string());
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Token span conversion overhead
// ============================================================================

/// Benchmark: Converting Annotation highlights to gRPC TokenSpan format.
///
/// This simulates the `.map()` in `build_token_update` that converts each
/// highlight annotation to a `TokenSpan` with string category allocation.
fn bench_token_span_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax/token_span_conversion");

    // Simulated highlight categories (typical for Rust)
    let categories = [
        "keyword",
        "function",
        "type",
        "variable",
        "string",
        "comment",
        "operator",
        "punctuation",
        "number",
        "attribute",
    ];

    for &count in &[100, 500, 2000, 10000] {
        // Simulate annotation data (byte offsets + category)
        let annotations: Vec<(u32, u32, &str)> = (0..count)
            .map(|i| {
                let start = (i * 8) as u32;
                let end = start + 6;
                let cat = categories[i % categories.len()];
                (start, end, cat)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("tokens", count),
            &annotations,
            |b, annotations| {
                b.iter(|| {
                    let tokens: Vec<(u32, u32, String)> = annotations
                        .iter()
                        .map(|&(start, end, cat)| (start, end, cat.to_string()))
                        .collect();
                    std::hint::black_box(tokens)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_bridge_json_serialize,
    bench_bridge_json_parse,
    bench_bridge_detection_loop,
    bench_bridge_detection_fixed,
    bench_unnecessary_bridge_notifications,
    bench_token_span_conversion,
);
criterion_main!(benches);
