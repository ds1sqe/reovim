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

// Screen I/O benchmarks
use bench_modules::screen::{
    bench_file_io, bench_file_io_viewport, bench_io_bytes_written, bench_screen_render_full_io,
    bench_screen_viewport_io,
};

// Input simulation benchmarks
use bench_modules::input::{
    bench_completion_popup, bench_mode_switching, bench_scrolling_simulation,
    bench_sustained_input, bench_typing_simulation,
};

// RTT benchmarks
use bench_modules::rtt::{bench_rtt_explorer_toggle, bench_rtt_input_lag, bench_rtt_movement_lag};

// Stress test benchmarks
use bench_modules::stress::{
    bench_buffer_clone_cost, bench_buffer_vec_cost, bench_stress_completion,
    bench_stress_editing_session, bench_stress_mode_operations, bench_stress_rapid_scroll,
    bench_stress_worst_case,
};

fn window_benches(c: &mut Criterion) {
    bench_window_render(c);
    bench_window_viewport_size(c);
    bench_window_with_highlights(c);
    bench_render_throughput(c);
}

fn screen_benches(c: &mut Criterion) {
    bench_screen_render_full_io(c);
    bench_io_bytes_written(c);
    bench_screen_viewport_io(c);
    bench_file_io(c);
    bench_file_io_viewport(c);
}

fn input_benches(c: &mut Criterion) {
    bench_typing_simulation(c);
    bench_scrolling_simulation(c);
    bench_mode_switching(c);
    bench_completion_popup(c);
    bench_sustained_input(c);
}

fn rtt_benches(c: &mut Criterion) {
    bench_rtt_explorer_toggle(c);
    bench_rtt_input_lag(c);
    bench_rtt_movement_lag(c);
}

fn stress_benches(c: &mut Criterion) {
    bench_stress_editing_session(c);
    bench_stress_rapid_scroll(c);
    bench_stress_mode_operations(c);
    bench_stress_completion(c);
    bench_stress_worst_case(c);
    bench_buffer_clone_cost(c);
    bench_buffer_vec_cost(c);
}

fn main() {
    let mut criterion = Criterion::default()
        .measurement_time(Duration::from_secs(1))
        .warm_up_time(Duration::from_millis(200))
        .sample_size(30)
        .configure_from_args();

    window_benches(&mut criterion);
    screen_benches(&mut criterion);
    input_benches(&mut criterion);
    rtt_benches(&mut criterion);
    stress_benches(&mut criterion);

    criterion.final_summary();
}
