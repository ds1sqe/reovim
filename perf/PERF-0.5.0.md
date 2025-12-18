# Performance Benchmarks v0.5.0

> Last updated: 2025-12-18T12:49:33Z | Commit: f19bf7f | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.5.0"
commit = "f19bf7f"
date = "2025-12-18T12:49:33Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 219.64 µs |
| buffer_vec | 4 | 215.07 µs |
| file_io | 2 | 95.92 µs |
| file_io_viewport | 3 | 24.93 µs |
| input_completion | 2 | 20.91 µs |
| input_mode_switch | 1 | 46.85 µs |
| input_scrolling | 3 | 501.81 ms |
| input_sustained | 1 | 3.00 ms |
| input_typing | 2 | 23.95 µs |
| io_bytes | 1 | 14.50 µs |
| rtt_explorer | 3 | 20.29 µs |
| rtt_input_lag | 2 | 58.31 µs |
| rtt_movement_lag | 5 | 494.58 µs |
| screen_io | 3 | 15.28 µs |
| screen_viewport_io | 3 | 17.59 µs |
| stress_completion | 1 | 530.14 µs |
| stress_editing | 3 | 31.71 ms |
| stress_mode_ops | 1 | 2.98 ms |
| stress_scroll | 3 | 17.10 ms |
| stress_worst_case | 1 | 38.38 ms |
| throughput | 1 | 6.94 µs |
| viewport_size | 4 | 12.60 µs |
| window_render | 4 | 6.04 µs |
| with_highlights | 1 | 7.84 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 6.50, median = 5.80, std_dev = 1.18, unit = "µs" }
clone_1000 = { mean = 75.89, median = 71.27, std_dev = 9.36, unit = "µs" }
clone_10000 = { mean = 792.15, median = 718.75, std_dev = 116.64, unit = "µs" }
clone_50000 = { mean = 4.04, median = 3.75, std_dev = 0.51, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 6.53, median = 6.49, std_dev = 0.67, unit = "µs" }
vec_clone_1000 = { mean = 76.79, median = 76.91, std_dev = 6.01, unit = "µs" }
vec_clone_10000 = { mean = 772.93, median = 781.69, std_dev = 59.73, unit = "µs" }
vec_clone_50000 = { mean = 4.01, median = 4.03, std_dev = 0.31, unit = "ms" }

