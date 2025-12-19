# Performance Benchmarks v0.6.1

> Last updated: 2025-12-19T04:41:23Z | Commit: 53aae8d | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.6.1"
commit = "53aae8d"
date = "2025-12-19T04:41:23Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 229.92 µs |
| buffer_vec | 4 | 209.93 µs |
| file_io | 2 | 178.91 µs |
| file_io_viewport | 3 | 94.59 µs |
| input_completion | 2 | 79.03 µs |
| input_mode_switch | 1 | 241.93 µs |
| input_scrolling | 3 | 645.78 ms |
| input_sustained | 1 | 9.06 ms |
| input_typing | 2 | 88.06 µs |
| io_bytes | 1 | 81.25 µs |
| rtt_explorer | 3 | 97.62 µs |
| rtt_input_lag | 2 | 128.49 µs |
| rtt_movement_lag | 5 | 634.37 µs |
| screen_io | 3 | 79.81 µs |
| screen_viewport_io | 3 | 85.25 µs |
| stress_completion | 1 | 1.78 ms |
| stress_editing | 3 | 36.60 ms |
| stress_mode_ops | 1 | 4.87 ms |
| stress_scroll | 3 | 22.65 ms |
| stress_worst_case | 1 | 45.55 ms |
| throughput | 1 | 77.26 µs |
| viewport_size | 4 | 123.89 µs |
| window_render | 4 | 49.31 µs |
| with_highlights | 1 | 74.06 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 6.93, median = 6.54, std_dev = 1.01, unit = "µs" }
clone_1000 = { mean = 86.82, median = 87.32, std_dev = 12.02, unit = "µs" }
clone_10000 = { mean = 822.14, median = 740.52, std_dev = 146.19, unit = "µs" }
clone_50000 = { mean = 3.80, median = 3.74, std_dev = 0.22, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 6.02, median = 5.98, std_dev = 0.13, unit = "µs" }
vec_clone_1000 = { mean = 73.91, median = 72.03, std_dev = 5.43, unit = "µs" }
vec_clone_10000 = { mean = 755.96, median = 734.05, std_dev = 71.75, unit = "µs" }
vec_clone_50000 = { mean = 3.83, median = 3.76, std_dev = 0.23, unit = "ms" }

