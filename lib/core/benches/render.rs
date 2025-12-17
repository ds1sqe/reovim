//! Render performance benchmarks for reovim
//!
//! Run with: `cargo bench -p reovim-core`
//!
//! Benchmark groups:
//! - window_*: Window::render() benchmarks
//! - screen_*: Full screen I/O benchmarks
//! - input_*: Input simulation benchmarks
//! - rtt_*: Round-trip time benchmarks
//! - stress_*: Stress test benchmarks
//! - buffer_*: Buffer operation benchmarks

mod bench_modules;

use criterion::{criterion_group, criterion_main};

// Window render benchmarks
use bench_modules::window::{
    bench_render_throughput,
    bench_window_render,
    bench_window_viewport_size,
    bench_window_with_highlights,
};

// Screen I/O benchmarks
use bench_modules::screen::{
    bench_file_io,
    bench_file_io_viewport,
    bench_io_bytes_written,
    bench_screen_render_full_io,
    bench_screen_viewport_io,
};

// Input simulation benchmarks
use bench_modules::input::{
    bench_completion_popup,
    bench_mode_switching,
    bench_scrolling_simulation,
    bench_sustained_input,
    bench_typing_simulation,
};

// RTT benchmarks
use bench_modules::rtt::{
    bench_rtt_explorer_toggle,
    bench_rtt_input_lag,
    bench_rtt_movement_lag,
};

// Stress test benchmarks
use bench_modules::stress::{
    bench_buffer_clone_cost,
    bench_buffer_vec_cost,
    bench_stress_completion,
    bench_stress_editing_session,
    bench_stress_mode_operations,
    bench_stress_rapid_scroll,
    bench_stress_worst_case,
};

criterion_group!(
    window_benches,
    bench_window_render,
    bench_window_viewport_size,
    bench_window_with_highlights,
    bench_render_throughput,
);

criterion_group!(
    screen_benches,
    bench_screen_render_full_io,
    bench_io_bytes_written,
    bench_screen_viewport_io,
    bench_file_io,
    bench_file_io_viewport,
);

criterion_group!(
    input_benches,
    bench_typing_simulation,
    bench_scrolling_simulation,
    bench_mode_switching,
    bench_completion_popup,
    bench_sustained_input,
);

criterion_group!(
    rtt_benches,
    bench_rtt_explorer_toggle,
    bench_rtt_input_lag,
    bench_rtt_movement_lag,
);

criterion_group!(
    stress_benches,
    bench_stress_editing_session,
    bench_stress_rapid_scroll,
    bench_stress_mode_operations,
    bench_stress_completion,
    bench_stress_worst_case,
    bench_buffer_clone_cost,
    bench_buffer_vec_cost,
);

criterion_main!(
    window_benches,
    screen_benches,
    input_benches,
    rtt_benches,
    stress_benches,
);
