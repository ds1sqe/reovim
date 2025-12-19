# Performance Benchmarks v0.6.6

> Last updated: 2025-12-19T17:00:50Z | Commit: 37cac92 | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.6.6"
commit = "37cac92"
date = "2025-12-19T17:00:50Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 198.91 µs |
| buffer_vec | 4 | 198.46 µs |
| file_io | 2 | 143.79 µs |
| file_io_viewport | 3 | 80.58 µs |
| input_completion | 2 | 65.79 µs |
| input_mode_switch | 1 | 188.54 µs |
| input_scrolling | 3 | 524.78 ms |
| input_sustained | 1 | 7.67 ms |
| input_typing | 2 | 73.26 µs |
| io_bytes | 1 | 62.80 µs |
| rtt_explorer | 3 | 82.95 µs |
| rtt_input_lag | 2 | 107.24 µs |
| rtt_movement_lag | 5 | 535.80 µs |
| screen_io | 3 | 62.12 µs |
| screen_viewport_io | 3 | 72.68 µs |
| stress_completion | 1 | 1.47 ms |
| stress_editing | 3 | 32.00 ms |
| stress_mode_ops | 1 | 4.41 ms |
| stress_scroll | 3 | 18.24 ms |
| stress_worst_case | 1 | 38.57 ms |
| throughput | 1 | 55.26 µs |
| viewport_size | 4 | 98.81 µs |
| window_render | 4 | 44.16 µs |
| with_highlights | 1 | 55.11 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 5.79, median = 5.78, std_dev = 0.05, unit = "µs" }
clone_1000 = { mean = 70.96, median = 70.90, std_dev = 0.30, unit = "µs" }
clone_10000 = { mean = 715.21, median = 713.30, std_dev = 3.80, unit = "µs" }
clone_50000 = { mean = 3.69, median = 3.69, std_dev = 0.01, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 5.81, median = 5.81, std_dev = 0.02, unit = "µs" }
vec_clone_1000 = { mean = 70.88, median = 70.89, std_dev = 0.12, unit = "µs" }
vec_clone_10000 = { mean = 713.44, median = 712.99, std_dev = 2.62, unit = "µs" }
vec_clone_50000 = { mean = 3.70, median = 3.70, std_dev = 0.01, unit = "ms" }

