# Performance Benchmarks v0.4.2

> Last updated: 2025-12-17T03:47:51Z | Commit: 2a3c9cd | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.4.2"
commit = "2a3c9cd"
date = "2025-12-17T03:47:51Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.5-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 101.49 µs |
| buffer_vec | 4 | 100.58 µs |
| file_io | 2 | 69.79 µs |
| file_io_viewport | 3 | 15.23 µs |
| input_completion | 2 | 9.38 µs |
| input_mode_switch | 1 | 22.92 µs |
| input_scrolling | 3 | 264.50 ms |
| input_sustained | 1 | 1.57 ms |
| input_typing | 2 | 9.67 µs |
| io_bytes | 1 | 6.14 µs |
| rtt_explorer | 3 | 8.72 µs |
| rtt_input_lag | 2 | 31.91 µs |
| rtt_movement_lag | 5 | 265.68 µs |
| screen_io | 3 | 5.91 µs |
| screen_viewport_io | 3 | 6.81 µs |
| stress_completion | 1 | 225.12 µs |
| stress_editing | 3 | 320.72 ms |
| stress_mode_ops | 1 | 1.42 ms |
| stress_scroll | 3 | 8.55 ms |
| stress_worst_case | 1 | 20.10 ms |
| throughput | 1 | 2.49 µs |
| viewport_size | 4 | 4.60 µs |
| window_render | 4 | 161.83 ns |
| with_highlights | 1 | 2.48 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 3.00, median = 2.98, std_dev = 0.08, unit = "µs" }
clone_1000 = { mean = 36.39, median = 35.83, std_dev = 2.29, unit = "µs" }
clone_10000 = { mean = 364.60, median = 365.23, std_dev = 9.28, unit = "µs" }
clone_50000 = { mean = 1.96, median = 1.96, std_dev = 0.05, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 3.58, median = 3.46, std_dev = 0.51, unit = "µs" }
vec_clone_1000 = { mean = 35.12, median = 35.11, std_dev = 0.49, unit = "µs" }
vec_clone_10000 = { mean = 361.68, median = 357.98, std_dev = 13.77, unit = "µs" }
vec_clone_50000 = { mean = 1.94, median = 1.94, std_dev = 0.05, unit = "ms" }

