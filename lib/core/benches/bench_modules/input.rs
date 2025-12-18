//! Input simulation benchmarks

#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_possible_truncation)]

use std::hint::black_box;

use {
    criterion::Criterion,
    reovim_core::{
        buffer::TextOps,
        command_line::CommandLine,
        completion::{CompletionItem, CompletionState},
        folding::FoldManager,
        highlight::{ColorMode, HighlightStore, Theme},
        indent::IndentAnalyzer,
        leap::LeapState,
        modd::ModeState,
        screen::{Screen, WhichKeyPanel},
        settings_menu::SettingsMenuState,
        telescope::TelescopeState,
    },
};

use super::common::{MockWriter, buffer_to_map, create_buffer};

/// Benchmark simulating typing in insert mode
pub fn bench_typing_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_typing");

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let mode = ModeState::insert();
    let leap = LeapState::new();
    let fold_manager = FoldManager::new();
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let completion = CompletionState::new();
    let telescope = TelescopeState::new();
    let indent_analyzer = IndentAnalyzer::new(4);
    let settings_menu = SettingsMenuState::new();

    let chars_to_type = "Hello, world! This is a test of typing speed.";

    group.bench_function("single_char_render", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(100);
                buffer.cur.y = 50;
                let writer = MockWriter::new();
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen)
            },
            |(mut buffer, mut screen)| {
                buffer.insert_char('x');
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

    group.bench_function("burst_10_chars_render", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(100);
                buffer.cur.y = 50;
                let writer = MockWriter::new();
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen)
            },
            |(mut buffer, mut screen)| {
                for c in chars_to_type.chars().take(10) {
                    buffer.insert_char(c);
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

    group.finish();
}

/// Benchmark simulating scrolling (cursor movement + render)
pub fn bench_scrolling_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_scrolling");

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let mode = ModeState::normal();
    let leap = LeapState::new();
    let fold_manager = FoldManager::new();
    let cmd_line = CommandLine::default();
    let pending_keys = "";
    let last_command = "";
    let which_key = WhichKeyPanel::new();
    let completion = CompletionState::new();
    let telescope = TelescopeState::new();
    let indent_analyzer = IndentAnalyzer::new(4);
    let settings_menu = SettingsMenuState::new();

    group.bench_function("scroll_one_line", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(10000);
                buffer.cur.y = 100;
                let writer = MockWriter::new();
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen)
            },
            |(mut buffer, mut screen)| {
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

    group.bench_function("scroll_10_lines", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(10000);
                buffer.cur.y = 100;
                let writer = MockWriter::new();
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen)
            },
            |(mut buffer, mut screen)| {
                for _ in 0..10 {
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
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.bench_function("scroll_half_page", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(10000);
                buffer.cur.y = 100;
                let writer = MockWriter::new();
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen)
            },
            |(mut buffer, mut screen)| {
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

    group.finish();
}

/// Benchmark mode switching overhead
pub fn bench_mode_switching(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_mode_switch");

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

    let buffer = create_buffer(1000);
    let buffers = buffer_to_map(buffer);

    let normal_mode = ModeState::normal();
    let insert_mode = ModeState::insert();

    group.bench_function("normal_insert_normal", |b| {
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
                        black_box(&fold_manager),
                        black_box(&indent_analyzer),
                        black_box(&settings_menu),
                    )
                    .unwrap();

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

/// Benchmark with completion popup visible
pub fn bench_completion_popup(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_completion");

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
    let fold_manager = FoldManager::new();
    let indent_analyzer = IndentAnalyzer::new(4);
    let settings_menu = SettingsMenuState::new();

    let buffer = create_buffer(1000);
    let buffers = buffer_to_map(buffer);

    let mut completion = CompletionState::new();
    let items: Vec<CompletionItem> = (0..20)
        .map(|i| CompletionItem::new(format!("completion_item_{i}"), "buffer"))
        .collect();
    completion.activate(items, "comp".to_string(), 0, 50);

    group.bench_function("with_completion_popup", |b| {
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

    let no_completion = CompletionState::new();
    group.bench_function("without_completion_popup", |b| {
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
                        black_box(&no_completion),
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

/// Benchmark sustained rapid input (worst case scenario)
pub fn bench_sustained_input(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_sustained");
    group.sample_size(50);

    let theme = Theme::default();
    let highlight_store = HighlightStore::new();
    let color_mode = ColorMode::TrueColor;
    let mode = ModeState::insert();
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

    group.bench_function("100_keystrokes_each_rendered", |b| {
        b.iter_with_setup(
            || {
                let mut buffer = create_buffer(100);
                buffer.cur.y = 50;
                let writer = MockWriter::new();
                let screen = Screen::with_writer(writer, 120, 50);
                (buffer, screen)
            },
            |(mut buffer, mut screen)| {
                for i in 0..100u8 {
                    buffer.insert_char((b'a' + (i % 26)) as char);
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
                }
                screen.flush().unwrap();
                black_box((buffer, screen))
            },
        )
    });

    group.finish();
}