[results.file_io]
buffered_file = { mean = 22.18, median = 22.18, std_dev = 0.19, unit = "µs" }
unbuffered_file = { mean = 169.65, median = 164.48, std_dev = 17.45, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 36.19, median = 34.99, std_dev = 4.70, unit = "µs" }
viewport_24 = { mean = 15.50, median = 15.47, std_dev = 0.18, unit = "µs" }
viewport_50 = { mean = 23.11, median = 22.30, std_dev = 2.54, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 26.09, median = 25.17, std_dev = 3.04, unit = "µs" }
without_completion_popup = { mean = 15.74, median = 15.18, std_dev = 1.86, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 46.85, median = 44.92, std_dev = 6.16, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 7.61, median = 7.34, std_dev = 0.78, unit = "ms" }
scroll_half_page = { mean = 750.69, median = 735.53, std_dev = 57.20, unit = "µs" }
scroll_one_line = { mean = 747.14, median = 735.23, std_dev = 46.23, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 3.00, median = 2.83, std_dev = 0.45, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 24.74, median = 24.27, std_dev = 2.25, unit = "µs" }
single_char_render = { mean = 23.17, median = 23.18, std_dev = 0.08, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 14.50, median = 14.49, std_dev = 0.07, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 15.64, median = 15.07, std_dev = 1.84, unit = "µs" }
open_explorer = { mean = 15.17, median = 14.55, std_dev = 2.10, unit = "µs" }
toggle_cycle = { mean = 30.05, median = 30.05, std_dev = 0.02, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 61.04, median = 55.41, std_dev = 10.84, unit = "µs" }
char_insert_rtt = { mean = 55.58, median = 55.51, std_dev = 0.31, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 759.19, median = 753.32, std_dev = 31.53, unit = "µs" }
half_page_down_rtt = { mean = 788.15, median = 750.92, std_dev = 85.73, unit = "µs" }
move_down_rtt = { mean = 751.31, median = 750.93, std_dev = 2.24, unit = "µs" }
move_right_rtt = { mean = 84.93, median = 84.84, std_dev = 0.40, unit = "µs" }
word_forward_rtt = { mean = 89.31, median = 85.99, std_dev = 9.61, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 15.72, median = 15.17, std_dev = 1.95, unit = "µs" }
full_render_1000 = { mean = 15.03, median = 14.48, std_dev = 1.85, unit = "µs" }
full_render_10000 = { mean = 15.10, median = 14.54, std_dev = 1.91, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 27.03, median = 25.64, std_dev = 4.11, unit = "µs" }
viewport_height_24 = { mean = 8.96, median = 8.95, std_dev = 0.02, unit = "µs" }
viewport_height_50 = { mean = 16.79, median = 15.15, std_dev = 3.89, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 530.14, median = 484.16, std_dev = 98.81, unit = "µs" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 15.05, median = 15.05, std_dev = 0.02, unit = "ms" }
edit_navigate_cycle_1k = { mean = 1.94, median = 1.90, std_dev = 0.13, unit = "ms" }
edit_navigate_cycle_50k = { mean = 78.13, median = 75.50, std_dev = 7.33, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 2.98, median = 2.85, std_dev = 0.34, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 37.38, median = 37.37, std_dev = 0.04, unit = "ms" }
jump_around_file = { mean = 6.31, median = 5.97, std_dev = 0.84, unit = "ms" }
spam_ctrl_d_10_times = { mean = 7.61, median = 7.42, std_dev = 0.72, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 38.38, median = 37.66, std_dev = 3.10, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 6.94, median = 6.57, std_dev = 1.12, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 13.42, median = 12.83, std_dev = 1.79, unit = "µs" }
height_200 = { mean = 26.09, median = 25.15, std_dev = 3.27, unit = "µs" }
height_24 = { mean = 3.37, median = 3.23, std_dev = 0.43, unit = "µs" }
height_50 = { mean = 7.51, median = 7.21, std_dev = 0.99, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 1.63, median = 1.63, std_dev = 0.01, unit = "µs" }
buffer_lines_100 = { mean = 7.46, median = 7.46, std_dev = 0.03, unit = "µs" }
buffer_lines_1000 = { mean = 8.16, median = 7.47, std_dev = 1.50, unit = "µs" }
buffer_lines_10000 = { mean = 6.91, median = 6.60, std_dev = 1.00, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 7.84, median = 7.51, std_dev = 1.13, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 6.50 µs | 5.80 µs | 1.18 µs | [6.11, 6.93] µs |
| buffer_clone/clone/1000 | 75.89 µs | 71.27 µs | 9.36 µs | [72.89, 79.39] µs |
| buffer_clone/clone/10000 | 792.15 µs | 718.75 µs | 116.64 µs | [753.48, 835.85] µs |
| buffer_clone/clone/50000 | 4.04 ms | 3.75 ms | 0.51 ms | [3.87, 4.23] ms |
| buffer_vec/vec_clone/100 | 6.53 µs | 6.49 µs | 0.67 µs | [6.32, 6.79] µs |
| buffer_vec/vec_clone/1000 | 76.79 µs | 76.91 µs | 6.01 µs | [74.80, 79.05] µs |
| buffer_vec/vec_clone/10000 | 772.93 µs | 781.69 µs | 59.73 µs | [753.12, 795.06] µs |
| buffer_vec/vec_clone/50000 | 4.01 ms | 4.03 ms | 0.31 ms | [3.91, 4.12] ms |
| file_io/buffered_file | 22.18 µs | 22.18 µs | 0.19 µs | [22.12, 22.25] µs |
| file_io/unbuffered_file | 169.65 µs | 164.48 µs | 17.45 µs | [164.47, 176.60] µs |
| file_io_viewport/viewport/100 | 36.19 µs | 34.99 µs | 4.70 µs | [34.92, 38.11] µs |
| file_io_viewport/viewport/24 | 15.50 µs | 15.47 µs | 0.18 µs | [15.44, 15.57] µs |
| file_io_viewport/viewport/50 | 23.11 µs | 22.30 µs | 2.54 µs | [22.36, 24.13] µs |
| input_completion/with_completion_popup | 26.09 µs | 25.17 µs | 3.04 µs | [25.19, 27.31] µs |
| input_completion/without_completion_popup | 15.74 µs | 15.18 µs | 1.86 µs | [15.22, 16.50] µs |
| input_mode_switch/normal_insert_normal | 46.85 µs | 44.92 µs | 6.16 µs | [44.91, 49.27] µs |
| input_scrolling/scroll_10_lines | 7.61 ms | 7.34 ms | 0.78 ms | [7.36, 7.91] ms |
| input_scrolling/scroll_half_page | 750.69 µs | 735.53 µs | 57.20 µs | [735.66, 774.04] µs |
| input_scrolling/scroll_one_line | 747.14 µs | 735.23 µs | 46.23 µs | [735.29, 766.25] µs |
| input_sustained/100_keystrokes_each_rendered | 3.00 ms | 2.83 ms | 0.45 ms | [2.89, 3.14] ms |
| input_typing/burst_10_chars_render | 24.74 µs | 24.27 µs | 2.25 µs | [24.27, 25.61] µs |
| input_typing/single_char_render | 23.17 µs | 23.18 µs | 0.08 µs | [23.14, 23.20] µs |
| io_bytes/full_screen_render | 14.50 µs | 14.49 µs | 0.07 µs | [14.48, 14.53] µs |
| rtt_explorer/close_explorer | 15.64 µs | 15.07 µs | 1.84 µs | [15.11, 16.39] µs |
| rtt_explorer/open_explorer | 15.17 µs | 14.55 µs | 2.10 µs | [14.52, 16.01] µs |
| rtt_explorer/toggle_cycle | 30.05 µs | 30.05 µs | 0.02 µs | [30.04, 30.06] µs |
| rtt_input_lag/backspace_rtt | 61.04 µs | 55.41 µs | 10.84 µs | [57.50, 65.11] µs |
| rtt_input_lag/char_insert_rtt | 55.58 µs | 55.51 µs | 0.31 µs | [55.48, 55.70] µs |
| rtt_movement_lag/goto_top_rtt | 759.19 µs | 753.32 µs | 31.53 µs | [752.98, 771.03] µs |
| rtt_movement_lag/half_page_down_rtt | 788.15 µs | 750.92 µs | 85.73 µs | [761.52, 820.87] µs |
| rtt_movement_lag/move_down_rtt | 751.31 µs | 750.93 µs | 2.24 µs | [750.53, 752.10] µs |
| rtt_movement_lag/move_right_rtt | 84.93 µs | 84.84 µs | 0.40 µs | [84.82, 85.09] µs |
| rtt_movement_lag/word_forward_rtt | 89.31 µs | 85.99 µs | 9.61 µs | [86.28, 93.14] µs |
| screen_io/full_render/100 | 15.72 µs | 15.17 µs | 1.95 µs | [15.17, 16.50] µs |
| screen_io/full_render/1000 | 15.03 µs | 14.48 µs | 1.85 µs | [14.47, 15.78] µs |
| screen_io/full_render/10000 | 15.10 µs | 14.54 µs | 1.91 µs | [14.53, 15.87] µs |
| screen_viewport_io/viewport_height/100 | 27.03 µs | 25.64 µs | 4.11 µs | [25.73, 28.62] µs |
| screen_viewport_io/viewport_height/24 | 8.96 µs | 8.95 µs | 0.02 µs | [8.95, 8.96] µs |
| screen_viewport_io/viewport_height/50 | 16.79 µs | 15.15 µs | 3.89 µs | [15.57, 18.31] µs |
| stress_completion/completion_scroll_100_items | 530.14 µs | 484.16 µs | 98.81 µs | [498.36, 567.42] µs |
| stress_editing/edit_navigate_cycle_10k | 15.05 ms | 15.05 ms | 0.02 ms | [15.04, 15.06] ms |
| stress_editing/edit_navigate_cycle_1k | 1.94 ms | 1.90 ms | 0.13 ms | [1.90, 1.99] ms |
| stress_editing/edit_navigate_cycle_50k | 78.13 ms | 75.50 ms | 7.33 ms | [75.89, 80.95] ms |
| stress_mode_ops/insert_escape_move_cycle | 2.98 ms | 2.85 ms | 0.34 ms | [2.88, 3.11] ms |
| stress_scroll/hold_j_50_lines_10k_file | 37.38 ms | 37.37 ms | 0.04 ms | [37.36, 37.39] ms |
| stress_scroll/jump_around_file | 6.31 ms | 5.97 ms | 0.84 ms | [6.05, 6.63] ms |
| stress_scroll/spam_ctrl_d_10_times | 7.61 ms | 7.42 ms | 0.72 ms | [7.42, 7.90] ms |
| stress_worst_case/all_features_50k_file | 38.38 ms | 37.66 ms | 3.10 ms | [37.65, 39.81] ms |
| throughput/renders_per_second | 6.94 µs | 6.57 µs | 1.12 µs | [6.58, 7.38] µs |
| viewport_size/height/100 | 13.42 µs | 12.83 µs | 1.79 µs | [12.86, 14.12] µs |
| viewport_size/height/200 | 26.09 µs | 25.15 µs | 3.27 µs | [25.19, 27.40] µs |
| viewport_size/height/24 | 3.37 µs | 3.23 µs | 0.43 µs | [3.25, 3.54] µs |
| viewport_size/height/50 | 7.51 µs | 7.21 µs | 0.99 µs | [7.21, 7.92] µs |
| window_render/buffer_lines/10 | 1.63 µs | 1.63 µs | 0.01 µs | [1.63, 1.64] µs |
| window_render/buffer_lines/100 | 7.46 µs | 7.46 µs | 0.03 µs | [7.46, 7.47] µs |
| window_render/buffer_lines/1000 | 8.16 µs | 7.47 µs | 1.50 µs | [7.68, 8.71] µs |
| window_render/buffer_lines/10000 | 6.91 µs | 6.60 µs | 1.00 µs | [6.62, 7.30] µs |
| with_highlights/no_highlights | 7.84 µs | 7.51 µs | 1.13 µs | [7.52, 8.29] µs |