[results.file_io]
buffered_file = { mean = 12.68, median = 12.42, std_dev = 1.31, unit = "µs" }
unbuffered_file = { mean = 126.89, median = 126.40, std_dev = 3.35, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 19.45, median = 19.24, std_dev = 0.78, unit = "µs" }
viewport_24 = { mean = 9.04, median = 8.97, std_dev = 0.67, unit = "µs" }
viewport_50 = { mean = 17.21, median = 17.92, std_dev = 2.96, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 11.69, median = 11.80, std_dev = 0.47, unit = "µs" }
without_completion_popup = { mean = 7.07, median = 6.83, std_dev = 0.85, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 22.92, median = 22.79, std_dev = 1.75, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 3.62, median = 3.61, std_dev = 0.08, unit = "ms" }
scroll_half_page = { mean = 413.42, median = 384.12, std_dev = 62.17, unit = "µs" }
scroll_one_line = { mean = 376.45, median = 378.08, std_dev = 7.74, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 1.57, median = 1.59, std_dev = 0.11, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 9.92, median = 9.89, std_dev = 0.22, unit = "µs" }
single_char_render = { mean = 9.41, median = 9.49, std_dev = 0.17, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 6.14, median = 6.01, std_dev = 0.61, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 6.24, median = 6.25, std_dev = 0.22, unit = "µs" }
open_explorer = { mean = 6.72, median = 6.65, std_dev = 0.42, unit = "µs" }
toggle_cycle = { mean = 13.21, median = 12.81, std_dev = 1.38, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 29.82, median = 29.53, std_dev = 0.81, unit = "µs" }
char_insert_rtt = { mean = 34.00, median = 33.66, std_dev = 3.64, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 396.60, median = 394.44, std_dev = 11.76, unit = "µs" }
half_page_down_rtt = { mean = 410.37, median = 404.96, std_dev = 18.19, unit = "µs" }
move_down_rtt = { mean = 397.35, median = 395.27, std_dev = 6.25, unit = "µs" }
move_right_rtt = { mean = 54.93, median = 49.41, std_dev = 10.02, unit = "µs" }
word_forward_rtt = { mean = 69.16, median = 71.75, std_dev = 14.33, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 5.93, median = 5.91, std_dev = 0.17, unit = "µs" }
full_render_1000 = { mean = 5.89, median = 5.87, std_dev = 0.13, unit = "µs" }
full_render_10000 = { mean = 5.91, median = 5.89, std_dev = 0.07, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 11.01, median = 11.02, std_dev = 0.29, unit = "µs" }
viewport_height_24 = { mean = 3.51, median = 3.51, std_dev = 0.06, unit = "µs" }
viewport_height_50 = { mean = 5.90, median = 5.88, std_dev = 0.09, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 225.12, median = 224.61, std_dev = 3.94, unit = "µs" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 8.41, median = 8.10, std_dev = 0.98, unit = "ms" }
edit_navigate_cycle_1k = { mean = 914.76, median = 920.47, std_dev = 19.15, unit = "µs" }
edit_navigate_cycle_50k = { mean = 38.99, median = 38.54, std_dev = 1.57, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 1.42, median = 1.41, std_dev = 0.01, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 18.16, median = 17.96, std_dev = 0.46, unit = "ms" }
jump_around_file = { mean = 3.14, median = 3.11, std_dev = 0.17, unit = "ms" }
spam_ctrl_d_10_times = { mean = 4.36, median = 4.04, std_dev = 0.49, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 20.10, median = 19.86, std_dev = 0.96, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 2.49, median = 2.50, std_dev = 0.06, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 4.96, median = 4.90, std_dev = 0.11, unit = "µs" }
height_200 = { mean = 9.73, median = 9.78, std_dev = 0.11, unit = "µs" }
height_24 = { mean = 1.20, median = 1.20, std_dev = 0.05, unit = "µs" }
height_50 = { mean = 2.50, median = 2.50, std_dev = 0.06, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 638.94, median = 636.54, std_dev = 36.20, unit = "ns" }
buffer_lines_100 = { mean = 3.12, median = 3.15, std_dev = 0.15, unit = "µs" }
buffer_lines_1000 = { mean = 2.77, median = 2.73, std_dev = 0.14, unit = "µs" }
buffer_lines_10000 = { mean = 2.48, median = 2.48, std_dev = 0.07, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 2.48, median = 2.46, std_dev = 0.18, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 3.00 µs | 2.98 µs | 0.08 µs | [2.99, 3.02] µs |
| buffer_clone/clone/1000 | 36.39 µs | 35.83 µs | 2.29 µs | [35.98, 36.87] µs |
| buffer_clone/clone/10000 | 364.60 µs | 365.23 µs | 9.28 µs | [362.86, 366.47] µs |
| buffer_clone/clone/50000 | 1.96 ms | 1.96 ms | 0.05 ms | [1.95, 1.97] ms |
| buffer_vec/vec_clone/100 | 3.58 µs | 3.46 µs | 0.51 µs | [3.49, 3.68] µs |
| buffer_vec/vec_clone/1000 | 35.12 µs | 35.11 µs | 0.49 µs | [35.03, 35.22] µs |
| buffer_vec/vec_clone/10000 | 361.68 µs | 357.98 µs | 13.77 µs | [359.04, 364.41] µs |
| buffer_vec/vec_clone/50000 | 1.94 ms | 1.94 ms | 0.05 ms | [1.93, 1.95] ms |
| file_io/buffered_file | 12.68 µs | 12.42 µs | 1.31 µs | [12.45, 12.96] µs |
| file_io/unbuffered_file | 126.89 µs | 126.40 µs | 3.35 µs | [126.25, 127.55] µs |
| file_io_viewport/viewport/100 | 19.45 µs | 19.24 µs | 0.78 µs | [19.30, 19.61] µs |
| file_io_viewport/viewport/24 | 9.04 µs | 8.97 µs | 0.67 µs | [8.91, 9.18] µs |
| file_io_viewport/viewport/50 | 17.21 µs | 17.92 µs | 2.96 µs | [16.63, 17.79] µs |
| input_completion/with_completion_popup | 11.69 µs | 11.80 µs | 0.47 µs | [11.60, 11.78] µs |
| input_completion/without_completion_popup | 7.07 µs | 6.83 µs | 0.85 µs | [6.91, 7.24] µs |
| input_mode_switch/normal_insert_normal | 22.92 µs | 22.79 µs | 1.75 µs | [22.58, 23.26] µs |
| input_scrolling/scroll_10_lines | 3.62 ms | 3.61 ms | 0.08 ms | [3.60, 3.63] ms |
| input_scrolling/scroll_half_page | 413.42 µs | 384.12 µs | 62.17 µs | [401.84, 425.99] µs |
| input_scrolling/scroll_one_line | 376.45 µs | 378.08 µs | 7.74 µs | [374.92, 377.94] µs |
| input_sustained/100_keystrokes_each_rendered | 1.57 ms | 1.59 ms | 0.11 ms | [1.54, 1.60] ms |
| input_typing/burst_10_chars_render | 9.92 µs | 9.89 µs | 0.22 µs | [9.88, 9.97] µs |
| input_typing/single_char_render | 9.41 µs | 9.49 µs | 0.17 µs | [9.38, 9.44] µs |
| io_bytes/full_screen_render | 6.14 µs | 6.01 µs | 0.61 µs | [6.05, 6.28] µs |
| rtt_explorer/close_explorer | 6.24 µs | 6.25 µs | 0.22 µs | [6.20, 6.29] µs |
| rtt_explorer/open_explorer | 6.72 µs | 6.65 µs | 0.42 µs | [6.64, 6.80] µs |
| rtt_explorer/toggle_cycle | 13.21 µs | 12.81 µs | 1.38 µs | [12.98, 13.51] µs |
| rtt_input_lag/backspace_rtt | 29.82 µs | 29.53 µs | 0.81 µs | [29.67, 29.99] µs |
| rtt_input_lag/char_insert_rtt | 34.00 µs | 33.66 µs | 3.64 µs | [33.29, 34.71] µs |
| rtt_movement_lag/goto_top_rtt | 396.60 µs | 394.44 µs | 11.76 µs | [394.59, 399.13] µs |
| rtt_movement_lag/half_page_down_rtt | 410.37 µs | 404.96 µs | 18.19 µs | [406.99, 414.09] µs |
| rtt_movement_lag/move_down_rtt | 397.35 µs | 395.27 µs | 6.25 µs | [396.19, 398.62] µs |
| rtt_movement_lag/move_right_rtt | 54.93 µs | 49.41 µs | 10.02 µs | [53.03, 56.92] µs |
| rtt_movement_lag/word_forward_rtt | 69.16 µs | 71.75 µs | 14.33 µs | [66.36, 71.94] µs |
| screen_io/full_render/100 | 5.93 µs | 5.91 µs | 0.17 µs | [5.90, 5.97] µs |
| screen_io/full_render/1000 | 5.89 µs | 5.87 µs | 0.13 µs | [5.87, 5.92] µs |
| screen_io/full_render/10000 | 5.91 µs | 5.89 µs | 0.07 µs | [5.90, 5.92] µs |
| screen_viewport_io/viewport_height/100 | 11.01 µs | 11.02 µs | 0.29 µs | [10.95, 11.06] µs |
| screen_viewport_io/viewport_height/24 | 3.51 µs | 3.51 µs | 0.06 µs | [3.50, 3.52] µs |
| screen_viewport_io/viewport_height/50 | 5.90 µs | 5.88 µs | 0.09 µs | [5.88, 5.92] µs |
| stress_completion/completion_scroll_100_items | 225.12 µs | 224.61 µs | 3.94 µs | [223.76, 226.53] µs |
| stress_editing/edit_navigate_cycle_10k | 8.41 ms | 8.10 ms | 0.98 ms | [8.07, 8.76] ms |
| stress_editing/edit_navigate_cycle_1k | 914.76 µs | 920.47 µs | 19.15 µs | [907.79, 921.22] µs |
| stress_editing/edit_navigate_cycle_50k | 38.99 ms | 38.54 ms | 1.57 ms | [38.49, 39.58] ms |
| stress_mode_ops/insert_escape_move_cycle | 1.42 ms | 1.41 ms | 0.01 ms | [1.41, 1.42] ms |
| stress_scroll/hold_j_50_lines_10k_file | 18.16 ms | 17.96 ms | 0.46 ms | [18.00, 18.33] ms |
| stress_scroll/jump_around_file | 3.14 ms | 3.11 ms | 0.17 ms | [3.09, 3.20] ms |
| stress_scroll/spam_ctrl_d_10_times | 4.36 ms | 4.04 ms | 0.49 ms | [4.19, 4.54] ms |
| stress_worst_case/all_features_50k_file | 20.10 ms | 19.86 ms | 0.96 ms | [19.71, 20.54] ms |
| throughput/renders_per_second | 2.49 µs | 2.50 µs | 0.06 µs | [2.48, 2.50] µs |
| viewport_size/height/100 | 4.96 µs | 4.90 µs | 0.11 µs | [4.94, 4.99] µs |
| viewport_size/height/200 | 9.73 µs | 9.78 µs | 0.11 µs | [9.71, 9.75] µs |
| viewport_size/height/24 | 1.20 µs | 1.20 µs | 0.05 µs | [1.19, 1.21] µs |
| viewport_size/height/50 | 2.50 µs | 2.50 µs | 0.06 µs | [2.49, 2.52] µs |
| window_render/buffer_lines/10 | 638.94 ns | 636.54 ns | 36.20 ns | [632.36, 646.55] ns |
| window_render/buffer_lines/100 | 3.12 µs | 3.15 µs | 0.15 µs | [3.09, 3.15] µs |
| window_render/buffer_lines/1000 | 2.77 µs | 2.73 µs | 0.14 µs | [2.74, 2.79] µs |
| window_render/buffer_lines/10000 | 2.48 µs | 2.48 µs | 0.07 µs | [2.47, 2.50] µs |
| with_highlights/no_highlights | 2.48 µs | 2.46 µs | 0.18 µs | [2.45, 2.52] µs |
