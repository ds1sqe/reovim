//! Real render pipeline benchmarks
//!
//! Tests the actual `RenderData::from_buffer()` path that runs during scrolling.
//! This measures what users actually experience, not isolated functions.

use std::{hint::black_box, path::Path, sync::Arc};

use {
    criterion::{BenchmarkId, Criterion},
    reovim_core::{
        buffer::{Buffer, TextOps},
        content::WindowContentSource,
        frame::FrameRenderer,
        highlight::Style,
        modd::ModeState,
        render::RenderData,
        screen::{
            Position,
            window::{Anchor, LineNumber, SignColumnMode, Window},
        },
        syntax::SyntaxFactory,
    },
    reovim_plugin_treesitter::{SharedTreesitterManager, factory::TreesitterSyntaxFactory},
};

/// Setup treesitter manager with all available languages
fn setup_manager() -> Arc<SharedTreesitterManager> {
    let manager = Arc::new(SharedTreesitterManager::new());

    manager.with_mut(|m| {
        // Core languages
        m.register_language(Arc::new(reovim_lang_rust::RustLanguage));
        m.register_language(Arc::new(reovim_lang_c::CLanguage));
        m.register_language(Arc::new(reovim_lang_javascript::JavaScriptLanguage));
        m.register_language(Arc::new(reovim_lang_python::PythonLanguage));

        // Data/config languages
        m.register_language(Arc::new(reovim_lang_json::JsonLanguage));
        m.register_language(Arc::new(reovim_lang_toml::TomlLanguage));

        // Shell
        m.register_language(Arc::new(reovim_lang_bash::BashLanguage));

        // Markdown (with injections)
        m.register_language(Arc::new(reovim_lang_markdown::MarkdownLanguage));
        m.register_language(Arc::new(reovim_lang_markdown::MarkdownInlineLanguage));
    });

    manager
}

/// Create a buffer with syntax provider attached
fn create_buffer_with_syntax(
    manager: &Arc<SharedTreesitterManager>,
    content: &str,
    filename: &str,
) -> Buffer {
    let mut buffer = Buffer::empty(0);
    buffer.set_content(content);

    // Attach syntax provider
    let factory = TreesitterSyntaxFactory::new(Arc::clone(manager));
    if let Some(mut syntax) = factory.create_syntax(filename, content) {
        syntax.parse(content);
        buffer.attach_syntax(syntax);
    }

    buffer
}

/// Create a window at specific scroll position
fn create_window_at_scroll(height: u16, scroll_y: u16) -> Window {
    Window {
        id: 0,
        source: WindowContentSource::FileBuffer {
            buffer_id: 0,
            buffer_anchor: Anchor { x: 0, y: scroll_y },
        },
        anchor: Anchor { x: 0, y: 0 },
        width: 120,
        height,
        z_order: 0,
        is_active: true,
        is_floating: false,
        line_number: Some(LineNumber::default()),
        sign_column_mode: SignColumnMode::No,
        scrollbar_enabled: false,
        cursor: Position { x: 0, y: scroll_y },
        desired_col: None,
        border_config: None,
    }
}

