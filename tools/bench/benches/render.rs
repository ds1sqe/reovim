//! Render performance benchmarks for reovim
//!
//! Run with: `cargo bench -p reovim-bench`
//!
//! Benchmark groups:
//! - window_*: `Window::render()` benchmarks (no syntax)
//! - render_data_*: Real render pipeline benchmarks (RenderData::from_buffer with syntax)

mod bench_modules;

use {criterion::Criterion, std::time::Duration};

use bench_modules::window::{
    bench_render_throughput, bench_window_render, bench_window_viewport_size,
    bench_window_with_highlights,
};

use bench_modules::render_pipeline::{
    bench_complete_cycle, bench_frame_operations, bench_large_file, bench_render_data_changelog,
    bench_render_data_jjjjj, bench_render_data_markdown, bench_render_data_syntax_overhead,
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

fn main() {
    let mut criterion = Criterion::default()
        .measurement_time(Duration::from_secs(1))
        .warm_up_time(Duration::from_millis(200))
        .sample_size(30)
        .configure_from_args();

    window_benches(&mut criterion);
    render_pipeline_benches(&mut criterion);

    criterion.final_summary();
}
