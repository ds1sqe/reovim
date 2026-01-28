//! Window render benchmarks

use std::hint::black_box;

use {
    criterion::{BenchmarkId, Criterion},
    reovim_core::{
        frame::FrameBuffer,
        highlight::{HighlightStore, Theme},
        indent::IndentAnalyzer,
        modd::EditMode,
        visibility::NO_OP_BUFFER_VISIBILITY,
    },
};

use super::common::{create_buffer, create_window};

/// Benchmark `Window::render_to_buffer()` with various buffer sizes
pub fn bench_window_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("window_render");
    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let indent_analyzer = IndentAnalyzer::new(4);
    let window = create_window(50);
    let edit_mode = EditMode::Normal;

    for lines in [10, 100, 1000, 10000] {
        let buffer = create_buffer(lines);

        group.bench_with_input(BenchmarkId::new("buffer_lines", lines), &buffer, |b, buf| {
            b.iter(|| {
                let mut framebuffer = FrameBuffer::new(80, 50);
                window.render_to_buffer(
                    black_box(&mut framebuffer),
                    black_box(buf),
                    black_box(&highlight_store),
                    black_box(&theme),
                    &NO_OP_BUFFER_VISIBILITY,
                    black_box(&indent_analyzer),
                    None,
                    black_box(&edit_mode),
                );
                black_box(framebuffer)
            });
        });
    }

    group.finish();
}

/// Benchmark `Window::render_to_buffer()` with different viewport heights
pub fn bench_window_viewport_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("viewport_size");
    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let indent_analyzer = IndentAnalyzer::new(4);
    let edit_mode = EditMode::Normal;

    let buffer = create_buffer(10000);

    for height in [24, 50, 100, 200] {
        let window = create_window(height);

        group.bench_with_input(BenchmarkId::new("height", height), &buffer, |b, buf| {
            b.iter(|| {
                let mut framebuffer = FrameBuffer::new(80, height);
                window.render_to_buffer(
                    black_box(&mut framebuffer),
                    black_box(buf),
                    black_box(&highlight_store),
                    black_box(&theme),
                    &NO_OP_BUFFER_VISIBILITY,
                    black_box(&indent_analyzer),
                    None,
                    black_box(&edit_mode),
                );
                black_box(framebuffer)
            });
        });
    }

    group.finish();
}

/// Benchmark `Window::render_to_buffer()` with syntax highlighting
pub fn bench_window_with_highlights(c: &mut Criterion) {
    let mut group = c.benchmark_group("with_highlights");
    let theme = Theme::default();
    let indent_analyzer = IndentAnalyzer::new(4);
    let window = create_window(50);
    let buffer = create_buffer(1000);
    let edit_mode = EditMode::Normal;

    let empty_store = HighlightStore::new();
    group.bench_function("no_highlights", |b| {
        b.iter(|| {
            let mut framebuffer = FrameBuffer::new(80, 50);
            window.render_to_buffer(
                black_box(&mut framebuffer),
                black_box(&buffer),
                black_box(&empty_store),
                black_box(&theme),
                &NO_OP_BUFFER_VISIBILITY,
                black_box(&indent_analyzer),
                None,
                black_box(&edit_mode),
            );
            black_box(framebuffer)
        });
    });

    group.finish();
}

/// Benchmark to measure rendering throughput
pub fn bench_render_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.throughput(criterion::Throughput::Elements(1));

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let indent_analyzer = IndentAnalyzer::new(4);
    let window = create_window(50);
    let buffer = create_buffer(1000);
    let edit_mode = EditMode::Normal;

    group.bench_function("renders_per_second", |b| {
        b.iter(|| {
            let mut framebuffer = FrameBuffer::new(80, 50);
            window.render_to_buffer(
                black_box(&mut framebuffer),
                black_box(&buffer),
                black_box(&highlight_store),
                black_box(&theme),
                &NO_OP_BUFFER_VISIBILITY,
                black_box(&indent_analyzer),
                None,
                black_box(&edit_mode),
            );
            black_box(framebuffer)
        });
    });

    group.finish();
}
