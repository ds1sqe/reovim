# Performance Benchmarks v0.6.0

> Last updated: 2025-12-18T17:27:18Z | Commit: e761a79 | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.6.0"
commit = "e761a79"
date = "2025-12-18T17:27:18Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 201.03 µs |
| buffer_vec | 4 | 201.26 µs |
| file_io | 2 | 143.74 µs |
| file_io_viewport | 3 | 79.16 µs |
| input_completion | 2 | 66.70 µs |
| input_mode_switch | 1 | 186.77 µs |
| input_scrolling | 3 | 527.05 ms |
| input_sustained | 1 | 7.65 ms |
| input_typing | 2 | 72.72 µs |
| io_bytes | 1 | 62.41 µs |
| rtt_explorer | 3 | 83.02 µs |
| rtt_input_lag | 2 | 107.28 µs |
| rtt_movement_lag | 5 | 536.55 µs |
| screen_io | 3 | 62.06 µs |
| screen_viewport_io | 3 | 71.57 µs |
| stress_completion | 1 | 1.47 ms |
| stress_editing | 3 | 31.93 ms |
| stress_mode_ops | 1 | 4.43 ms |
| stress_scroll | 3 | 18.23 ms |
| stress_worst_case | 1 | 38.69 ms |
| throughput | 1 | 54.48 µs |
| viewport_size | 4 | 101.71 µs |
| window_render | 4 | 43.87 µs |
| with_highlights | 1 | 54.54 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 5.97, median = 5.97, std_dev = 0.01, unit = "µs" }
clone_1000 = { mean = 71.54, median = 71.53, std_dev = 0.15, unit = "µs" }
clone_10000 = { mean = 722.83, median = 722.66, std_dev = 1.37, unit = "µs" }
clone_50000 = { mean = 3.76, median = 3.75, std_dev = 0.04, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 5.90, median = 5.90, std_dev = 0.01, unit = "µs" }
vec_clone_1000 = { mean = 71.72, median = 71.73, std_dev = 0.09, unit = "µs" }
vec_clone_10000 = { mean = 723.70, median = 723.19, std_dev = 1.70, unit = "µs" }
vec_clone_50000 = { mean = 3.74, median = 3.74, std_dev = 0.01, unit = "ms" }

