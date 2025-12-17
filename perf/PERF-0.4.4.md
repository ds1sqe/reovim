# Performance Benchmarks v0.4.3

> Last updated: 2025-12-17T07:23:24Z | Commit: babbd26 | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.4.3"
commit = "babbd26"
date = "2025-12-17T07:23:24Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.5-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 96.76 µs |
| buffer_vec | 4 | 96.74 µs |
| file_io | 2 | 68.42 µs |
| file_io_viewport | 3 | 12.40 µs |
| input_completion | 2 | 8.25 µs |
| input_mode_switch | 1 | 17.90 µs |
| input_scrolling | 3 | 249.17 ms |
| input_sustained | 1 | 1.27 ms |
| input_typing | 2 | 9.86 µs |
| io_bytes | 1 | 6.01 µs |
| rtt_explorer | 3 | 7.91 µs |
| rtt_input_lag | 2 | 28.23 µs |
| rtt_movement_lag | 5 | 250.74 µs |
| screen_io | 3 | 6.01 µs |
| screen_viewport_io | 3 | 6.65 µs |
| stress_completion | 1 | 208.46 µs |
| stress_editing | 3 | 316.85 ms |
| stress_mode_ops | 1 | 1.36 ms |
| stress_scroll | 3 | 8.15 ms |
| stress_worst_case | 1 | 19.41 ms |
| throughput | 1 | 2.49 µs |
| viewport_size | 4 | 6.87 µs |
| window_render | 4 | 120.32 ns |
| with_highlights | 1 | 2.63 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 2.93, median = 2.91, std_dev = 0.04, unit = "µs" }
clone_1000 = { mean = 34.71, median = 34.66, std_dev = 0.37, unit = "µs" }
clone_10000 = { mean = 347.50, median = 347.47, std_dev = 2.79, unit = "µs" }
clone_50000 = { mean = 1.90, median = 1.88, std_dev = 0.06, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 2.91, median = 2.90, std_dev = 0.03, unit = "µs" }
vec_clone_1000 = { mean = 34.81, median = 34.76, std_dev = 0.47, unit = "µs" }
vec_clone_10000 = { mean = 347.35, median = 348.18, std_dev = 2.87, unit = "µs" }
vec_clone_50000 = { mean = 1.90, median = 1.89, std_dev = 0.06, unit = "ms" }

