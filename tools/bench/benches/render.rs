//! Performance benchmarks for reovim
//!
//! Run with: `cargo bench -p reovim-bench`
//!
//! Benchmark groups:
//! - window_*: `Window::render()` benchmarks (no syntax)
//! - render_data_*: Real render pipeline benchmarks (RenderData::from_buffer with syntax)
//! - event_bus_*: Event bus dispatch latency benchmarks (Issue #86)
//! - treesitter_*: Query compilation time benchmarks (Issue #86)

mod bench_modules;

use {criterion::Criterion, std::time::Duration};

use bench_modules::event_bus::{
    bench_blocking_handler, bench_dispatch_consumed, bench_dispatch_handler_count,
    bench_dispatch_many_event_types, bench_dispatch_priority_sorted, bench_dispatch_with_emit,
    bench_dyn_event_creation, bench_handler_registration, bench_latency_with_without_blocking,
    bench_queue_behind_slow_handler, bench_rwlock_acquisition, bench_sequential_slow_handlers,
};

use bench_modules::render_pipeline::{
    bench_complete_cycle, bench_frame_operations, bench_large_file, bench_render_data_changelog,
    bench_render_data_jjjjj, bench_render_data_markdown, bench_render_data_syntax_overhead,
};

use bench_modules::treesitter_init::{
    bench_all_languages_registration, bench_language_registration, bench_query_compilation,
};

use bench_modules::window::{
    bench_render_throughput, bench_window_render, bench_window_viewport_size,
    bench_window_with_highlights,
};

fn window_benches(c: &mut Criterion) {
    bench_window_render(c);
    bench_window_viewport_size(c);
    bench_window_with_highlights(c);
    bench_render_throughput(c);
}

fn render_pipeline_benches(c: &mut Criterion) {
    bench_render_data_markdown(c);
    bench_render_data_changelog(c);
    bench_render_data_jjjjj(c);
    bench_render_data_syntax_overhead(c);
    bench_frame_operations(c);
    bench_large_file(c);
    bench_complete_cycle(c);
}

fn event_bus_benches(c: &mut Criterion) {
    bench_dyn_event_creation(c);
    bench_dispatch_handler_count(c);
    bench_dispatch_many_event_types(c);
    bench_dispatch_priority_sorted(c);
    bench_dispatch_with_emit(c);
    bench_dispatch_consumed(c);
    bench_rwlock_acquisition(c);
    bench_handler_registration(c);
}

/// Bottleneck simulation benchmarks
///
/// These benchmarks simulate the actual issue (#86) where slow handlers
/// block fast event dispatch. Use these to verify improvements from:
/// - Issue #93: Lazy query compilation
/// - Issue #94: Background query compilation
/// - Issue #95: Priority event channel
fn bottleneck_benches(c: &mut Criterion) {
    bench_blocking_handler(c);
    bench_queue_behind_slow_handler(c);
    bench_sequential_slow_handlers(c);
    bench_latency_with_without_blocking(c);
}

fn treesitter_init_benches(c: &mut Criterion) {
    bench_query_compilation(c);
    bench_language_registration(c);
    bench_all_languages_registration(c);
}

fn main() {
    let mut criterion = Criterion::default()
        .measurement_time(Duration::from_secs(1))
        .warm_up_time(Duration::from_millis(200))
        .sample_size(30)
        .configure_from_args();

    window_benches(&mut criterion);
    render_pipeline_benches(&mut criterion);
    event_bus_benches(&mut criterion);
    treesitter_init_benches(&mut criterion);
    bottleneck_benches(&mut criterion);

    criterion.final_summary();
}