[results.file_io]
buffered_file = { mean = 71.02, median = 70.94, std_dev = 0.44, unit = "µs" }
unbuffered_file = { mean = 216.46, median = 216.56, std_dev = 0.88, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 129.23, median = 129.15, std_dev = 0.29, unit = "µs" }
viewport_24 = { mean = 38.98, median = 38.97, std_dev = 0.20, unit = "µs" }
viewport_50 = { mean = 69.26, median = 69.28, std_dev = 0.32, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 71.88, median = 71.79, std_dev = 0.37, unit = "µs" }
without_completion_popup = { mean = 61.52, median = 61.31, std_dev = 0.69, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 186.77, median = 186.67, std_dev = 0.53, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 7.83, median = 7.83, std_dev = 0.01, unit = "ms" }
scroll_half_page = { mean = 786.18, median = 785.34, std_dev = 2.58, unit = "µs" }
scroll_one_line = { mean = 787.13, median = 786.92, std_dev = 1.07, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 7.65, median = 7.64, std_dev = 0.06, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 73.36, median = 73.33, std_dev = 0.10, unit = "µs" }
single_char_render = { mean = 72.09, median = 71.97, std_dev = 0.43, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 62.41, median = 62.39, std_dev = 0.06, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 62.29, median = 62.28, std_dev = 0.15, unit = "µs" }
open_explorer = { mean = 63.23, median = 63.48, std_dev = 0.66, unit = "µs" }
toggle_cycle = { mean = 123.55, median = 123.47, std_dev = 0.41, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 106.38, median = 106.33, std_dev = 0.60, unit = "µs" }
char_insert_rtt = { mean = 108.17, median = 108.17, std_dev = 0.46, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 800.61, median = 800.32, std_dev = 1.42, unit = "µs" }
half_page_down_rtt = { mean = 803.91, median = 803.46, std_dev = 2.62, unit = "µs" }
move_down_rtt = { mean = 806.35, median = 804.88, std_dev = 6.10, unit = "µs" }
move_right_rtt = { mean = 135.97, median = 135.85, std_dev = 0.35, unit = "µs" }
word_forward_rtt = { mean = 135.91, median = 135.87, std_dev = 0.33, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 61.49, median = 61.48, std_dev = 0.06, unit = "µs" }
full_render_1000 = { mean = 61.99, median = 62.01, std_dev = 0.16, unit = "µs" }
full_render_10000 = { mean = 62.70, median = 62.66, std_dev = 0.15, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 120.64, median = 120.63, std_dev = 0.13, unit = "µs" }
viewport_height_24 = { mean = 32.36, median = 32.36, std_dev = 0.06, unit = "µs" }
viewport_height_50 = { mean = 61.71, median = 61.67, std_dev = 0.20, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 1.47, median = 1.47, std_dev = 0.00, unit = "ms" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 16.17, median = 16.16, std_dev = 0.03, unit = "ms" }
edit_navigate_cycle_1k = { mean = 2.94, median = 2.93, std_dev = 0.01, unit = "ms" }
edit_navigate_cycle_50k = { mean = 76.70, median = 76.68, std_dev = 0.18, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 4.43, median = 4.42, std_dev = 0.05, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 40.20, median = 40.21, std_dev = 0.07, unit = "ms" }
jump_around_file = { mean = 6.45, median = 6.45, std_dev = 0.01, unit = "ms" }
spam_ctrl_d_10_times = { mean = 8.03, median = 8.02, std_dev = 0.02, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 38.69, median = 38.66, std_dev = 0.13, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 54.48, median = 54.47, std_dev = 0.08, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 110.80, median = 110.33, std_dev = 2.35, unit = "µs" }
height_200 = { mean = 214.59, median = 214.55, std_dev = 0.34, unit = "µs" }
height_24 = { mean = 26.55, median = 26.53, std_dev = 0.07, unit = "µs" }
height_50 = { mean = 54.90, median = 54.90, std_dev = 0.07, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 10.15, median = 10.14, std_dev = 0.02, unit = "µs" }
buffer_lines_100 = { mean = 54.39, median = 54.37, std_dev = 0.08, unit = "µs" }
buffer_lines_1000 = { mean = 54.86, median = 54.57, std_dev = 1.53, unit = "µs" }
buffer_lines_10000 = { mean = 56.08, median = 56.43, std_dev = 0.79, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 54.54, median = 54.51, std_dev = 0.21, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 5.97 µs | 5.97 µs | 0.01 µs | [5.97, 5.97] µs |
| buffer_clone/clone/1000 | 71.54 µs | 71.53 µs | 0.15 µs | [71.49, 71.60] µs |
| buffer_clone/clone/10000 | 722.83 µs | 722.66 µs | 1.37 µs | [722.40, 723.37] µs |
| buffer_clone/clone/50000 | 3.76 ms | 3.75 ms | 0.04 ms | [3.75, 3.78] ms |
| buffer_vec/vec_clone/100 | 5.90 µs | 5.90 µs | 0.01 µs | [5.90, 5.91] µs |
| buffer_vec/vec_clone/1000 | 71.72 µs | 71.73 µs | 0.09 µs | [71.69, 71.75] µs |
| buffer_vec/vec_clone/10000 | 723.70 µs | 723.19 µs | 1.70 µs | [723.16, 724.35] µs |
| buffer_vec/vec_clone/50000 | 3.74 ms | 3.74 ms | 0.01 ms | [3.74, 3.74] ms |
| file_io/buffered_file | 71.02 µs | 70.94 µs | 0.44 µs | [70.87, 71.19] µs |
| file_io/unbuffered_file | 216.46 µs | 216.56 µs | 0.88 µs | [216.16, 216.77] µs |
| file_io_viewport/viewport/100 | 129.23 µs | 129.15 µs | 0.29 µs | [129.14, 129.34] µs |
| file_io_viewport/viewport/24 | 38.98 µs | 38.97 µs | 0.20 µs | [38.91, 39.05] µs |
| file_io_viewport/viewport/50 | 69.26 µs | 69.28 µs | 0.32 µs | [69.14, 69.37] µs |
| input_completion/with_completion_popup | 71.88 µs | 71.79 µs | 0.37 µs | [71.77, 72.03] µs |
| input_completion/without_completion_popup | 61.52 µs | 61.31 µs | 0.69 µs | [61.33, 61.80] µs |
| input_mode_switch/normal_insert_normal | 186.77 µs | 186.67 µs | 0.53 µs | [186.63, 186.99] µs |
| input_scrolling/scroll_10_lines | 7.83 ms | 7.83 ms | 0.01 ms | [7.83, 7.84] ms |
| input_scrolling/scroll_half_page | 786.18 µs | 785.34 µs | 2.58 µs | [785.37, 787.19] µs |
| input_scrolling/scroll_one_line | 787.13 µs | 786.92 µs | 1.07 µs | [786.77, 787.52] µs |
| input_sustained/100_keystrokes_each_rendered | 7.65 ms | 7.64 ms | 0.06 ms | [7.64, 7.67] ms |
| input_typing/burst_10_chars_render | 73.36 µs | 73.33 µs | 0.10 µs | [73.33, 73.40] µs |
| input_typing/single_char_render | 72.09 µs | 71.97 µs | 0.43 µs | [71.97, 72.26] µs |
| io_bytes/full_screen_render | 62.41 µs | 62.39 µs | 0.06 µs | [62.39, 62.43] µs |
| rtt_explorer/close_explorer | 62.29 µs | 62.28 µs | 0.15 µs | [62.23, 62.34] µs |
| rtt_explorer/open_explorer | 63.23 µs | 63.48 µs | 0.66 µs | [62.99, 63.45] µs |
| rtt_explorer/toggle_cycle | 123.55 µs | 123.47 µs | 0.41 µs | [123.43, 123.72] µs |
| rtt_input_lag/backspace_rtt | 106.38 µs | 106.33 µs | 0.60 µs | [106.20, 106.62] µs |
| rtt_input_lag/char_insert_rtt | 108.17 µs | 108.17 µs | 0.46 µs | [108.02, 108.34] µs |
| rtt_movement_lag/goto_top_rtt | 800.61 µs | 800.32 µs | 1.42 µs | [800.12, 801.11] µs |
| rtt_movement_lag/half_page_down_rtt | 803.91 µs | 803.46 µs | 2.62 µs | [803.10, 804.93] µs |
| rtt_movement_lag/move_down_rtt | 806.35 µs | 804.88 µs | 6.10 µs | [804.77, 808.82] µs |
| rtt_movement_lag/move_right_rtt | 135.97 µs | 135.85 µs | 0.35 µs | [135.85, 136.09] µs |
| rtt_movement_lag/word_forward_rtt | 135.91 µs | 135.87 µs | 0.33 µs | [135.79, 136.03] µs |
| screen_io/full_render/100 | 61.49 µs | 61.48 µs | 0.06 µs | [61.47, 61.51] µs |
| screen_io/full_render/1000 | 61.99 µs | 62.01 µs | 0.16 µs | [61.93, 62.04] µs |
| screen_io/full_render/10000 | 62.70 µs | 62.66 µs | 0.15 µs | [62.65, 62.76] µs |
| screen_viewport_io/viewport_height/100 | 120.64 µs | 120.63 µs | 0.13 µs | [120.59, 120.69] µs |
| screen_viewport_io/viewport_height/24 | 32.36 µs | 32.36 µs | 0.06 µs | [32.34, 32.38] µs |
| screen_viewport_io/viewport_height/50 | 61.71 µs | 61.67 µs | 0.20 µs | [61.66, 61.79] µs |
| stress_completion/completion_scroll_100_items | 1.47 ms | 1.47 ms | 0.00 ms | [1.47, 1.47] ms |
| stress_editing/edit_navigate_cycle_10k | 16.17 ms | 16.16 ms | 0.03 ms | [16.16, 16.18] ms |
| stress_editing/edit_navigate_cycle_1k | 2.94 ms | 2.93 ms | 0.01 ms | [2.93, 2.94] ms |
| stress_editing/edit_navigate_cycle_50k | 76.70 ms | 76.68 ms | 0.18 ms | [76.64, 76.76] ms |
| stress_mode_ops/insert_escape_move_cycle | 4.43 ms | 4.42 ms | 0.05 ms | [4.42, 4.44] ms |
| stress_scroll/hold_j_50_lines_10k_file | 40.20 ms | 40.21 ms | 0.07 ms | [40.17, 40.22] ms |
| stress_scroll/jump_around_file | 6.45 ms | 6.45 ms | 0.01 ms | [6.45, 6.46] ms |
| stress_scroll/spam_ctrl_d_10_times | 8.03 ms | 8.02 ms | 0.02 ms | [8.02, 8.04] ms |
| stress_worst_case/all_features_50k_file | 38.69 ms | 38.66 ms | 0.13 ms | [38.64, 38.75] ms |
| throughput/renders_per_second | 54.48 µs | 54.47 µs | 0.08 µs | [54.45, 54.51] µs |
| viewport_size/height/100 | 110.80 µs | 110.33 µs | 2.35 µs | [110.32, 111.70] µs |
| viewport_size/height/200 | 214.59 µs | 214.55 µs | 0.34 µs | [214.50, 214.73] µs |
| viewport_size/height/24 | 26.55 µs | 26.53 µs | 0.07 µs | [26.53, 26.58] µs |
| viewport_size/height/50 | 54.90 µs | 54.90 µs | 0.07 µs | [54.88, 54.93] µs |
| window_render/buffer_lines/10 | 10.15 µs | 10.14 µs | 0.02 µs | [10.14, 10.16] µs |
| window_render/buffer_lines/100 | 54.39 µs | 54.37 µs | 0.08 µs | [54.37, 54.42] µs |
| window_render/buffer_lines/1000 | 54.86 µs | 54.57 µs | 1.53 µs | [54.57, 55.44] µs |
| window_render/buffer_lines/10000 | 56.08 µs | 56.43 µs | 0.79 µs | [55.80, 56.35] µs |
| with_highlights/no_highlights | 54.54 µs | 54.51 µs | 0.21 µs | [54.49, 54.63] µs |
