# Performance Benchmarks v0.5.0

> Last updated: 2025-12-17T16:34:16Z | Commit: f7d629b | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.5.0"
commit = "f7d629b"
date = "2025-12-17T16:34:16Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.5-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 122.44 µs |
| buffer_vec | 4 | 118.17 µs |
| file_io | 2 | 90.10 µs |
| file_io_viewport | 3 | 18.25 µs |
| input_completion | 2 | 8.36 µs |
| input_mode_switch | 1 | 17.78 µs |
| input_scrolling | 3 | 347.47 ms |
| input_sustained | 1 | 1.27 ms |
| input_typing | 2 | 14.46 µs |
| io_bytes | 1 | 11.02 µs |
| rtt_explorer | 3 | 8.44 µs |
| rtt_input_lag | 2 | 29.04 µs |
| rtt_movement_lag | 5 | 249.68 µs |
| screen_io | 3 | 10.91 µs |
| screen_viewport_io | 3 | 11.51 µs |
| stress_completion | 1 | 318.60 µs |
| stress_editing | 3 | 320.09 ms |
| stress_mode_ops | 1 | 1.61 ms |
| stress_scroll | 3 | 10.65 ms |
| stress_worst_case | 1 | 26.13 ms |
| throughput | 1 | 4.43 µs |
| viewport_size | 4 | 10.28 µs |
| window_render | 4 | 4.52 µs |
| with_highlights | 1 | 4.91 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 4.17, median = 4.17, std_dev = 0.29, unit = "µs" }
clone_1000 = { mean = 45.62, median = 45.55, std_dev = 2.99, unit = "µs" }
clone_10000 = { mean = 437.57, median = 435.93, std_dev = 29.63, unit = "µs" }
clone_50000 = { mean = 2.41, median = 2.40, std_dev = 0.17, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 3.85, median = 3.84, std_dev = 0.24, unit = "µs" }
vec_clone_1000 = { mean = 42.15, median = 43.58, std_dev = 6.86, unit = "µs" }
vec_clone_10000 = { mean = 424.25, median = 416.16, std_dev = 22.70, unit = "µs" }
vec_clone_50000 = { mean = 2.42, median = 2.43, std_dev = 0.24, unit = "ms" }

