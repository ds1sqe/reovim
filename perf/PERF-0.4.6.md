# Performance Benchmarks v0.4.7

> Last updated: 2025-12-17T16:11:17Z | Commit: 9c88f3e | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.4.7"
commit = "9c88f3e"
date = "2025-12-17T16:11:17Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.5-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 51.81 µs |
| buffer_vec | 4 | 79.06 µs |
| file_io | 2 | 239.04 µs |
| file_io_viewport | 3 | 42.13 µs |
| input_completion | 2 | 40.41 µs |
| input_mode_switch | 1 | 97.13 µs |
| input_scrolling | 3 | 5.38 ms |
| input_sustained | 1 | 3.82 ms |
| input_typing | 2 | 36.09 µs |
| io_bytes | 1 | 17.99 µs |
| rtt_explorer | 3 | 39.85 µs |
| rtt_input_lag | 2 | 162.04 µs |
| rtt_movement_lag | 5 | 226.35 ms |
| screen_io | 3 | 16.52 µs |
| screen_viewport_io | 3 | 28.22 µs |
| stress_completion | 1 | 1.31 ms |
| stress_editing | 3 | 137.11 ms |
| stress_mode_ops | 1 | 11.47 ms |
| stress_scroll | 3 | 67.56 ms |
| stress_worst_case | 1 | 201.17 ms |
| throughput | 1 | 5.71 µs |
| viewport_size | 4 | 14.65 µs |
| window_render | 4 | 5.31 µs |
| with_highlights | 1 | 7.68 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 16.92, median = 14.57, std_dev = 5.31, unit = "µs" }
clone_1000 = { mean = 175.04, median = 172.13, std_dev = 9.46, unit = "µs" }
clone_10000 = { mean = 2.26, median = 1.99, std_dev = 0.81, unit = "ms" }
clone_50000 = { mean = 13.02, median = 11.05, std_dev = 4.53, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 17.73, median = 15.77, std_dev = 4.02, unit = "µs" }
vec_clone_1000 = { mean = 282.66, median = 230.16, std_dev = 110.68, unit = "µs" }
vec_clone_10000 = { mean = 3.24, median = 3.52, std_dev = 1.31, unit = "ms" }
vec_clone_50000 = { mean = 12.60, median = 10.25, std_dev = 4.17, unit = "ms" }

