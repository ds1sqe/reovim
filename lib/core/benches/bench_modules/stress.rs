//! Stress test benchmarks - Combined real-world scenarios

use criterion::{black_box, BenchmarkId, Criterion};
use reovim_core::buffer::TextOps;
use reovim_core::{
    command_line::CommandLine,
    completion::{CompletionItem, CompletionState},
    explorer::ExplorerState,
    highlight::{ColorMode, HighlightStore, Theme},
    leap::LeapState,
    modd::ModeState,
    screen::{Screen, WhichKeyPanel},
    telescope::TelescopeState,
};
use std::io::BufWriter;
use std::path::PathBuf;
use tempfile::NamedTempFile;

use super::common::{create_realistic_buffer, MockWriter};

/// Stress test: Rapid editing session (type, move, delete cycle)
pub fn bench_stress_editing_session(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_editing");
    group.sample_size(30);

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let completion = CompletionState::new();
    let telescope = TelescopeState::new();
    let leap = LeapState::new();
    let mode = ModeState::normal();

    group.bench_function("edit_navigate_cycle_1k", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_realistic_buffer(1000);
                buffer.cur.y = 500;
                buffer.cur.x = 10;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                for i in 0..20 {
                    match i % 5 {
                        0 => {
                            buffer.cur.y = buffer.cur.y.saturating_add(5).min(buffer.contents.len() as u16 - 1);
                        }
                        1 => {
                            buffer.insert_char('/');
                            buffer.insert_char('/');
                            buffer.insert_char(' ');
                        }
                        2 => {
                            buffer.cur.x = buffer.cur.x.saturating_add(10);
                        }
                        3 => {
                            buffer.delete_char_backward();
                        }
                        _ => {
                            buffer.cur.y = buffer.cur.y.saturating_sub(3);
                        }
                    }

                    let buffers = vec![buffer.clone()];
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
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("edit_navigate_cycle_10k", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_realistic_buffer(10000);
                buffer.cur.y = 5000;
                buffer.cur.x = 10;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                for i in 0..20 {
                    match i % 5 {
                        0 => buffer.cur.y = buffer.cur.y.saturating_add(5).min(buffer.contents.len() as u16 - 1),
                        1 => {
                            buffer.insert_char('/');
                            buffer.insert_char('/');
                        }
                        2 => buffer.cur.x = buffer.cur.x.saturating_add(10),
                        3 => buffer.delete_char_backward(),
                        _ => buffer.cur.y = buffer.cur.y.saturating_sub(3),
                    }

                    let buffers = vec![buffer.clone()];
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
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("edit_navigate_cycle_50k", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_realistic_buffer(50000);
                buffer.cur.y = 25000;
                buffer.cur.x = 10;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                for i in 0..20 {
                    match i % 5 {
                        0 => buffer.cur.y = buffer.cur.y.saturating_add(5).min(buffer.contents.len() as u16 - 1),
                        1 => buffer.insert_char('x'),
                        2 => buffer.cur.x = buffer.cur.x.saturating_add(10),
                        3 => buffer.delete_char_backward(),
                        _ => buffer.cur.y = buffer.cur.y.saturating_sub(3),
                    }

                    let buffers = vec![buffer.clone()];
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
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.finish();
}

/// Stress test: Rapid scrolling (holding j/k key)
pub fn bench_stress_rapid_scroll(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_scroll");
    group.sample_size(30);

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

    group.bench_function("hold_j_50_lines_10k_file", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_realistic_buffer(10000);
                buffer.cur.y = 1000;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                for _ in 0..50 {
                    if (buffer.cur.y as usize) < buffer.contents.len() - 1 {
                        buffer.cur.y += 1;
                    }
                    let buffers = vec![buffer.clone()];
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
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("spam_ctrl_d_10_times", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_realistic_buffer(10000);
                buffer.cur.y = 1000;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                for _ in 0..10 {
                    buffer.cur.y = buffer.cur.y.saturating_add(25).min(buffer.contents.len() as u16 - 1);
                    let buffers = vec![buffer.clone()];
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
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("jump_around_file", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_realistic_buffer(10000);
                buffer.cur.y = 5000;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                let positions = [0, 9999, 5000, 2500, 7500, 0, 9999, 5000];
                for &pos in &positions {
                    buffer.cur.y = pos.min(buffer.contents.len() as u16 - 1);
                    buffer.cur.x = 0;
                    let buffers = vec![buffer.clone()];
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
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.finish();
}

/// Stress test: Mode switching with operations
pub fn bench_stress_mode_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_mode_ops");
    group.sample_size(30);

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let completion = CompletionState::new();
    let telescope = TelescopeState::new();
    let leap = LeapState::new();
    let normal_mode = ModeState::normal();
    let insert_mode = ModeState::insert();

    group.bench_function("insert_escape_move_cycle", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_realistic_buffer(1000);
                buffer.cur.y = 500;
                buffer.cur.x = 0;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                for _ in 0..10 {
                    // Enter insert mode
                    let buffers = vec![buffer.clone()];
                    screen
                        .render(
                            black_box(&buffers),
                            black_box(&highlight_store),
                            black_box(&insert_mode),
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

                    // Type a few chars
                    buffer.insert_char('t');
                    buffer.insert_char('e');
                    buffer.insert_char('s');
                    buffer.insert_char('t');
                    let buffers = vec![buffer.clone()];
                    screen
                        .render(
                            black_box(&buffers),
                            black_box(&highlight_store),
                            black_box(&insert_mode),
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

                    // Escape to normal
                    let buffers = vec![buffer.clone()];
                    screen
                        .render(
                            black_box(&buffers),
                            black_box(&highlight_store),
                            black_box(&normal_mode),
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

                    // Move down
                    buffer.cur.y = buffer.cur.y.saturating_add(1);
                    buffer.cur.x = 0;
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.finish();
}

/// Stress test: Completion popup interactions
pub fn bench_stress_completion(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_completion");
    group.sample_size(30);

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let mode = ModeState::insert();
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let telescope = TelescopeState::new();
    let leap = LeapState::new();

    let large_items: Vec<CompletionItem> = (0..100)
        .map(|i| CompletionItem::new(format!("completion_item_with_long_name_{:03}", i), "buffer"))
        .collect();

    let mut large_completion = CompletionState::new();
    large_completion.activate(large_items, "comp".to_string(), 0, 50);

    let buffer = create_realistic_buffer(1000);
    let buffers = vec![buffer];

    group.bench_function("completion_scroll_100_items", |b| {
        b.iter_with_setup(
            || {
                let writer = MockWriter::new();
                let screen = Screen::with_writer(writer, 120, 50);
                let comp = large_completion.clone();
                (screen, comp)
            },
            |(mut screen, mut comp)| {
                for _ in 0..20 {
                    comp.select_next();
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
                            black_box(&comp),
                            black_box(&telescope),
                            black_box(&leap),
                        )
                        .unwrap();
                }
                screen.flush().unwrap();
                black_box((screen, comp))
            },
        )
    });

    group.finish();
}

/// Stress test: Combined worst-case scenario
pub fn bench_stress_worst_case(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_worst_case");
    group.sample_size(20);

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let cmd_line = CommandLine::default();
    let pending_keys = "dw";
    let last_command = "5j";
    let which_key = WhichKeyPanel::new();
    let telescope = TelescopeState::new();
    let leap = LeapState::new();
    let mode = ModeState::insert();

    let items: Vec<CompletionItem> = (0..50)
        .map(|i| CompletionItem::new(format!("item_{}", i), "buffer"))
        .collect();
    let mut completion = CompletionState::new();
    completion.activate(items, "it".to_string(), 0, 50);

    let explorer_state = ExplorerState::new(PathBuf::from("/tmp")).ok();

    group.bench_function("all_features_50k_file", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_realistic_buffer(50000);
                buffer.cur.y = 25000;
                buffer.cur.x = 20;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 160, 60);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                for i in 0..10 {
                    match i % 4 {
                        0 => buffer.cur.y = buffer.cur.y.saturating_add(10),
                        1 => {
                            buffer.insert_char('a');
                            buffer.insert_char('b');
                        }
                        2 => buffer.cur.y = buffer.cur.y.saturating_sub(5),
                        _ => buffer.delete_char_backward(),
                    }

                    let buffers = vec![buffer.clone()];
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
                            black_box(explorer_state.as_ref()),
                            black_box(&which_key),
                            black_box(&completion),
                            black_box(&telescope),
                            black_box(&leap),
                        )
                        .unwrap();
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.finish();
}

/// Micro-benchmark: Isolate buffer clone cost
pub fn bench_buffer_clone_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_clone");

    for lines in [100, 1000, 10000, 50000] {
        let buffer = create_realistic_buffer(lines);

        group.bench_with_input(
            BenchmarkId::new("clone", lines),
            &buffer,
            |b, buf| {
                b.iter(|| {
                    let cloned = black_box(buf).clone();
                    black_box(cloned)
                })
            },
        );
    }

    group.finish();
}

/// Micro-benchmark: Vec<Buffer> creation cost
pub fn bench_buffer_vec_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_vec");

    for lines in [100, 1000, 10000, 50000] {
        let buffer = create_realistic_buffer(lines);

        group.bench_with_input(
            BenchmarkId::new("vec_clone", lines),
            &buffer,
            |b, buf| {
                b.iter(|| {
                    let buffers = vec![black_box(buf).clone()];
                    black_box(buffers)
                })
            },
        );
    }

    group.finish();
}
