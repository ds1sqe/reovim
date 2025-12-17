//! Screen and I/O benchmarks

use criterion::{black_box, BenchmarkId, Criterion};
use reovim_core::{
    command_line::CommandLine,
    completion::CompletionState,
    highlight::{ColorMode, HighlightStore, Theme},
    leap::LeapState,
    modd::ModeState,
    screen::{Screen, WhichKeyPanel},
    telescope::TelescopeState,
};
use std::io::BufWriter;
use tempfile::NamedTempFile;

use super::common::{create_buffer, MockWriter};

/// Benchmark full Screen::render() with I/O (mock writer)
pub fn bench_screen_render_full_io(c: &mut Criterion) {
    let mut group = c.benchmark_group("screen_io");

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let mode = ModeState::normal();
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let completion = CompletionState::new();
    let telescope = TelescopeState::new();
    let leap = LeapState::new();

    for lines in [100, 1000, 10000] {
        let buffer = create_buffer(lines);
        let buffers = vec![buffer];

        group.bench_with_input(
            BenchmarkId::new("full_render", lines),
            &buffers,
            |b, bufs| {
                b.iter_with_setup(
                    || {
                        let writer = MockWriter::new();
                        Screen::with_writer(writer, 120, 50)
                    },
                    |mut screen| {
                        screen
                            .render(
                                black_box(bufs),
                                black_box(&highlight_store),
                                black_box(&mode),
                                black_box(&cmd_line),
                                black_box(pending_keys),
                                black_box(last_command),
                                black_box(color_mode),
                                black_box(&theme),
                                black_box(None),
                                black_box(&which_key),
                                black_box(&completion),
                                black_box(&telescope),
                                black_box(&leap),
                            )
                            .unwrap();
                        screen.flush().unwrap();
                        black_box(screen)
                    },
                )
            },
        );
    }

    group.finish();
}

/// Benchmark I/O bytes written per render
pub fn bench_io_bytes_written(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_bytes");

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let mode = ModeState::normal();
    let leap = LeapState::new();
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let completion = CompletionState::new();
    let telescope = TelescopeState::new();

    let buffer = create_buffer(1000);
    let buffers = vec![buffer];

    let estimated_bytes = 20_000u64;
    group.throughput(criterion::Throughput::Bytes(estimated_bytes));

    group.bench_function("full_screen_render", |b| {
        b.iter_with_setup(
            || {
                let writer = MockWriter::new();
                Screen::with_writer(writer, 120, 50)
            },
            |mut screen| {
                screen
                    .render(
                        black_box(&buffers),
                        black_box(&highlight_store),
                        black_box(&mode),
                        black_box(&cmd_line),
                        black_box(pending_keys),
                        black_box(last_command),
                        black_box(color_mode),
                        black_box(&theme),
                        black_box(None),
                        black_box(&which_key),
                        black_box(&completion),
                        black_box(&telescope),
                        black_box(&leap),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box(screen)
            },
        )
    });

    group.finish();
}

/// Benchmark comparing render with different viewport sizes (full I/O)
pub fn bench_screen_viewport_io(c: &mut Criterion) {
    let mut group = c.benchmark_group("screen_viewport_io");

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let mode = ModeState::normal();
    let leap = LeapState::new();
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let completion = CompletionState::new();
    let telescope = TelescopeState::new();

    let buffer = create_buffer(10000);
    let buffers = vec![buffer];

    for height in [24, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("viewport_height", height),
            &buffers,
            |b, bufs| {
                b.iter_with_setup(
                    || {
                        let writer = MockWriter::new();
                        Screen::with_writer(writer, 120, height)
                    },
                    |mut screen| {
                        screen
                            .render(
                                black_box(bufs),
                                black_box(&highlight_store),
                                black_box(&mode),
                                black_box(&cmd_line),
                                black_box(pending_keys),
                                black_box(last_command),
                                black_box(color_mode),
                                black_box(&theme),
                                black_box(None),
                                black_box(&which_key),
                                black_box(&completion),
                                black_box(&telescope),
                                black_box(&leap),
                            )
                            .unwrap();
                        screen.flush().unwrap();
                        black_box(screen)
                    },
                )
            },
        );
    }

    group.finish();
}

/// Benchmark with real file I/O
pub fn bench_file_io(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_io");

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let mode = ModeState::normal();
    let leap = LeapState::new();
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let completion = CompletionState::new();
    let telescope = TelescopeState::new();

    let buffer = create_buffer(1000);
    let buffers = vec![buffer];

    group.bench_function("unbuffered_file", |b| {
        b.iter_with_setup(
            || {
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                (Screen::with_writer(file, 120, 50), tmp)
            },
            |(mut screen, _tmp)| {
                screen
                    .render(
                        black_box(&buffers),
                        black_box(&highlight_store),
                        black_box(&mode),
                        black_box(&cmd_line),
                        black_box(pending_keys),
                        black_box(last_command),
                        black_box(color_mode),
                        black_box(&theme),
                        black_box(None),
                        black_box(&which_key),
                        black_box(&completion),
                        black_box(&telescope),
                        black_box(&leap),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box(screen)
            },
        )
    });

    group.bench_function("buffered_file", |b| {
        b.iter_with_setup(
            || {
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                (Screen::with_writer(writer, 120, 50), tmp)
            },
            |(mut screen, _tmp)| {
                screen
                    .render(
                        black_box(&buffers),
                        black_box(&highlight_store),
                        black_box(&mode),
                        black_box(&cmd_line),
                        black_box(pending_keys),
                        black_box(last_command),
                        black_box(color_mode),
                        black_box(&theme),
                        black_box(None),
                        black_box(&which_key),
                        black_box(&completion),
                        black_box(&telescope),
                        black_box(&leap),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box(screen)
            },
        )
    });

    group.finish();
}

/// Benchmark comparing viewport sizes with real file I/O
pub fn bench_file_io_viewport(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_io_viewport");

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let mode = ModeState::normal();
    let leap = LeapState::new();
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let completion = CompletionState::new();
    let telescope = TelescopeState::new();

    let buffer = create_buffer(10000);
    let buffers = vec![buffer];

    for height in [24, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("viewport", height),
            &buffers,
            |b, bufs| {
                b.iter_with_setup(
                    || {
                        let tmp = NamedTempFile::new().unwrap();
                        let file = tmp.reopen().unwrap();
                        let writer = BufWriter::with_capacity(64 * 1024, file);
                        (Screen::with_writer(writer, 120, height), tmp)
                    },
                    |(mut screen, _tmp)| {
                        screen
                            .render(
                                black_box(bufs),
                                black_box(&highlight_store),
                                black_box(&mode),
                                black_box(&cmd_line),
                                black_box(pending_keys),
                                black_box(last_command),
                                black_box(color_mode),
                                black_box(&theme),
                                black_box(None),
                                black_box(&which_key),
                                black_box(&completion),
                                black_box(&telescope),
                                black_box(&leap),
                            )
                            .unwrap();
                        screen.flush().unwrap();
                        black_box(screen)
                    },
                )
            },
        );
    }

    group.finish();
}