[results.file_io]
buffered_file = { mean = 38.92, median = 40.10, std_dev = 3.37, unit = "µs" }
unbuffered_file = { mean = 439.15, median = 431.53, std_dev = 27.72, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 61.26, median = 60.81, std_dev = 2.27, unit = "µs" }
viewport_24 = { mean = 22.45, median = 19.14, std_dev = 5.85, unit = "µs" }
viewport_50 = { mean = 42.67, median = 43.64, std_dev = 3.57, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 43.69, median = 39.39, std_dev = 10.00, unit = "µs" }
without_completion_popup = { mean = 37.12, median = 31.51, std_dev = 10.29, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 97.13, median = 89.56, std_dev = 26.30, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 12.92, median = 13.07, std_dev = 0.46, unit = "ms" }
scroll_half_page = { mean = 1.65, median = 1.70, std_dev = 0.10, unit = "ms" }
scroll_one_line = { mean = 1.59, median = 1.55, std_dev = 0.11, unit = "ms" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 3.82, median = 3.81, std_dev = 0.43, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 40.03, median = 38.43, std_dev = 2.85, unit = "µs" }
single_char_render = { mean = 32.16, median = 32.09, std_dev = 1.05, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 17.99, median = 17.84, std_dev = 1.47, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 18.98, median = 18.91, std_dev = 0.34, unit = "µs" }
open_explorer = { mean = 14.94, median = 14.67, std_dev = 1.52, unit = "µs" }
toggle_cycle = { mean = 85.63, median = 70.24, std_dev = 40.80, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 147.24, median = 142.13, std_dev = 37.13, unit = "µs" }
char_insert_rtt = { mean = 176.83, median = 167.74, std_dev = 54.50, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 3.16, median = 3.22, std_dev = 0.68, unit = "ms" }
half_page_down_rtt = { mean = 2.30, median = 1.62, std_dev = 1.10, unit = "ms" }
move_down_rtt = { mean = 3.60, median = 3.56, std_dev = 1.53, unit = "ms" }
move_right_rtt = { mean = 684.40, median = 672.31, std_dev = 360.69, unit = "µs" }
word_forward_rtt = { mean = 438.30, median = 351.58, std_dev = 225.83, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 16.65, median = 15.72, std_dev = 1.94, unit = "µs" }
full_render_1000 = { mean = 18.29, median = 18.13, std_dev = 1.78, unit = "µs" }
full_render_10000 = { mean = 14.62, median = 14.49, std_dev = 1.23, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 47.18, median = 47.20, std_dev = 0.48, unit = "µs" }
viewport_height_24 = { mean = 10.78, median = 10.79, std_dev = 0.18, unit = "µs" }
viewport_height_50 = { mean = 26.71, median = 26.37, std_dev = 4.23, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 1.31, median = 1.25, std_dev = 0.16, unit = "ms" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 56.28, median = 42.45, std_dev = 23.31, unit = "ms" }
edit_navigate_cycle_1k = { mean = 9.02, median = 9.01, std_dev = 3.71, unit = "ms" }
edit_navigate_cycle_50k = { mean = 346.03, median = 333.89, std_dev = 89.31, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 11.47, median = 9.58, std_dev = 4.83, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 146.58, median = 120.50, std_dev = 52.15, unit = "ms" }
jump_around_file = { mean = 24.28, median = 18.11, std_dev = 10.33, unit = "ms" }
spam_ctrl_d_10_times = { mean = 31.81, median = 31.63, std_dev = 8.90, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 201.17, median = 208.07, std_dev = 75.23, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 5.71, median = 5.78, std_dev = 0.42, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 15.46, median = 15.44, std_dev = 0.13, unit = "µs" }
height_200 = { mean = 31.89, median = 31.92, std_dev = 1.14, unit = "µs" }
height_24 = { mean = 3.96, median = 3.94, std_dev = 0.18, unit = "µs" }
height_50 = { mean = 7.28, median = 7.23, std_dev = 0.61, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 1.16, median = 1.13, std_dev = 0.10, unit = "µs" }
buffer_lines_100 = { mean = 6.08, median = 6.17, std_dev = 0.89, unit = "µs" }
buffer_lines_1000 = { mean = 5.99, median = 5.40, std_dev = 1.46, unit = "µs" }
buffer_lines_10000 = { mean = 8.00, median = 7.97, std_dev = 0.12, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 7.68, median = 7.75, std_dev = 0.92, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 16.92 µs | 14.57 µs | 5.31 µs | [15.16, 18.90] µs |
| buffer_clone/clone/1000 | 175.04 µs | 172.13 µs | 9.46 µs | [171.84, 178.50] µs |
| buffer_clone/clone/10000 | 2.26 ms | 1.99 ms | 0.81 ms | [2.01, 2.57] ms |
| buffer_clone/clone/50000 | 13.02 ms | 11.05 ms | 4.53 ms | [11.55, 14.72] ms |
| buffer_vec/vec_clone/100 | 17.73 µs | 15.77 µs | 4.02 µs | [16.44, 19.26] µs |
| buffer_vec/vec_clone/1000 | 282.66 µs | 230.16 µs | 110.68 µs | [246.91, 324.27] µs |
| buffer_vec/vec_clone/10000 | 3.24 ms | 3.52 ms | 1.31 ms | [2.79, 3.71] ms |
| buffer_vec/vec_clone/50000 | 12.60 ms | 10.25 ms | 4.17 ms | [11.23, 14.16] ms |
| file_io/buffered_file | 38.92 µs | 40.10 µs | 3.37 µs | [37.70, 40.05] µs |
| file_io/unbuffered_file | 439.15 µs | 431.53 µs | 27.72 µs | [430.24, 449.70] µs |
| file_io_viewport/viewport/100 | 61.26 µs | 60.81 µs | 2.27 µs | [60.50, 62.09] µs |
| file_io_viewport/viewport/24 | 22.45 µs | 19.14 µs | 5.85 µs | [20.43, 24.53] µs |
| file_io_viewport/viewport/50 | 42.67 µs | 43.64 µs | 3.57 µs | [41.33, 43.83] µs |
| input_completion/with_completion_popup | 43.69 µs | 39.39 µs | 10.00 µs | [40.51, 47.50] µs |
| input_completion/without_completion_popup | 37.12 µs | 31.51 µs | 10.29 µs | [33.57, 40.77] µs |
| input_mode_switch/normal_insert_normal | 97.13 µs | 89.56 µs | 26.30 µs | [89.96, 107.91] µs |
| input_scrolling/scroll_10_lines | 12.92 ms | 13.07 ms | 0.46 ms | [12.75, 13.08] ms |
| input_scrolling/scroll_half_page | 1.65 ms | 1.70 ms | 0.10 ms | [1.61, 1.68] ms |
| input_scrolling/scroll_one_line | 1.59 ms | 1.55 ms | 0.11 ms | [1.55, 1.63] ms |
| input_sustained/100_keystrokes_each_rendered | 3.82 ms | 3.81 ms | 0.43 ms | [3.70, 3.94] ms |
| input_typing/burst_10_chars_render | 40.03 µs | 38.43 µs | 2.85 µs | [39.07, 41.07] µs |
| input_typing/single_char_render | 32.16 µs | 32.09 µs | 1.05 µs | [31.80, 32.54] µs |
| io_bytes/full_screen_render | 17.99 µs | 17.84 µs | 1.47 µs | [17.48, 18.51] µs |
| rtt_explorer/close_explorer | 18.98 µs | 18.91 µs | 0.34 µs | [18.86, 19.10] µs |
| rtt_explorer/open_explorer | 14.94 µs | 14.67 µs | 1.52 µs | [14.43, 15.50] µs |
| rtt_explorer/toggle_cycle | 85.63 µs | 70.24 µs | 40.80 µs | [71.91, 100.67] µs |
| rtt_input_lag/backspace_rtt | 147.24 µs | 142.13 µs | 37.13 µs | [136.18, 161.99] µs |
| rtt_input_lag/char_insert_rtt | 176.83 µs | 167.74 µs | 54.50 µs | [159.34, 197.55] µs |
| rtt_movement_lag/goto_top_rtt | 3.16 ms | 3.22 ms | 0.68 ms | [2.91, 3.38] ms |
| rtt_movement_lag/half_page_down_rtt | 2.30 ms | 1.62 ms | 1.10 ms | [1.94, 2.71] ms |
| rtt_movement_lag/move_down_rtt | 3.60 ms | 3.56 ms | 1.53 ms | [3.07, 4.15] ms |
| rtt_movement_lag/move_right_rtt | 684.40 µs | 672.31 µs | 360.69 µs | [561.39, 815.75] µs |
| rtt_movement_lag/word_forward_rtt | 438.30 µs | 351.58 µs | 225.83 µs | [365.14, 522.74] µs |
| screen_io/full_render/100 | 16.65 µs | 15.72 µs | 1.94 µs | [15.98, 17.34] µs |
| screen_io/full_render/1000 | 18.29 µs | 18.13 µs | 1.78 µs | [17.68, 18.93] µs |
| screen_io/full_render/10000 | 14.62 µs | 14.49 µs | 1.23 µs | [14.21, 15.08] µs |
| screen_viewport_io/viewport_height/100 | 47.18 µs | 47.20 µs | 0.48 µs | [47.01, 47.35] µs |
| screen_viewport_io/viewport_height/24 | 10.78 µs | 10.79 µs | 0.18 µs | [10.71, 10.84] µs |
| screen_viewport_io/viewport_height/50 | 26.71 µs | 26.37 µs | 4.23 µs | [25.22, 28.20] µs |
| stress_completion/completion_scroll_100_items | 1.31 ms | 1.25 ms | 0.16 ms | [1.26, 1.37] ms |
| stress_editing/edit_navigate_cycle_10k | 56.28 ms | 42.45 ms | 23.31 ms | [48.33, 64.69] ms |
| stress_editing/edit_navigate_cycle_1k | 9.02 ms | 9.01 ms | 3.71 ms | [7.74, 10.33] ms |
| stress_editing/edit_navigate_cycle_50k | 346.03 ms | 333.89 ms | 89.31 ms | [315.51, 378.28] ms |
| stress_mode_ops/insert_escape_move_cycle | 11.47 ms | 9.58 ms | 4.83 ms | [9.96, 13.35] ms |
| stress_scroll/hold_j_50_lines_10k_file | 146.58 ms | 120.50 ms | 52.15 ms | [129.05, 165.73] ms |
| stress_scroll/jump_around_file | 24.28 ms | 18.11 ms | 10.33 ms | [20.78, 28.00] ms |
| stress_scroll/spam_ctrl_d_10_times | 31.81 ms | 31.63 ms | 8.90 ms | [28.72, 34.96] ms |
| stress_worst_case/all_features_50k_file | 201.17 ms | 208.07 ms | 75.23 ms | [169.36, 233.62] ms |
| throughput/renders_per_second | 5.71 µs | 5.78 µs | 0.42 µs | [5.56, 5.85] µs |
| viewport_size/height/100 | 15.46 µs | 15.44 µs | 0.13 µs | [15.42, 15.51] µs |
| viewport_size/height/200 | 31.89 µs | 31.92 µs | 1.14 µs | [31.49, 32.29] µs |
| viewport_size/height/24 | 3.96 µs | 3.94 µs | 0.18 µs | [3.90, 4.03] µs |
| viewport_size/height/50 | 7.28 µs | 7.23 µs | 0.61 µs | [7.06, 7.49] µs |
| window_render/buffer_lines/10 | 1.16 µs | 1.13 µs | 0.10 µs | [1.13, 1.20] µs |
| window_render/buffer_lines/100 | 6.08 µs | 6.17 µs | 0.89 µs | [5.79, 6.41] µs |
| window_render/buffer_lines/1000 | 5.99 µs | 5.40 µs | 1.46 µs | [5.51, 6.54] µs |
| window_render/buffer_lines/10000 | 8.00 µs | 7.97 µs | 0.12 µs | [7.96, 8.05] µs |
| with_highlights/no_highlights | 7.68 µs | 7.75 µs | 0.92 µs | [7.36, 8.00] µs |