[results.file_io]
buffered_file = { mean = 80.01, median = 69.25, std_dev = 16.06, unit = "µs" }
unbuffered_file = { mean = 277.81, median = 289.35, std_dev = 35.14, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 154.69, median = 148.03, std_dev = 28.73, unit = "µs" }
viewport_24 = { mean = 42.25, median = 38.33, std_dev = 6.75, unit = "µs" }
viewport_50 = { mean = 86.84, median = 79.71, std_dev = 19.10, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 88.28, median = 89.13, std_dev = 15.82, unit = "µs" }
without_completion_popup = { mean = 69.77, median = 62.16, std_dev = 11.68, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 241.93, median = 235.38, std_dev = 52.76, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 9.17, median = 9.34, std_dev = 1.15, unit = "ms" }
scroll_half_page = { mean = 959.38, median = 944.12, std_dev = 133.73, unit = "µs" }
scroll_one_line = { mean = 968.78, median = 939.02, std_dev = 220.31, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 9.06, median = 8.32, std_dev = 1.77, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 92.74, median = 89.24, std_dev = 17.35, unit = "µs" }
single_char_render = { mean = 83.38, median = 78.11, std_dev = 16.39, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 81.25, median = 83.18, std_dev = 10.49, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 72.60, median = 69.68, std_dev = 12.83, unit = "µs" }
open_explorer = { mean = 71.97, median = 71.01, std_dev = 10.72, unit = "µs" }
toggle_cycle = { mean = 148.29, median = 144.13, std_dev = 26.21, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 131.24, median = 131.85, std_dev = 21.72, unit = "µs" }
char_insert_rtt = { mean = 125.74, median = 117.72, std_dev = 20.78, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 905.66, median = 824.37, std_dev = 163.72, unit = "µs" }
half_page_down_rtt = { mean = 983.12, median = 969.65, std_dev = 131.33, unit = "µs" }
move_down_rtt = { mean = 960.94, median = 887.12, std_dev = 153.79, unit = "µs" }
move_right_rtt = { mean = 161.07, median = 160.29, std_dev = 20.47, unit = "µs" }
word_forward_rtt = { mean = 161.07, median = 151.96, std_dev = 26.85, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 84.37, median = 80.08, std_dev = 18.51, unit = "µs" }
full_render_1000 = { mean = 80.58, median = 81.27, std_dev = 14.48, unit = "µs" }
full_render_10000 = { mean = 74.47, median = 72.94, std_dev = 12.09, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 144.66, median = 137.93, std_dev = 25.55, unit = "µs" }
viewport_height_24 = { mean = 37.71, median = 34.87, std_dev = 7.83, unit = "µs" }
viewport_height_50 = { mean = 73.37, median = 71.16, std_dev = 11.59, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 1.78, median = 1.82, std_dev = 0.31, unit = "ms" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 17.47, median = 16.29, std_dev = 2.01, unit = "ms" }
edit_navigate_cycle_1k = { mean = 3.54, median = 3.55, std_dev = 0.55, unit = "ms" }
edit_navigate_cycle_50k = { mean = 88.79, median = 88.59, std_dev = 11.32, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 4.87, median = 4.54, std_dev = 0.63, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 50.43, median = 50.47, std_dev = 5.31, unit = "ms" }
jump_around_file = { mean = 7.95, median = 7.70, std_dev = 1.04, unit = "ms" }
spam_ctrl_d_10_times = { mean = 9.56, median = 9.32, std_dev = 1.14, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 45.55, median = 44.62, std_dev = 5.21, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 77.26, median = 79.76, std_dev = 16.08, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 139.02, median = 138.47, std_dev = 25.28, unit = "µs" }
height_200 = { mean = 266.41, median = 274.04, std_dev = 44.83, unit = "µs" }
height_24 = { mean = 28.42, median = 26.21, std_dev = 4.07, unit = "µs" }
height_50 = { mean = 61.70, median = 59.22, std_dev = 7.03, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 11.62, median = 11.33, std_dev = 1.66, unit = "µs" }
buffer_lines_100 = { mean = 58.13, median = 54.54, std_dev = 10.08, unit = "µs" }
buffer_lines_1000 = { mean = 65.81, median = 57.33, std_dev = 14.20, unit = "µs" }
buffer_lines_10000 = { mean = 61.68, median = 56.20, std_dev = 9.17, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 74.06, median = 72.41, std_dev = 15.77, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 6.93 µs | 6.54 µs | 1.01 µs | [6.58, 7.29] µs |
| buffer_clone/clone/1000 | 86.82 µs | 87.32 µs | 12.02 µs | [82.71, 91.12] µs |
| buffer_clone/clone/10000 | 822.14 µs | 740.52 µs | 146.19 µs | [776.75, 879.10] µs |
| buffer_clone/clone/50000 | 3.80 ms | 3.74 ms | 0.22 ms | [3.74, 3.89] ms |
| buffer_vec/vec_clone/100 | 6.02 µs | 5.98 µs | 0.13 µs | [5.98, 6.07] µs |
| buffer_vec/vec_clone/1000 | 73.91 µs | 72.03 µs | 5.43 µs | [72.22, 76.06] µs |
| buffer_vec/vec_clone/10000 | 755.96 µs | 734.05 µs | 71.75 µs | [733.65, 784.20] µs |
| buffer_vec/vec_clone/50000 | 3.83 ms | 3.76 ms | 0.23 ms | [3.76, 3.92] ms |
| file_io/buffered_file | 80.01 µs | 69.25 µs | 16.06 µs | [74.69, 85.91] µs |
| file_io/unbuffered_file | 277.81 µs | 289.35 µs | 35.14 µs | [265.42, 290.15] µs |
| file_io_viewport/viewport/100 | 154.69 µs | 148.03 µs | 28.73 µs | [144.90, 165.08] µs |
| file_io_viewport/viewport/24 | 42.25 µs | 38.33 µs | 6.75 µs | [40.05, 44.75] µs |
| file_io_viewport/viewport/50 | 86.84 µs | 79.71 µs | 19.10 µs | [80.42, 93.87] µs |
| input_completion/with_completion_popup | 88.28 µs | 89.13 µs | 15.82 µs | [82.78, 93.91] µs |
| input_completion/without_completion_popup | 69.77 µs | 62.16 µs | 11.68 µs | [65.89, 74.07] µs |
| input_mode_switch/normal_insert_normal | 241.93 µs | 235.38 µs | 52.76 µs | [223.62, 260.62] µs |
| input_scrolling/scroll_10_lines | 9.17 ms | 9.34 ms | 1.15 ms | [8.78, 9.59] ms |
| input_scrolling/scroll_half_page | 959.38 µs | 944.12 µs | 133.73 µs | [914.18, 1007.91] µs |
| input_scrolling/scroll_one_line | 968.78 µs | 939.02 µs | 220.31 µs | [900.30, 1054.44] µs |
| input_sustained/100_keystrokes_each_rendered | 9.06 ms | 8.32 ms | 1.77 ms | [8.59, 9.56] ms |
| input_typing/burst_10_chars_render | 92.74 µs | 89.24 µs | 17.35 µs | [86.74, 99.10] µs |
| input_typing/single_char_render | 83.38 µs | 78.11 µs | 16.39 µs | [78.05, 89.48] µs |
| io_bytes/full_screen_render | 81.25 µs | 83.18 µs | 10.49 µs | [77.49, 84.92] µs |
| rtt_explorer/close_explorer | 72.60 µs | 69.68 µs | 12.83 µs | [68.25, 77.26] µs |
| rtt_explorer/open_explorer | 71.97 µs | 71.01 µs | 10.72 µs | [68.30, 75.83] µs |
| rtt_explorer/toggle_cycle | 148.29 µs | 144.13 µs | 26.21 µs | [139.33, 157.69] µs |
| rtt_input_lag/backspace_rtt | 131.24 µs | 131.85 µs | 21.72 µs | [124.01, 139.27] µs |
| rtt_input_lag/char_insert_rtt | 125.74 µs | 117.72 µs | 20.78 µs | [118.70, 133.28] µs |
| rtt_movement_lag/goto_top_rtt | 905.66 µs | 824.37 µs | 163.72 µs | [852.85, 967.01] µs |
| rtt_movement_lag/half_page_down_rtt | 983.12 µs | 969.65 µs | 131.33 µs | [937.68, 1030.08] µs |
| rtt_movement_lag/move_down_rtt | 960.94 µs | 887.12 µs | 153.79 µs | [908.39, 1016.80] µs |
| rtt_movement_lag/move_right_rtt | 161.07 µs | 160.29 µs | 20.47 µs | [154.00, 168.30] µs |
| rtt_movement_lag/word_forward_rtt | 161.07 µs | 151.96 µs | 26.85 µs | [152.05, 170.82] µs |
| screen_io/full_render/100 | 84.37 µs | 80.08 µs | 18.51 µs | [77.96, 90.92] µs |
| screen_io/full_render/1000 | 80.58 µs | 81.27 µs | 14.48 µs | [75.48, 85.69] µs |
| screen_io/full_render/10000 | 74.47 µs | 72.94 µs | 12.09 µs | [70.37, 78.89] µs |
| screen_viewport_io/viewport_height/100 | 144.66 µs | 137.93 µs | 25.55 µs | [136.04, 153.86] µs |
| screen_viewport_io/viewport_height/24 | 37.71 µs | 34.87 µs | 7.83 µs | [35.11, 40.60] µs |
| screen_viewport_io/viewport_height/50 | 73.37 µs | 71.16 µs | 11.59 µs | [69.55, 77.62] µs |
| stress_completion/completion_scroll_100_items | 1.78 ms | 1.82 ms | 0.31 ms | [1.68, 1.89] ms |
| stress_editing/edit_navigate_cycle_10k | 17.47 ms | 16.29 ms | 2.01 ms | [16.81, 18.21] ms |
| stress_editing/edit_navigate_cycle_1k | 3.54 ms | 3.55 ms | 0.55 ms | [3.35, 3.74] ms |
| stress_editing/edit_navigate_cycle_50k | 88.79 ms | 88.59 ms | 11.32 ms | [84.91, 92.86] ms |
| stress_mode_ops/insert_escape_move_cycle | 4.87 ms | 4.54 ms | 0.63 ms | [4.66, 5.10] ms |
| stress_scroll/hold_j_50_lines_10k_file | 50.43 ms | 50.47 ms | 5.31 ms | [48.56, 52.28] ms |
| stress_scroll/jump_around_file | 7.95 ms | 7.70 ms | 1.04 ms | [7.59, 8.32] ms |
| stress_scroll/spam_ctrl_d_10_times | 9.56 ms | 9.32 ms | 1.14 ms | [9.18, 9.97] ms |
| stress_worst_case/all_features_50k_file | 45.55 ms | 44.62 ms | 5.21 ms | [43.38, 47.86] ms |
| throughput/renders_per_second | 77.26 µs | 79.76 µs | 16.08 µs | [71.67, 82.98] µs |
| viewport_size/height/100 | 139.02 µs | 138.47 µs | 25.28 µs | [130.69, 148.48] µs |
| viewport_size/height/200 | 266.41 µs | 274.04 µs | 44.83 µs | [250.56, 282.42] µs |
| viewport_size/height/24 | 28.42 µs | 26.21 µs | 4.07 µs | [27.11, 29.94] µs |
| viewport_size/height/50 | 61.70 µs | 59.22 µs | 7.03 µs | [59.32, 64.22] µs |
| window_render/buffer_lines/10 | 11.62 µs | 11.33 µs | 1.66 µs | [11.08, 12.21] µs |
| window_render/buffer_lines/100 | 58.13 µs | 54.54 µs | 10.08 µs | [55.13, 62.10] µs |
| window_render/buffer_lines/1000 | 65.81 µs | 57.33 µs | 14.20 µs | [61.06, 71.07] µs |
| window_render/buffer_lines/10000 | 61.68 µs | 56.20 µs | 9.17 µs | [58.60, 65.03] µs |
| with_highlights/no_highlights | 74.06 µs | 72.41 µs | 15.77 µs | [68.68, 79.68] µs |
