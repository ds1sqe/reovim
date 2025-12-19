# Performance Benchmarks v0.6.2

> Last updated: 2025-12-19T04:43:58Z | Commit: 53aae8d | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.6.2"
commit = "53aae8d"
date = "2025-12-19T04:43:58Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 216.89 µs |
| buffer_vec | 4 | 203.29 µs |
| file_io | 2 | 161.05 µs |
| file_io_viewport | 3 | 87.07 µs |
| input_completion | 2 | 71.79 µs |
| input_mode_switch | 1 | 205.12 µs |
| input_scrolling | 3 | 540.91 ms |
| input_sustained | 1 | 8.12 ms |
| input_typing | 2 | 81.96 µs |
| io_bytes | 1 | 67.78 µs |
| rtt_explorer | 3 | 86.47 µs |
| rtt_input_lag | 2 | 109.69 µs |
| rtt_movement_lag | 5 | 592.22 µs |
| screen_io | 3 | 65.61 µs |
| screen_viewport_io | 3 | 79.15 µs |
| stress_completion | 1 | 1.76 ms |
| stress_editing | 3 | 33.75 ms |
| stress_mode_ops | 1 | 4.48 ms |
| stress_scroll | 3 | 18.57 ms |
| stress_worst_case | 1 | 42.55 ms |
| throughput | 1 | 59.90 µs |
| viewport_size | 4 | 109.26 µs |
| window_render | 4 | 47.59 µs |
| with_highlights | 1 | 55.48 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 7.18, median = 6.05, std_dev = 1.75, unit = "µs" }
clone_1000 = { mean = 84.93, median = 72.11, std_dev = 17.42, unit = "µs" }
clone_10000 = { mean = 771.36, median = 722.79, std_dev = 89.69, unit = "µs" }
clone_50000 = { mean = 4.07, median = 3.77, std_dev = 0.62, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 6.68, median = 5.94, std_dev = 1.26, unit = "µs" }
vec_clone_1000 = { mean = 73.23, median = 72.69, std_dev = 1.92, unit = "µs" }
vec_clone_10000 = { mean = 729.45, median = 725.56, std_dev = 9.80, unit = "µs" }
vec_clone_50000 = { mean = 3.81, median = 3.79, std_dev = 0.07, unit = "ms" }

