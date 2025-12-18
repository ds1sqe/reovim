//! RTT (Round-Trip Time) benchmarks - Real-world latency measurements

#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_possible_truncation)]

use std::{hint::black_box, io::BufWriter, path::PathBuf};

use {
    criterion::Criterion,
    reovim_core::{
        buffer::TextOps,
        command_line::CommandLine,
        completion::CompletionState,
        explorer::ExplorerState,
        folding::FoldManager,
        highlight::{ColorMode, HighlightStore, Theme},
        indent::IndentAnalyzer,
        leap::LeapState,
        modd::ModeState,
        screen::{Screen, WhichKeyPanel},
        settings_menu::SettingsMenuState,
        telescope::TelescopeState,
    },
    tempfile::NamedTempFile,
};

use super::common::{MockWriter, buffer_to_map, create_buffer};

/// Benchmark RTT for explorer toggle (open and close)
pub fn bench_rtt_explorer_toggle(c: &mut Criterion) {
    let mut group = c.benchmark_group("rtt_explorer");

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
    let fold_manager = FoldManager::new();
    let indent_analyzer = IndentAnalyzer::new(4);
    let settings_menu = SettingsMenuState::new();

    let buffer = create_buffer(1000);
    let buffers = buffer_to_map(buffer);

    let explorer_state = ExplorerState::new(PathBuf::from("/tmp")).ok();

    group.bench_function("open_explorer", |b| {
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
                        black_box(explorer_state.as_ref()),
                        black_box(&which_key),
                        black_box(&completion),
                        black_box(&telescope),
                        black_box(&leap),
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box(screen)
            },
        )
    });

    group.bench_function("close_explorer", |b| {
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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box(screen)
            },
        )
    });

    group.bench_function("toggle_cycle", |b| {
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
                        black_box(explorer_state.as_ref()),
                        black_box(&which_key),
                        black_box(&completion),
                        black_box(&telescope),
                        black_box(&leap),
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();

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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();

                screen.flush().unwrap();
                black_box(screen)
            },
        )
    });

    group.finish();
}

/// Benchmark RTT for input lag (keystroke to screen update)
pub fn bench_rtt_input_lag(c: &mut Criterion) {
    let mut group = c.benchmark_group("rtt_input_lag");

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
    let fold_manager = FoldManager::new();
    let indent_analyzer = IndentAnalyzer::new(4);
    let settings_menu = SettingsMenuState::new();
    let insert_mode = ModeState::insert();

    group.bench_function("char_insert_rtt", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(500);
                buffer.cur.y = 250;
                buffer.cur.x = 20;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                buffer.insert_char('x');
                let buffers = buffer_to_map(buffer.clone());
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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("backspace_rtt", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(500);
                buffer.cur.y = 250;
                buffer.cur.x = 30;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                buffer.delete_char_backward();
                let buffers = buffer_to_map(buffer.clone());
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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.finish();
}

/// Benchmark RTT for movement lag (cursor movement to screen update)
pub fn bench_rtt_movement_lag(c: &mut Criterion) {
    let mut group = c.benchmark_group("rtt_movement_lag");

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
    let fold_manager = FoldManager::new();
    let indent_analyzer = IndentAnalyzer::new(4);
    let settings_menu = SettingsMenuState::new();

    group.bench_function("move_down_rtt", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(10000);
                buffer.cur.y = 5000;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                if (buffer.cur.y as usize) < buffer.contents.len() - 1 {
                    buffer.cur.y += 1;
                }
                let buffers = buffer_to_map(buffer.clone());
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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("move_right_rtt", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(1000);
                buffer.cur.y = 500;
                buffer.cur.x = 20;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                buffer.cur.x += 1;
                let buffers = buffer_to_map(buffer.clone());
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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("word_forward_rtt", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(1000);
                buffer.cur.y = 500;
                buffer.cur.x = 0;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                buffer.cur.x = buffer.cur.x.saturating_add(5);
                let buffers = buffer_to_map(buffer.clone());
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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("half_page_down_rtt", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(10000);
                buffer.cur.y = 5000;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                buffer.cur.y = buffer
                    .cur
                    .y
                    .saturating_add(25)
                    .min(buffer.contents.len() as u16 - 1);
                let buffers = buffer_to_map(buffer.clone());
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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("goto_top_rtt", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(10000);
                buffer.cur.y = 5000;
                let tmp = NamedTempFile::new().unwrap();
                let file = tmp.reopen().unwrap();
                let writer = BufWriter::with_capacity(64 * 1024, file);
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen, tmp)
            },
            |(mut buffer, mut screen, _tmp)| {
                buffer.cur.y = 0;
                buffer.cur.x = 0;
                let buffers = buffer_to_map(buffer.clone());
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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.finish();
}