[results.file_io]
buffered_file = { mean = 16.28, median = 16.10, std_dev = 1.77, unit = "µs" }
unbuffered_file = { mean = 163.91, median = 159.38, std_dev = 21.85, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 25.67, median = 26.27, std_dev = 2.47, unit = "µs" }
viewport_24 = { mean = 12.71, median = 12.23, std_dev = 1.27, unit = "µs" }
viewport_50 = { mean = 16.37, median = 16.38, std_dev = 1.69, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 10.78, median = 10.61, std_dev = 1.21, unit = "µs" }
without_completion_popup = { mean = 5.94, median = 5.76, std_dev = 0.68, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 17.78, median = 17.76, std_dev = 1.58, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 4.70, median = 4.57, std_dev = 0.54, unit = "ms" }
scroll_half_page = { mean = 505.41, median = 515.51, std_dev = 58.58, unit = "µs" }
scroll_one_line = { mean = 532.29, median = 534.26, std_dev = 59.37, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 1.27, median = 1.23, std_dev = 0.13, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 14.66, median = 14.63, std_dev = 1.67, unit = "µs" }
single_char_render = { mean = 14.26, median = 14.76, std_dev = 1.50, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 11.02, median = 11.14, std_dev = 0.95, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 6.40, median = 6.35, std_dev = 0.55, unit = "µs" }
open_explorer = { mean = 6.21, median = 6.26, std_dev = 0.56, unit = "µs" }
toggle_cycle = { mean = 12.73, median = 12.66, std_dev = 1.25, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 29.36, median = 29.14, std_dev = 2.73, unit = "µs" }
char_insert_rtt = { mean = 28.72, median = 28.91, std_dev = 2.48, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 384.80, median = 378.91, std_dev = 43.66, unit = "µs" }
half_page_down_rtt = { mean = 391.24, median = 387.18, std_dev = 42.46, unit = "µs" }
move_down_rtt = { mean = 379.48, median = 370.73, std_dev = 42.74, unit = "µs" }
move_right_rtt = { mean = 46.01, median = 46.13, std_dev = 4.51, unit = "µs" }
word_forward_rtt = { mean = 46.88, median = 47.17, std_dev = 5.21, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 11.19, median = 11.34, std_dev = 0.76, unit = "µs" }
full_render_1000 = { mean = 10.80, median = 10.85, std_dev = 1.47, unit = "µs" }
full_render_10000 = { mean = 10.73, median = 10.71, std_dev = 0.63, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 17.72, median = 17.87, std_dev = 1.67, unit = "µs" }
viewport_height_24 = { mean = 6.30, median = 6.28, std_dev = 0.38, unit = "µs" }
viewport_height_50 = { mean = 10.52, median = 10.94, std_dev = 1.42, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 318.60, median = 314.59, std_dev = 38.61, unit = "µs" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 7.84, median = 7.70, std_dev = 0.73, unit = "ms" }
edit_navigate_cycle_1k = { mean = 917.87, median = 912.26, std_dev = 76.80, unit = "µs" }
edit_navigate_cycle_50k = { mean = 34.56, median = 33.69, std_dev = 3.05, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 1.61, median = 1.60, std_dev = 0.11, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 23.57, median = 23.32, std_dev = 2.00, unit = "ms" }
jump_around_file = { mean = 3.36, median = 3.23, std_dev = 0.39, unit = "ms" }
spam_ctrl_d_10_times = { mean = 5.03, median = 4.88, std_dev = 0.45, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 26.13, median = 24.57, std_dev = 2.85, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 4.43, median = 4.54, std_dev = 0.32, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 11.89, median = 10.84, std_dev = 2.65, unit = "µs" }
height_200 = { mean = 20.44, median = 19.72, std_dev = 2.50, unit = "µs" }
height_24 = { mean = 2.64, median = 2.70, std_dev = 0.21, unit = "µs" }
height_50 = { mean = 6.14, median = 6.28, std_dev = 0.65, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 1.01, median = 0.94, std_dev = 0.13, unit = "µs" }
buffer_lines_100 = { mean = 5.74, median = 5.72, std_dev = 0.71, unit = "µs" }
buffer_lines_1000 = { mean = 5.74, median = 5.87, std_dev = 0.29, unit = "µs" }
buffer_lines_10000 = { mean = 5.61, median = 5.48, std_dev = 0.67, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 4.91, median = 4.92, std_dev = 0.37, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 4.17 µs | 4.17 µs | 0.29 µs | [4.07, 4.27] µs |
| buffer_clone/clone/1000 | 45.62 µs | 45.55 µs | 2.99 µs | [44.55, 46.67] µs |
| buffer_clone/clone/10000 | 437.57 µs | 435.93 µs | 29.63 µs | [427.20, 448.05] µs |
| buffer_clone/clone/50000 | 2.41 ms | 2.40 ms | 0.17 ms | [2.35, 2.47] ms |
| buffer_vec/vec_clone/100 | 3.85 µs | 3.84 µs | 0.24 µs | [3.77, 3.93] µs |
| buffer_vec/vec_clone/1000 | 42.15 µs | 43.58 µs | 6.86 µs | [39.76, 44.58] µs |
| buffer_vec/vec_clone/10000 | 424.25 µs | 416.16 µs | 22.70 µs | [417.50, 433.25] µs |
| buffer_vec/vec_clone/50000 | 2.42 ms | 2.43 ms | 0.24 ms | [2.33, 2.50] ms |
| file_io/buffered_file | 16.28 µs | 16.10 µs | 1.77 µs | [15.69, 16.93] µs |
| file_io/unbuffered_file | 163.91 µs | 159.38 µs | 21.85 µs | [156.71, 171.93] µs |
| file_io_viewport/viewport/100 | 25.67 µs | 26.27 µs | 2.47 µs | [24.81, 26.54] µs |
| file_io_viewport/viewport/24 | 12.71 µs | 12.23 µs | 1.27 µs | [12.27, 13.16] µs |
| file_io_viewport/viewport/50 | 16.37 µs | 16.38 µs | 1.69 µs | [15.77, 16.96] µs |
| input_completion/with_completion_popup | 10.78 µs | 10.61 µs | 1.21 µs | [10.36, 11.20] µs |
| input_completion/without_completion_popup | 5.94 µs | 5.76 µs | 0.68 µs | [5.70, 6.18] µs |
| input_mode_switch/normal_insert_normal | 17.78 µs | 17.76 µs | 1.58 µs | [17.23, 18.34] µs |
| input_scrolling/scroll_10_lines | 4.70 ms | 4.57 ms | 0.54 ms | [4.51, 4.89] ms |
| input_scrolling/scroll_half_page | 505.41 µs | 515.51 µs | 58.58 µs | [484.41, 525.59] µs |
| input_scrolling/scroll_one_line | 532.29 µs | 534.26 µs | 59.37 µs | [511.25, 552.81] µs |
| input_sustained/100_keystrokes_each_rendered | 1.27 ms | 1.23 ms | 0.13 ms | [1.24, 1.31] ms |
| input_typing/burst_10_chars_render | 14.66 µs | 14.63 µs | 1.67 µs | [14.08, 15.26] µs |
| input_typing/single_char_render | 14.26 µs | 14.76 µs | 1.50 µs | [13.72, 14.78] µs |
| io_bytes/full_screen_render | 11.02 µs | 11.14 µs | 0.95 µs | [10.68, 11.35] µs |
| rtt_explorer/close_explorer | 6.40 µs | 6.35 µs | 0.55 µs | [6.20, 6.59] µs |
| rtt_explorer/open_explorer | 6.21 µs | 6.26 µs | 0.56 µs | [6.01, 6.41] µs |
| rtt_explorer/toggle_cycle | 12.73 µs | 12.66 µs | 1.25 µs | [12.29, 13.17] µs |
| rtt_input_lag/backspace_rtt | 29.36 µs | 29.14 µs | 2.73 µs | [28.41, 30.33] µs |
| rtt_input_lag/char_insert_rtt | 28.72 µs | 28.91 µs | 2.48 µs | [27.85, 29.59] µs |
| rtt_movement_lag/goto_top_rtt | 384.80 µs | 378.91 µs | 43.66 µs | [369.69, 400.30] µs |
| rtt_movement_lag/half_page_down_rtt | 391.24 µs | 387.18 µs | 42.46 µs | [376.50, 406.24] µs |
| rtt_movement_lag/move_down_rtt | 379.48 µs | 370.73 µs | 42.74 µs | [364.71, 394.74] µs |
| rtt_movement_lag/move_right_rtt | 46.01 µs | 46.13 µs | 4.51 µs | [44.42, 47.60] µs |
| rtt_movement_lag/word_forward_rtt | 46.88 µs | 47.17 µs | 5.21 µs | [45.04, 48.69] µs |
| screen_io/full_render/100 | 11.19 µs | 11.34 µs | 0.76 µs | [10.92, 11.45] µs |
| screen_io/full_render/1000 | 10.80 µs | 10.85 µs | 1.47 µs | [10.29, 11.32] µs |
| screen_io/full_render/10000 | 10.73 µs | 10.71 µs | 0.63 µs | [10.51, 10.95] µs |
| screen_viewport_io/viewport_height/100 | 17.72 µs | 17.87 µs | 1.67 µs | [17.14, 18.31] µs |
| screen_viewport_io/viewport_height/24 | 6.30 µs | 6.28 µs | 0.38 µs | [6.17, 6.43] µs |
| screen_viewport_io/viewport_height/50 | 10.52 µs | 10.94 µs | 1.42 µs | [10.01, 11.01] µs |
| stress_completion/completion_scroll_100_items | 318.60 µs | 314.59 µs | 38.61 µs | [305.17, 332.48] µs |
| stress_editing/edit_navigate_cycle_10k | 7.84 ms | 7.70 ms | 0.73 ms | [7.58, 8.10] ms |
| stress_editing/edit_navigate_cycle_1k | 917.87 µs | 912.26 µs | 76.80 µs | [891.05, 944.70] µs |
| stress_editing/edit_navigate_cycle_50k | 34.56 ms | 33.69 ms | 3.05 ms | [33.53, 35.65] ms |
| stress_mode_ops/insert_escape_move_cycle | 1.61 ms | 1.60 ms | 0.11 ms | [1.58, 1.65] ms |
| stress_scroll/hold_j_50_lines_10k_file | 23.57 ms | 23.32 ms | 2.00 ms | [22.88, 24.28] ms |
| stress_scroll/jump_around_file | 3.36 ms | 3.23 ms | 0.39 ms | [3.22, 3.50] ms |
| stress_scroll/spam_ctrl_d_10_times | 5.03 ms | 4.88 ms | 0.45 ms | [4.89, 5.21] ms |
| stress_worst_case/all_features_50k_file | 26.13 ms | 24.57 ms | 2.85 ms | [25.01, 27.43] ms |
| throughput/renders_per_second | 4.43 µs | 4.54 µs | 0.32 µs | [4.31, 4.54] µs |
| viewport_size/height/100 | 11.89 µs | 10.84 µs | 2.65 µs | [10.98, 12.83] µs |
| viewport_size/height/200 | 20.44 µs | 19.72 µs | 2.50 µs | [19.62, 21.36] µs |
| viewport_size/height/24 | 2.64 µs | 2.70 µs | 0.21 µs | [2.56, 2.71] µs |
| viewport_size/height/50 | 6.14 µs | 6.28 µs | 0.65 µs | [5.90, 6.36] µs |
| window_render/buffer_lines/10 | 1.01 µs | 0.94 µs | 0.13 µs | [0.96, 1.05] µs |
| window_render/buffer_lines/100 | 5.74 µs | 5.72 µs | 0.71 µs | [5.49, 5.99] µs |
| window_render/buffer_lines/1000 | 5.74 µs | 5.87 µs | 0.29 µs | [5.63, 5.84] µs |
| window_render/buffer_lines/10000 | 5.61 µs | 5.48 µs | 0.67 µs | [5.38, 5.85] µs |
| with_highlights/no_highlights | 4.91 µs | 4.92 µs | 0.37 µs | [4.78, 5.04] µs |
