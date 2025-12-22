//! Render performance benchmarks for reovim
//!
//! Run with: `cargo bench -p reovim-core`
//!
//! Benchmark groups:
//! - window_*: `Window::render()` benchmarks
//! - screen_*: Full screen I/O benchmarks
//! - input_*: Input simulation benchmarks
//! - rtt_*: Round-trip time benchmarks
//! - stress_*: Stress test benchmarks
//! - buffer_*: Buffer operation benchmarks

mod bench_modules;

use {criterion::Criterion, std::time::Duration};

// Window render benchmarks
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

fn main() {
    let mut criterion = Criterion::default()
        .measurement_time(Duration::from_secs(1))
        .warm_up_time(Duration::from_millis(200))
        .sample_size(30)
        .configure_from_args();

    window_benches(&mut criterion);

    criterion.final_summary();
}