[results.file_io]
buffered_file = { mean = 83.11, median = 73.18, std_dev = 17.85, unit = "µs" }
unbuffered_file = { mean = 238.98, median = 216.54, std_dev = 45.80, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 148.78, median = 132.79, std_dev = 27.93, unit = "µs" }
viewport_24 = { mean = 41.81, median = 39.31, std_dev = 5.63, unit = "µs" }
viewport_50 = { mean = 70.61, median = 70.29, std_dev = 0.90, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 78.17, median = 77.47, std_dev = 6.52, unit = "µs" }
without_completion_popup = { mean = 65.42, median = 61.62, std_dev = 7.61, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 205.12, median = 186.34, std_dev = 35.27, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 8.52, median = 7.89, std_dev = 1.36, unit = "ms" }
scroll_half_page = { mean = 818.90, median = 786.69, std_dev = 88.64, unit = "µs" }
scroll_one_line = { mean = 795.30, median = 785.41, std_dev = 34.95, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 8.12, median = 7.66, std_dev = 1.25, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 87.45, median = 78.92, std_dev = 19.72, unit = "µs" }
single_char_render = { mean = 76.48, median = 71.44, std_dev = 11.90, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 67.78, median = 62.71, std_dev = 9.83, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 67.65, median = 61.83, std_dev = 14.21, unit = "µs" }
open_explorer = { mean = 63.65, median = 61.12, std_dev = 7.03, unit = "µs" }
toggle_cycle = { mean = 128.11, median = 125.20, std_dev = 9.88, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 110.14, median = 106.83, std_dev = 8.77, unit = "µs" }
char_insert_rtt = { mean = 109.23, median = 107.17, std_dev = 8.64, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 940.29, median = 824.75, std_dev = 247.89, unit = "µs" }
half_page_down_rtt = { mean = 880.77, median = 811.85, std_dev = 138.09, unit = "µs" }
move_down_rtt = { mean = 853.51, median = 811.96, std_dev = 79.34, unit = "µs" }
move_right_rtt = { mean = 140.53, median = 137.70, std_dev = 9.62, unit = "µs" }
word_forward_rtt = { mean = 146.01, median = 136.31, std_dev = 26.77, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 63.20, median = 63.19, std_dev = 0.11, unit = "µs" }
full_render_1000 = { mean = 68.87, median = 62.39, std_dev = 10.58, unit = "µs" }
full_render_10000 = { mean = 64.77, median = 62.92, std_dev = 4.78, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 132.24, median = 120.64, std_dev = 24.20, unit = "µs" }
viewport_height_24 = { mean = 32.98, median = 31.82, std_dev = 2.88, unit = "µs" }
viewport_height_50 = { mean = 72.24, median = 64.06, std_dev = 14.39, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 1.76, median = 1.49, std_dev = 0.38, unit = "ms" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 17.81, median = 16.36, std_dev = 2.86, unit = "ms" }
edit_navigate_cycle_1k = { mean = 3.22, median = 2.95, std_dev = 0.45, unit = "ms" }
edit_navigate_cycle_50k = { mean = 80.22, median = 77.29, std_dev = 6.62, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 4.48, median = 4.42, std_dev = 0.19, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 40.80, median = 40.40, std_dev = 1.14, unit = "ms" }
jump_around_file = { mean = 6.74, median = 6.46, std_dev = 0.64, unit = "ms" }
spam_ctrl_d_10_times = { mean = 8.16, median = 8.08, std_dev = 0.33, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 42.55, median = 39.89, std_dev = 5.14, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 59.90, median = 55.11, std_dev = 10.19, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 117.59, median = 112.51, std_dev = 13.49, unit = "µs" }
height_200 = { mean = 227.74, median = 220.39, std_dev = 25.37, unit = "µs" }
height_24 = { mean = 29.99, median = 27.53, std_dev = 4.07, unit = "µs" }
height_50 = { mean = 61.70, median = 56.33, std_dev = 10.28, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 10.19, median = 10.16, std_dev = 0.09, unit = "µs" }
buffer_lines_100 = { mean = 57.57, median = 54.48, std_dev = 7.69, unit = "µs" }
buffer_lines_1000 = { mean = 60.34, median = 54.95, std_dev = 10.51, unit = "µs" }
buffer_lines_10000 = { mean = 62.27, median = 56.04, std_dev = 12.20, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 55.48, median = 54.86, std_dev = 2.45, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 7.18 µs | 6.05 µs | 1.75 µs | [6.59, 7.83] µs |
| buffer_clone/clone/1000 | 84.93 µs | 72.11 µs | 17.42 µs | [79.07, 91.25] µs |
| buffer_clone/clone/10000 | 771.36 µs | 722.79 µs | 89.69 µs | [742.28, 804.88] µs |
| buffer_clone/clone/50000 | 4.07 ms | 3.77 ms | 0.62 ms | [3.87, 4.31] ms |
| buffer_vec/vec_clone/100 | 6.68 µs | 5.94 µs | 1.26 µs | [6.26, 7.15] µs |
| buffer_vec/vec_clone/1000 | 73.23 µs | 72.69 µs | 1.92 µs | [72.65, 73.99] µs |
| buffer_vec/vec_clone/10000 | 729.45 µs | 725.56 µs | 9.80 µs | [726.53, 733.36] µs |
| buffer_vec/vec_clone/50000 | 3.81 ms | 3.79 ms | 0.07 ms | [3.79, 3.83] ms |
| file_io/buffered_file | 83.11 µs | 73.18 µs | 17.85 µs | [77.25, 89.76] µs |
| file_io/unbuffered_file | 238.98 µs | 216.54 µs | 45.80 µs | [224.42, 256.46] µs |
| file_io_viewport/viewport/100 | 148.78 µs | 132.79 µs | 27.93 µs | [139.59, 159.03] µs |
| file_io_viewport/viewport/24 | 41.81 µs | 39.31 µs | 5.63 µs | [40.02, 43.93] µs |
| file_io_viewport/viewport/50 | 70.61 µs | 70.29 µs | 0.90 µs | [70.32, 70.95] µs |
| input_completion/with_completion_popup | 78.17 µs | 77.47 µs | 6.52 µs | [76.06, 80.65] µs |
| input_completion/without_completion_popup | 65.42 µs | 61.62 µs | 7.61 µs | [63.01, 68.45] µs |
| input_mode_switch/normal_insert_normal | 205.12 µs | 186.34 µs | 35.27 µs | [193.90, 218.47] µs |
| input_scrolling/scroll_10_lines | 8.52 ms | 7.89 ms | 1.36 ms | [8.11, 9.05] ms |
| input_scrolling/scroll_half_page | 818.90 µs | 786.69 µs | 88.64 µs | [790.79, 854.33] µs |
| input_scrolling/scroll_one_line | 795.30 µs | 785.41 µs | 34.95 µs | [785.70, 809.13] µs |
| input_sustained/100_keystrokes_each_rendered | 8.12 ms | 7.66 ms | 1.25 ms | [7.81, 8.50] ms |
| input_typing/burst_10_chars_render | 87.45 µs | 78.92 µs | 19.72 µs | [80.90, 94.70] µs |
| input_typing/single_char_render | 76.48 µs | 71.44 µs | 11.90 µs | [72.80, 81.17] µs |
| io_bytes/full_screen_render | 67.78 µs | 62.71 µs | 9.83 µs | [64.59, 71.51] µs |
| rtt_explorer/close_explorer | 67.65 µs | 61.83 µs | 14.21 µs | [63.32, 73.08] µs |
| rtt_explorer/open_explorer | 63.65 µs | 61.12 µs | 7.03 µs | [61.41, 66.42] µs |
| rtt_explorer/toggle_cycle | 128.11 µs | 125.20 µs | 9.88 µs | [125.41, 132.19] µs |
| rtt_input_lag/backspace_rtt | 110.14 µs | 106.83 µs | 8.77 µs | [107.54, 113.63] µs |
| rtt_input_lag/char_insert_rtt | 109.23 µs | 107.17 µs | 8.64 µs | [107.17, 112.67] µs |
| rtt_movement_lag/goto_top_rtt | 940.29 µs | 824.75 µs | 247.89 µs | [863.42, 1034.62] µs |
| rtt_movement_lag/half_page_down_rtt | 880.77 µs | 811.85 µs | 138.09 µs | [836.34, 933.20] µs |
| rtt_movement_lag/move_down_rtt | 853.51 µs | 811.96 µs | 79.34 µs | [828.04, 882.93] µs |
| rtt_movement_lag/move_right_rtt | 140.53 µs | 137.70 µs | 9.62 µs | [137.90, 144.39] µs |
| rtt_movement_lag/word_forward_rtt | 146.01 µs | 136.31 µs | 26.77 µs | [137.91, 156.47] µs |
| screen_io/full_render/100 | 63.20 µs | 63.19 µs | 0.11 µs | [63.16, 63.24] µs |
| screen_io/full_render/1000 | 68.87 µs | 62.39 µs | 10.58 µs | [65.35, 72.80] µs |
| screen_io/full_render/10000 | 64.77 µs | 62.92 µs | 4.78 µs | [63.33, 66.61] µs |
| screen_viewport_io/viewport_height/100 | 132.24 µs | 120.64 µs | 24.20 µs | [124.89, 141.44] µs |
| screen_viewport_io/viewport_height/24 | 32.98 µs | 31.82 µs | 2.88 µs | [32.09, 34.09] µs |
| screen_viewport_io/viewport_height/50 | 72.24 µs | 64.06 µs | 14.39 µs | [67.57, 77.49] µs |
| stress_completion/completion_scroll_100_items | 1.76 ms | 1.49 ms | 0.38 ms | [1.63, 1.90] ms |
| stress_editing/edit_navigate_cycle_10k | 17.81 ms | 16.36 ms | 2.86 ms | [16.90, 18.89] ms |
| stress_editing/edit_navigate_cycle_1k | 3.22 ms | 2.95 ms | 0.45 ms | [3.07, 3.39] ms |
| stress_editing/edit_navigate_cycle_50k | 80.22 ms | 77.29 ms | 6.62 ms | [78.18, 82.83] ms |
| stress_mode_ops/insert_escape_move_cycle | 4.48 ms | 4.42 ms | 0.19 ms | [4.42, 4.56] ms |
| stress_scroll/hold_j_50_lines_10k_file | 40.80 ms | 40.40 ms | 1.14 ms | [40.46, 41.25] ms |
| stress_scroll/jump_around_file | 6.74 ms | 6.46 ms | 0.64 ms | [6.54, 6.99] ms |
| stress_scroll/spam_ctrl_d_10_times | 8.16 ms | 8.08 ms | 0.33 ms | [8.08, 8.29] ms |
| stress_worst_case/all_features_50k_file | 42.55 ms | 39.89 ms | 5.14 ms | [40.50, 44.85] ms |
| throughput/renders_per_second | 59.90 µs | 55.11 µs | 10.19 µs | [56.72, 63.92] µs |
| viewport_size/height/100 | 117.59 µs | 112.51 µs | 13.49 µs | [113.45, 122.84] µs |
| viewport_size/height/200 | 227.74 µs | 220.39 µs | 25.37 µs | [219.90, 237.75] µs |
| viewport_size/height/24 | 29.99 µs | 27.53 µs | 4.07 µs | [28.64, 31.53] µs |
| viewport_size/height/50 | 61.70 µs | 56.33 µs | 10.28 µs | [58.42, 65.59] µs |
| window_render/buffer_lines/10 | 10.19 µs | 10.16 µs | 0.09 µs | [10.16, 10.22] µs |
| window_render/buffer_lines/100 | 57.57 µs | 54.48 µs | 7.69 µs | [55.16, 60.64] µs |
| window_render/buffer_lines/1000 | 60.34 µs | 54.95 µs | 10.51 µs | [57.00, 64.46] µs |
| window_render/buffer_lines/10000 | 62.27 µs | 56.04 µs | 12.20 µs | [58.35, 66.79] µs |
| with_highlights/no_highlights | 55.48 µs | 54.86 µs | 2.45 µs | [54.93, 56.44] µs |