[results.file_io]
buffered_file = { mean = 71.38, median = 71.27, std_dev = 0.50, unit = "µs" }
unbuffered_file = { mean = 216.20, median = 216.35, std_dev = 0.89, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 130.57, median = 130.30, std_dev = 0.72, unit = "µs" }
viewport_24 = { mean = 39.34, median = 39.28, std_dev = 0.36, unit = "µs" }
viewport_50 = { mean = 71.83, median = 71.68, std_dev = 0.73, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 71.41, median = 71.35, std_dev = 0.20, unit = "µs" }
without_completion_popup = { mean = 60.18, median = 60.12, std_dev = 0.18, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 188.54, median = 188.51, std_dev = 0.22, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 7.81, median = 7.81, std_dev = 0.01, unit = "ms" }
scroll_half_page = { mean = 782.45, median = 782.08, std_dev = 1.97, unit = "µs" }
scroll_one_line = { mean = 784.07, median = 783.84, std_dev = 3.17, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 7.67, median = 7.66, std_dev = 0.04, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 75.46, median = 75.79, std_dev = 1.15, unit = "µs" }
single_char_render = { mean = 71.07, median = 71.04, std_dev = 0.15, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 62.80, median = 62.75, std_dev = 0.26, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 62.00, median = 61.97, std_dev = 0.14, unit = "µs" }
open_explorer = { mean = 62.21, median = 62.18, std_dev = 0.13, unit = "µs" }
toggle_cycle = { mean = 124.64, median = 124.58, std_dev = 0.28, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 106.12, median = 106.10, std_dev = 0.50, unit = "µs" }
char_insert_rtt = { mean = 108.36, median = 108.24, std_dev = 0.61, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 801.81, median = 801.42, std_dev = 2.87, unit = "µs" }
half_page_down_rtt = { mean = 803.45, median = 802.93, std_dev = 3.45, unit = "µs" }
move_down_rtt = { mean = 803.43, median = 803.32, std_dev = 2.73, unit = "µs" }
move_right_rtt = { mean = 135.84, median = 135.81, std_dev = 0.54, unit = "µs" }
word_forward_rtt = { mean = 134.48, median = 134.45, std_dev = 0.51, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 60.73, median = 60.69, std_dev = 0.18, unit = "µs" }
full_render_1000 = { mean = 63.37, median = 63.38, std_dev = 0.12, unit = "µs" }
full_render_10000 = { mean = 62.26, median = 62.28, std_dev = 0.36, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 122.69, median = 122.56, std_dev = 0.67, unit = "µs" }
viewport_height_24 = { mean = 32.04, median = 32.03, std_dev = 0.09, unit = "µs" }
viewport_height_50 = { mean = 63.30, median = 63.30, std_dev = 0.15, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 1.47, median = 1.47, std_dev = 0.00, unit = "ms" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 16.15, median = 16.14, std_dev = 0.03, unit = "ms" }
edit_navigate_cycle_1k = { mean = 2.93, median = 2.92, std_dev = 0.01, unit = "ms" }
edit_navigate_cycle_50k = { mean = 76.91, median = 76.92, std_dev = 0.10, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 4.41, median = 4.40, std_dev = 0.03, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 40.22, median = 40.13, std_dev = 0.36, unit = "ms" }
jump_around_file = { mean = 6.44, median = 6.44, std_dev = 0.02, unit = "ms" }
spam_ctrl_d_10_times = { mean = 8.06, median = 8.06, std_dev = 0.02, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 38.57, median = 38.56, std_dev = 0.09, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 55.26, median = 55.24, std_dev = 0.10, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 107.35, median = 107.30, std_dev = 0.37, unit = "µs" }
height_200 = { mean = 207.00, median = 206.58, std_dev = 3.06, unit = "µs" }
height_24 = { mean = 26.60, median = 26.59, std_dev = 0.05, unit = "µs" }
height_50 = { mean = 54.28, median = 54.20, std_dev = 0.29, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 10.14, median = 10.14, std_dev = 0.03, unit = "µs" }
buffer_lines_100 = { mean = 55.77, median = 55.73, std_dev = 0.62, unit = "µs" }
buffer_lines_1000 = { mean = 54.84, median = 54.57, std_dev = 1.45, unit = "µs" }
buffer_lines_10000 = { mean = 55.92, median = 55.90, std_dev = 0.50, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 55.11, median = 55.09, std_dev = 0.13, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 5.79 µs | 5.78 µs | 0.05 µs | [5.78, 5.81] µs |
| buffer_clone/clone/1000 | 70.96 µs | 70.90 µs | 0.30 µs | [70.88, 71.08] µs |
| buffer_clone/clone/10000 | 715.21 µs | 713.30 µs | 3.80 µs | [713.97, 716.62] µs |
| buffer_clone/clone/50000 | 3.69 ms | 3.69 ms | 0.01 ms | [3.69, 3.69] ms |
| buffer_vec/vec_clone/100 | 5.81 µs | 5.81 µs | 0.02 µs | [5.81, 5.82] µs |
| buffer_vec/vec_clone/1000 | 70.88 µs | 70.89 µs | 0.12 µs | [70.84, 70.92] µs |
| buffer_vec/vec_clone/10000 | 713.44 µs | 712.99 µs | 2.62 µs | [712.80, 714.48] µs |
| buffer_vec/vec_clone/50000 | 3.70 ms | 3.70 ms | 0.01 ms | [3.70, 3.70] ms |
| file_io/buffered_file | 71.38 µs | 71.27 µs | 0.50 µs | [71.23, 71.57] µs |
| file_io/unbuffered_file | 216.20 µs | 216.35 µs | 0.89 µs | [215.89, 216.52] µs |
| file_io_viewport/viewport/100 | 130.57 µs | 130.30 µs | 0.72 µs | [130.34, 130.84] µs |
| file_io_viewport/viewport/24 | 39.34 µs | 39.28 µs | 0.36 µs | [39.25, 39.49] µs |
| file_io_viewport/viewport/50 | 71.83 µs | 71.68 µs | 0.73 µs | [71.61, 72.12] µs |
| input_completion/with_completion_popup | 71.41 µs | 71.35 µs | 0.20 µs | [71.35, 71.49] µs |
| input_completion/without_completion_popup | 60.18 µs | 60.12 µs | 0.18 µs | [60.13, 60.25] µs |
| input_mode_switch/normal_insert_normal | 188.54 µs | 188.51 µs | 0.22 µs | [188.47, 188.62] µs |
| input_scrolling/scroll_10_lines | 7.81 ms | 7.81 ms | 0.01 ms | [7.81, 7.81] ms |
| input_scrolling/scroll_half_page | 782.45 µs | 782.08 µs | 1.97 µs | [781.77, 783.15] µs |
| input_scrolling/scroll_one_line | 784.07 µs | 783.84 µs | 3.17 µs | [782.97, 785.20] µs |
| input_sustained/100_keystrokes_each_rendered | 7.67 ms | 7.66 ms | 0.04 ms | [7.66, 7.68] ms |
| input_typing/burst_10_chars_render | 75.46 µs | 75.79 µs | 1.15 µs | [75.05, 75.85] µs |
| input_typing/single_char_render | 71.07 µs | 71.04 µs | 0.15 µs | [71.02, 71.12] µs |
| io_bytes/full_screen_render | 62.80 µs | 62.75 µs | 0.26 µs | [62.73, 62.91] µs |
| rtt_explorer/close_explorer | 62.00 µs | 61.97 µs | 0.14 µs | [61.96, 62.06] µs |
| rtt_explorer/open_explorer | 62.21 µs | 62.18 µs | 0.13 µs | [62.18, 62.26] µs |
| rtt_explorer/toggle_cycle | 124.64 µs | 124.58 µs | 0.28 µs | [124.56, 124.76] µs |
| rtt_input_lag/backspace_rtt | 106.12 µs | 106.10 µs | 0.50 µs | [105.95, 106.30] µs |
| rtt_input_lag/char_insert_rtt | 108.36 µs | 108.24 µs | 0.61 µs | [108.17, 108.59] µs |
| rtt_movement_lag/goto_top_rtt | 801.81 µs | 801.42 µs | 2.87 µs | [800.85, 802.87] µs |
| rtt_movement_lag/half_page_down_rtt | 803.45 µs | 802.93 µs | 3.45 µs | [802.27, 804.73] µs |
| rtt_movement_lag/move_down_rtt | 803.43 µs | 803.32 µs | 2.73 µs | [802.49, 804.42] µs |
| rtt_movement_lag/move_right_rtt | 135.84 µs | 135.81 µs | 0.54 µs | [135.67, 136.05] µs |
| rtt_movement_lag/word_forward_rtt | 134.48 µs | 134.45 µs | 0.51 µs | [134.30, 134.67] µs |
| screen_io/full_render/100 | 60.73 µs | 60.69 µs | 0.18 µs | [60.68, 60.81] µs |
| screen_io/full_render/1000 | 63.37 µs | 63.38 µs | 0.12 µs | [63.32, 63.41] µs |
| screen_io/full_render/10000 | 62.26 µs | 62.28 µs | 0.36 µs | [62.15, 62.39] µs |
| screen_viewport_io/viewport_height/100 | 122.69 µs | 122.56 µs | 0.67 µs | [122.53, 122.95] µs |
| screen_viewport_io/viewport_height/24 | 32.04 µs | 32.03 µs | 0.09 µs | [32.01, 32.08] µs |
| screen_viewport_io/viewport_height/50 | 63.30 µs | 63.30 µs | 0.15 µs | [63.25, 63.36] µs |
| stress_completion/completion_scroll_100_items | 1.47 ms | 1.47 ms | 0.00 ms | [1.47, 1.47] ms |
| stress_editing/edit_navigate_cycle_10k | 16.15 ms | 16.14 ms | 0.03 ms | [16.14, 16.16] ms |
| stress_editing/edit_navigate_cycle_1k | 2.93 ms | 2.92 ms | 0.01 ms | [2.92, 2.93] ms |
| stress_editing/edit_navigate_cycle_50k | 76.91 ms | 76.92 ms | 0.10 ms | [76.87, 76.95] ms |
| stress_mode_ops/insert_escape_move_cycle | 4.41 ms | 4.40 ms | 0.03 ms | [4.40, 4.43] ms |
| stress_scroll/hold_j_50_lines_10k_file | 40.22 ms | 40.13 ms | 0.36 ms | [40.11, 40.36] ms |
| stress_scroll/jump_around_file | 6.44 ms | 6.44 ms | 0.02 ms | [6.44, 6.45] ms |
| stress_scroll/spam_ctrl_d_10_times | 8.06 ms | 8.06 ms | 0.02 ms | [8.05, 8.06] ms |
| stress_worst_case/all_features_50k_file | 38.57 ms | 38.56 ms | 0.09 ms | [38.53, 38.61] ms |
| throughput/renders_per_second | 55.26 µs | 55.24 µs | 0.10 µs | [55.23, 55.29] µs |
| viewport_size/height/100 | 107.35 µs | 107.30 µs | 0.37 µs | [107.24, 107.50] µs |
| viewport_size/height/200 | 207.00 µs | 206.58 µs | 3.06 µs | [206.30, 208.20] µs |
| viewport_size/height/24 | 26.60 µs | 26.59 µs | 0.05 µs | [26.58, 26.62] µs |
| viewport_size/height/50 | 54.28 µs | 54.20 µs | 0.29 µs | [54.20, 54.40] µs |
| window_render/buffer_lines/10 | 10.14 µs | 10.14 µs | 0.03 µs | [10.13, 10.15] µs |
| window_render/buffer_lines/100 | 55.77 µs | 55.73 µs | 0.62 µs | [55.58, 56.01] µs |
| window_render/buffer_lines/1000 | 54.84 µs | 54.57 µs | 1.45 µs | [54.56, 55.38] µs |
| window_render/buffer_lines/10000 | 55.92 µs | 55.90 µs | 0.50 µs | [55.77, 56.11] µs |
| with_highlights/no_highlights | 55.11 µs | 55.09 µs | 0.13 µs | [55.08, 55.17] µs |