/// Benchmark: Real RenderData::from_buffer() with markdown + syntax
///
/// This is what actually runs when user scrolls - the REAL path.
pub fn bench_render_data_markdown(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_data_real");
    let manager = setup_manager();

    // Test with CLAUDE.md
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("CLAUDE.md"));

    let content = match path.and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(c) => c,
        None => {
            eprintln!("Warning: Could not read CLAUDE.md");
            return;
        }
    };

    let line_count = content.lines().count();
    let buffer = create_buffer_with_syntax(&manager, &content, "CLAUDE.md");

    eprintln!(
        "CLAUDE.md: {} lines, syntax attached: {}",
        line_count,
        buffer.syntax().is_some()
    );

    // Test at different scroll positions
    for scroll_pos in [0, 50, 150, 300] {
        if scroll_pos >= line_count as u16 {
            continue;
        }

        let window = create_window_at_scroll(50, scroll_pos);

        group.bench_with_input(
            BenchmarkId::new("scroll_pos", scroll_pos),
            &(&window, &buffer),
            |b, (window, buffer)| {
                b.iter(|| {
                    let mode = ModeState::normal();
                    let render_data =
                        RenderData::from_buffer(black_box(window), black_box(buffer), &mode);
                    black_box(render_data)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: RenderData with CHANGELOG.md (larger file)
pub fn bench_render_data_changelog(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_data_changelog");
    let manager = setup_manager();

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("CHANGELOG.md"));

    let content = match path.and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(c) => c,
        None => {
            eprintln!("Warning: Could not read CHANGELOG.md");
            return;
        }
    };

    let line_count = content.lines().count();
    let buffer = create_buffer_with_syntax(&manager, &content, "CHANGELOG.md");

    eprintln!(
        "CHANGELOG.md: {} lines, syntax attached: {}",
        line_count,
        buffer.syntax().is_some()
    );

    // Test at different scroll positions
    for scroll_pos in [0, 100, 500, 1000, 1500] {
        if scroll_pos >= line_count as u16 {
            continue;
        }

        let window = create_window_at_scroll(50, scroll_pos);

        group.bench_with_input(
            BenchmarkId::new("scroll_pos", scroll_pos),
            &(&window, &buffer),
            |b, (window, buffer)| {
                b.iter(|| {
                    let mode = ModeState::normal();
                    let render_data =
                        RenderData::from_buffer(black_box(window), black_box(buffer), &mode);
                    black_box(render_data)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Simulated rapid "jjjjj" scroll with RenderData
///
/// Simulates holding 'j' to scroll - tests sequential render calls.
pub fn bench_render_data_jjjjj(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_data_jjjjj");
    let manager = setup_manager();

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("CLAUDE.md"));

    let content = match path.and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(c) => c,
        None => {
            eprintln!("Warning: Could not read CLAUDE.md");
            return;
        }
    };

    let buffer = create_buffer_with_syntax(&manager, &content, "CLAUDE.md");

    // Simulate 20 rapid 'j' presses (line-by-line scroll)
    group.bench_function("20_j_presses", |b| {
        b.iter(|| {
            for scroll_pos in 0..20u16 {
                let window = create_window_at_scroll(50, scroll_pos);
                let mode = ModeState::normal();
                let render_data =
                    RenderData::from_buffer(black_box(&window), black_box(&buffer), &mode);
                black_box(render_data);
            }
        });
    });

    // Simulate 50 rapid 'j' presses
    group.bench_function("50_j_presses", |b| {
        b.iter(|| {
            for scroll_pos in 0..50u16 {
                let window = create_window_at_scroll(50, scroll_pos);
                let mode = ModeState::normal();
                let render_data =
                    RenderData::from_buffer(black_box(&window), black_box(&buffer), &mode);
                black_box(render_data);
            }
        });
    });

    group.finish();
}

/// Benchmark: Compare with vs without syntax provider
///
/// Shows the overhead of syntax highlighting in the render path.
pub fn bench_render_data_syntax_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_data_syntax_overhead");
    let manager = setup_manager();

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("CLAUDE.md"));

    let content = match path.and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(c) => c,
        None => {
            eprintln!("Warning: Could not read CLAUDE.md");
            return;
        }
    };

    // Buffer WITHOUT syntax
    let mut buffer_no_syntax = Buffer::empty(0);
    buffer_no_syntax.set_content(&content);

    // Buffer WITH syntax
    let buffer_with_syntax = create_buffer_with_syntax(&manager, &content, "CLAUDE.md");

    let window = create_window_at_scroll(50, 100);

    group.bench_function("without_syntax", |b| {
        b.iter(|| {
            let mode = ModeState::normal();
            let render_data =
                RenderData::from_buffer(black_box(&window), black_box(&buffer_no_syntax), &mode);
            black_box(render_data)
        });
    });

    group.bench_function("with_syntax", |b| {
        b.iter(|| {
            let mode = ModeState::normal();
            let render_data =
                RenderData::from_buffer(black_box(&window), black_box(&buffer_with_syntax), &mode);
            black_box(render_data)
        });
    });

    group.finish();
}

/// Benchmark: FrameBuffer operations (fill + diff + flush)
///
/// Tests the frame rendering operations that happen AFTER RenderData creation.
/// This reveals additional overhead not captured by RenderData benchmarks.
pub fn bench_frame_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_operations");

    // Create a 120x50 frame renderer (typical terminal size)
    let mut renderer = FrameRenderer::new(120, 50);
    let style = Style::default();

    // Benchmark: Fill buffer with content (simulates render_data_to_framebuffer)
    group.bench_function("buffer_fill_6000_cells", |b| {
        b.iter(|| {
            renderer.clear();
            let buf = renderer.buffer_mut();
            for y in 0..50u16 {
                for x in 0..120u16 {
                    buf.put_char(x, y, 'x', &style);
                }
            }
            black_box(());
        });
    });

    // Benchmark: Flush to mock writer (diff + ANSI generation + write)
    // First, fill the buffer once
    {
        renderer.clear();
        let buf = renderer.buffer_mut();
        for y in 0..50u16 {
            for x in 0..120u16 {
                buf.put_char(x, y, 'a', &style);
            }
        }
    }
    let mut mock_writer: Vec<u8> = Vec::with_capacity(100_000);

    // First flush establishes baseline
    let _ = renderer.flush(&mut mock_writer);
    mock_writer.clear();

    group.bench_function("flush_full_screen", |b| {
        b.iter(|| {
            // Modify buffer to force full diff
            renderer.clear();
            let buf = renderer.buffer_mut();
            for y in 0..50u16 {
                for x in 0..120u16 {
                    buf.put_char(x, y, 'x', &style);
                }
            }
            // Flush (diff + write)
            renderer.flush(&mut mock_writer).unwrap();
            mock_writer.clear();
            black_box(());
        });
    });

    // Benchmark: Partial screen update (scrolling scenario - only some cells change)
    group.bench_function("flush_scroll_simulation", |b| {
        b.iter(|| {
            // Only change the top line (simulates scrolling by 1 line)
            let buf = renderer.buffer_mut();
            for x in 0..120u16 {
                buf.put_char(x, 0, 'n', &style); // new line at top
            }
            renderer.flush(&mut mock_writer).unwrap();
            mock_writer.clear();
            black_box(());
        });
    });

    group.finish();
}

/// Benchmark: Large synthetic file (5000 lines)
///
/// Reveals O(n) scaling issue in full-document syntax highlighting.
pub fn bench_large_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_file");
    let manager = setup_manager();

    // Generate large markdown content (5000 lines)
    let mut content = String::new();
    for i in 0..500 {
        content.push_str(&format!("# Heading {}\n\n", i));
        content.push_str("This is a paragraph with some text content for benchmarking.\n");
        content.push_str("Another line of text to make it more realistic.\n\n");

        // Add code block every 50 headings
        if i % 50 == 0 {
            content.push_str("```rust\n");
            content.push_str("fn example() {\n");
            content.push_str("    let x = 42;\n");
            content.push_str("    println!(\"value: {}\", x);\n");
            content.push_str("}\n");
            content.push_str("```\n\n");
        }
    }

    let line_count = content.lines().count();
    let buffer = create_buffer_with_syntax(&manager, &content, "large.md");

    eprintln!(
        "Generated large.md: {} lines, syntax attached: {}",
        line_count,
        buffer.syntax().is_some()
    );

    let window = create_window_at_scroll(50, 0);

    group.bench_function("5000_lines_render_data", |b| {
        b.iter(|| {
            let mode = ModeState::normal();
            let render_data =
                RenderData::from_buffer(black_box(&window), black_box(&buffer), &mode);
            black_box(render_data)
        });
    });

    // Test jjjjj on large file
    group.bench_function("5000_lines_20_j_presses", |b| {
        b.iter(|| {
            for scroll_pos in 0..20u16 {
                let window = create_window_at_scroll(50, scroll_pos);
                let mode = ModeState::normal();
                let render_data =
                    RenderData::from_buffer(black_box(&window), black_box(&buffer), &mode);
                black_box(render_data);
            }
        });
    });

    group.finish();
}

/// Benchmark: Complete render cycle (RenderData + FrameBuffer + Flush)
///
/// Shows the REAL total time per scroll including all phases.
pub fn bench_complete_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("complete_cycle");
    let manager = setup_manager();

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("CLAUDE.md"));

    let content = match path.and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(c) => c,
        None => {
            eprintln!("Warning: Could not read CLAUDE.md");
            return;
        }
    };

    let buffer = create_buffer_with_syntax(&manager, &content, "CLAUDE.md");
    let style = Style::default();
    let mut renderer = FrameRenderer::new(120, 50);
    let mut mock_writer: Vec<u8> = Vec::with_capacity(100_000);

    // Initialize renderer
    let _ = renderer.flush(&mut mock_writer);
    mock_writer.clear();

    group.bench_function("full_scroll_cycle", |b| {
        let mut scroll_pos = 0u16;
        b.iter(|| {
            // Phase 1: Create RenderData (syntax highlighting)
            let window = create_window_at_scroll(50, scroll_pos);
            let mode = ModeState::normal();
            let render_data = RenderData::from_buffer(&window, &buffer, &mode);

            // Phase 2: Write to FrameBuffer (simulated)
            renderer.clear();
            let buf = renderer.buffer_mut();
            for (y, line) in render_data.lines.iter().take(50).enumerate() {
                for (x, ch) in line.chars().take(120).enumerate() {
                    buf.put_char(x as u16, y as u16, ch, &style);
                }
            }

            // Phase 3: Flush (diff + ANSI + write)
            renderer.flush(&mut mock_writer).unwrap();
            mock_writer.clear();

            scroll_pos = (scroll_pos + 1) % 300;
            black_box(());
        });
    });

    group.finish();
}