[results.file_io]
buffered_file = { mean = 11.26, median = 11.20, std_dev = 0.45, unit = "µs" }
unbuffered_file = { mean = 125.59, median = 125.93, std_dev = 2.03, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 17.94, median = 17.82, std_dev = 0.49, unit = "µs" }
viewport_24 = { mean = 8.17, median = 8.14, std_dev = 0.46, unit = "µs" }
viewport_50 = { mean = 11.10, median = 11.15, std_dev = 0.42, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 10.38, median = 10.35, std_dev = 0.15, unit = "µs" }
without_completion_popup = { mean = 6.11, median = 6.09, std_dev = 0.11, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 17.90, median = 17.98, std_dev = 0.24, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 3.57, median = 3.59, std_dev = 0.05, unit = "ms" }
scroll_half_page = { mean = 375.04, median = 375.96, std_dev = 7.07, unit = "µs" }
scroll_one_line = { mean = 368.89, median = 370.09, std_dev = 6.44, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 1.27, median = 1.27, std_dev = 0.02, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 10.12, median = 9.89, std_dev = 1.23, unit = "µs" }
single_char_render = { mean = 9.60, median = 9.56, std_dev = 0.21, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 6.01, median = 6.00, std_dev = 0.11, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 6.01, median = 5.99, std_dev = 0.07, unit = "µs" }
open_explorer = { mean = 6.02, median = 6.01, std_dev = 0.05, unit = "µs" }
toggle_cycle = { mean = 11.71, median = 11.75, std_dev = 0.18, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 28.15, median = 28.08, std_dev = 1.45, unit = "µs" }
char_insert_rtt = { mean = 28.31, median = 28.32, std_dev = 0.71, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 390.17, median = 392.14, std_dev = 4.67, unit = "µs" }
half_page_down_rtt = { mean = 389.83, median = 385.73, std_dev = 13.99, unit = "µs" }
move_down_rtt = { mean = 383.28, median = 382.40, std_dev = 9.49, unit = "µs" }
move_right_rtt = { mean = 44.67, median = 44.46, std_dev = 0.85, unit = "µs" }
word_forward_rtt = { mean = 45.76, median = 45.99, std_dev = 1.19, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 6.08, median = 6.07, std_dev = 0.06, unit = "µs" }
full_render_1000 = { mean = 5.97, median = 5.93, std_dev = 0.12, unit = "µs" }
full_render_10000 = { mean = 5.97, median = 5.94, std_dev = 0.09, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 10.55, median = 10.55, std_dev = 0.21, unit = "µs" }
viewport_height_24 = { mean = 3.52, median = 3.51, std_dev = 0.09, unit = "µs" }
viewport_height_50 = { mean = 5.88, median = 5.88, std_dev = 0.11, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 208.46, median = 207.85, std_dev = 3.27, unit = "µs" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 7.27, median = 7.23, std_dev = 0.11, unit = "ms" }
edit_navigate_cycle_1k = { mean = 905.69, median = 908.32, std_dev = 12.88, unit = "µs" }
edit_navigate_cycle_50k = { mean = 37.60, median = 37.76, std_dev = 0.67, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 1.36, median = 1.37, std_dev = 0.03, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 17.88, median = 17.89, std_dev = 0.31, unit = "ms" }
jump_around_file = { mean = 2.90, median = 2.89, std_dev = 0.05, unit = "ms" }
spam_ctrl_d_10_times = { mean = 3.67, median = 3.68, std_dev = 0.05, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 19.41, median = 19.36, std_dev = 0.36, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 2.49, median = 2.47, std_dev = 0.04, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 6.10, median = 6.07, std_dev = 0.58, unit = "µs" }
height_200 = { mean = 10.61, median = 10.61, std_dev = 0.24, unit = "µs" }
height_24 = { mean = 3.80, median = 1.22, std_dev = 3.13, unit = "µs" }
height_50 = { mean = 6.97, median = 7.43, std_dev = 1.34, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 473.31, median = 447.23, std_dev = 42.14, unit = "ns" }
buffer_lines_100 = { mean = 2.60, median = 2.59, std_dev = 0.04, unit = "µs" }
buffer_lines_1000 = { mean = 2.79, median = 2.78, std_dev = 0.06, unit = "µs" }
buffer_lines_10000 = { mean = 2.57, median = 2.54, std_dev = 0.09, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 2.63, median = 2.65, std_dev = 0.08, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 2.93 µs | 2.91 µs | 0.04 µs | [2.92, 2.93] µs |
| buffer_clone/clone/1000 | 34.71 µs | 34.66 µs | 0.37 µs | [34.64, 34.78] µs |
| buffer_clone/clone/10000 | 347.50 µs | 347.47 µs | 2.79 µs | [346.98, 348.07] µs |
| buffer_clone/clone/50000 | 1.90 ms | 1.88 ms | 0.06 ms | [1.89, 1.91] ms |
| buffer_vec/vec_clone/100 | 2.91 µs | 2.90 µs | 0.03 µs | [2.90, 2.91] µs |
| buffer_vec/vec_clone/1000 | 34.81 µs | 34.76 µs | 0.47 µs | [34.72, 34.90] µs |
| buffer_vec/vec_clone/10000 | 347.35 µs | 348.18 µs | 2.87 µs | [346.80, 347.91] µs |
| buffer_vec/vec_clone/50000 | 1.90 ms | 1.89 ms | 0.06 ms | [1.89, 1.92] ms |
| file_io/buffered_file | 11.26 µs | 11.20 µs | 0.45 µs | [11.17, 11.35] µs |
| file_io/unbuffered_file | 125.59 µs | 125.93 µs | 2.03 µs | [125.19, 125.98] µs |
| file_io_viewport/viewport/100 | 17.94 µs | 17.82 µs | 0.49 µs | [17.85, 18.04] µs |
| file_io_viewport/viewport/24 | 8.17 µs | 8.14 µs | 0.46 µs | [8.08, 8.26] µs |
| file_io_viewport/viewport/50 | 11.10 µs | 11.15 µs | 0.42 µs | [11.02, 11.18] µs |
| input_completion/with_completion_popup | 10.38 µs | 10.35 µs | 0.15 µs | [10.36, 10.41] µs |
| input_completion/without_completion_popup | 6.11 µs | 6.09 µs | 0.11 µs | [6.09, 6.13] µs |
| input_mode_switch/normal_insert_normal | 17.90 µs | 17.98 µs | 0.24 µs | [17.85, 17.94] µs |
| input_scrolling/scroll_10_lines | 3.57 ms | 3.59 ms | 0.05 ms | [3.56, 3.58] ms |
| input_scrolling/scroll_half_page | 375.04 µs | 375.96 µs | 7.07 µs | [373.65, 376.42] µs |
| input_scrolling/scroll_one_line | 368.89 µs | 370.09 µs | 6.44 µs | [367.64, 370.16] µs |
| input_sustained/100_keystrokes_each_rendered | 1.27 ms | 1.27 ms | 0.02 ms | [1.27, 1.28] ms |
| input_typing/burst_10_chars_render | 10.12 µs | 9.89 µs | 1.23 µs | [9.91, 10.39] µs |
| input_typing/single_char_render | 9.60 µs | 9.56 µs | 0.21 µs | [9.56, 9.64] µs |
| io_bytes/full_screen_render | 6.01 µs | 6.00 µs | 0.11 µs | [5.99, 6.03] µs |
| rtt_explorer/close_explorer | 6.01 µs | 5.99 µs | 0.07 µs | [5.99, 6.02] µs |
| rtt_explorer/open_explorer | 6.02 µs | 6.01 µs | 0.05 µs | [6.01, 6.03] µs |
| rtt_explorer/toggle_cycle | 11.71 µs | 11.75 µs | 0.18 µs | [11.67, 11.75] µs |
| rtt_input_lag/backspace_rtt | 28.15 µs | 28.08 µs | 1.45 µs | [27.90, 28.47] µs |
| rtt_input_lag/char_insert_rtt | 28.31 µs | 28.32 µs | 0.71 µs | [28.18, 28.45] µs |
| rtt_movement_lag/goto_top_rtt | 390.17 µs | 392.14 µs | 4.67 µs | [389.25, 391.07] µs |
| rtt_movement_lag/half_page_down_rtt | 389.83 µs | 385.73 µs | 13.99 µs | [387.15, 392.60] µs |
| rtt_movement_lag/move_down_rtt | 383.28 µs | 382.40 µs | 9.49 µs | [381.56, 385.27] µs |
| rtt_movement_lag/move_right_rtt | 44.67 µs | 44.46 µs | 0.85 µs | [44.50, 44.84] µs |
| rtt_movement_lag/word_forward_rtt | 45.76 µs | 45.99 µs | 1.19 µs | [45.53, 45.99] µs |
| screen_io/full_render/100 | 6.08 µs | 6.07 µs | 0.06 µs | [6.07, 6.09] µs |
| screen_io/full_render/1000 | 5.97 µs | 5.93 µs | 0.12 µs | [5.95, 6.00] µs |
| screen_io/full_render/10000 | 5.97 µs | 5.94 µs | 0.09 µs | [5.95, 5.99] µs |
| screen_viewport_io/viewport_height/100 | 10.55 µs | 10.55 µs | 0.21 µs | [10.51, 10.59] µs |
| screen_viewport_io/viewport_height/24 | 3.52 µs | 3.51 µs | 0.09 µs | [3.50, 3.54] µs |
| screen_viewport_io/viewport_height/50 | 5.88 µs | 5.88 µs | 0.11 µs | [5.85, 5.90] µs |
| stress_completion/completion_scroll_100_items | 208.46 µs | 207.85 µs | 3.27 µs | [207.32, 209.63] µs |
| stress_editing/edit_navigate_cycle_10k | 7.27 ms | 7.23 ms | 0.11 ms | [7.23, 7.31] ms |
| stress_editing/edit_navigate_cycle_1k | 905.69 µs | 908.32 µs | 12.88 µs | [900.85, 909.90] µs |
| stress_editing/edit_navigate_cycle_50k | 37.60 ms | 37.76 ms | 0.67 ms | [37.36, 37.84] ms |
| stress_mode_ops/insert_escape_move_cycle | 1.36 ms | 1.37 ms | 0.03 ms | [1.35, 1.37] ms |
| stress_scroll/hold_j_50_lines_10k_file | 17.88 ms | 17.89 ms | 0.31 ms | [17.76, 17.98] ms |
| stress_scroll/jump_around_file | 2.90 ms | 2.89 ms | 0.05 ms | [2.88, 2.92] ms |
| stress_scroll/spam_ctrl_d_10_times | 3.67 ms | 3.68 ms | 0.05 ms | [3.65, 3.69] ms |
| stress_worst_case/all_features_50k_file | 19.41 ms | 19.36 ms | 0.36 ms | [19.26, 19.56] ms |
| throughput/renders_per_second | 2.49 µs | 2.47 µs | 0.04 µs | [2.48, 2.50] µs |
| viewport_size/height/100 | 6.10 µs | 6.07 µs | 0.58 µs | [5.99, 6.22] µs |
| viewport_size/height/200 | 10.61 µs | 10.61 µs | 0.24 µs | [10.57, 10.66] µs |
| viewport_size/height/24 | 3.80 µs | 1.22 µs | 3.13 µs | [3.19, 4.41] µs |
| viewport_size/height/50 | 6.97 µs | 7.43 µs | 1.34 µs | [6.71, 7.23] µs |
| window_render/buffer_lines/10 | 473.31 ns | 447.23 ns | 42.14 ns | [465.33, 481.70] ns |
| window_render/buffer_lines/100 | 2.60 µs | 2.59 µs | 0.04 µs | [2.59, 2.61] µs |
| window_render/buffer_lines/1000 | 2.79 µs | 2.78 µs | 0.06 µs | [2.78, 2.80] µs |
| window_render/buffer_lines/10000 | 2.57 µs | 2.54 µs | 0.09 µs | [2.55, 2.58] µs |
| with_highlights/no_highlights | 2.63 µs | 2.65 µs | 0.08 µs | [2.61, 2.64] µs |
