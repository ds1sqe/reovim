# Performance Benchmarks v0.6.4

> Last updated: 2025-12-19T10:40:26Z | Commit: b3b45df | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.6.4"
commit = "b3b45df"
date = "2025-12-19T10:40:26Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 201.42 µs |
| buffer_vec | 4 | 201.37 µs |
| file_io | 2 | 144.03 µs |
| file_io_viewport | 3 | 79.37 µs |
| input_completion | 2 | 66.88 µs |
| input_mode_switch | 1 | 188.39 µs |
| input_scrolling | 3 | 531.07 ms |
| input_sustained | 1 | 7.68 ms |
| input_typing | 2 | 70.98 µs |
| io_bytes | 1 | 62.01 µs |
| rtt_explorer | 3 | 83.51 µs |
| rtt_input_lag | 2 | 108.14 µs |
| rtt_movement_lag | 5 | 536.02 µs |
| screen_io | 3 | 62.56 µs |
| screen_viewport_io | 3 | 70.98 µs |
| stress_completion | 1 | 1.48 ms |
| stress_editing | 3 | 31.72 ms |
| stress_mode_ops | 1 | 4.40 ms |
| stress_scroll | 3 | 18.11 ms |
| stress_worst_case | 1 | 38.22 ms |
| throughput | 1 | 56.54 µs |
| viewport_size | 4 | 103.40 µs |
| window_render | 4 | 44.07 µs |
| with_highlights | 1 | 56.42 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 5.95, median = 5.95, std_dev = 0.02, unit = "µs" }
clone_1000 = { mean = 71.64, median = 71.63, std_dev = 0.08, unit = "µs" }
clone_10000 = { mean = 724.32, median = 724.04, std_dev = 1.17, unit = "µs" }
clone_50000 = { mean = 3.75, median = 3.75, std_dev = 0.00, unit = "ms" }

[results.buffer_vec]
vec_clone_100 = { mean = 5.90, median = 5.90, std_dev = 0.00, unit = "µs" }
vec_clone_1000 = { mean = 71.85, median = 71.80, std_dev = 0.17, unit = "µs" }
vec_clone_10000 = { mean = 723.89, median = 723.38, std_dev = 1.66, unit = "µs" }
vec_clone_50000 = { mean = 3.85, median = 3.87, std_dev = 0.04, unit = "ms" }

[results.file_io]
buffered_file = { mean = 71.81, median = 70.39, std_dev = 4.80, unit = "µs" }
unbuffered_file = { mean = 216.25, median = 216.19, std_dev = 0.86, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 127.87, median = 127.72, std_dev = 0.72, unit = "µs" }
viewport_24 = { mean = 38.47, median = 38.40, std_dev = 0.28, unit = "µs" }
viewport_50 = { mean = 71.77, median = 71.69, std_dev = 0.37, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 72.11, median = 72.16, std_dev = 0.28, unit = "µs" }
without_completion_popup = { mean = 61.66, median = 61.63, std_dev = 0.11, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 188.39, median = 188.34, std_dev = 0.77, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 7.85, median = 7.83, std_dev = 0.04, unit = "ms" }
scroll_half_page = { mean = 792.02, median = 785.17, std_dev = 35.77, unit = "µs" }
scroll_one_line = { mean = 793.33, median = 794.70, std_dev = 5.67, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 7.68, median = 7.68, std_dev = 0.03, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 71.29, median = 71.27, std_dev = 0.47, unit = "µs" }
single_char_render = { mean = 70.68, median = 70.61, std_dev = 0.47, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 62.01, median = 61.90, std_dev = 0.50, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 61.84, median = 61.83, std_dev = 0.06, unit = "µs" }
open_explorer = { mean = 63.75, median = 63.75, std_dev = 0.30, unit = "µs" }
toggle_cycle = { mean = 124.96, median = 124.95, std_dev = 0.21, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 107.82, median = 107.07, std_dev = 2.21, unit = "µs" }
char_insert_rtt = { mean = 108.47, median = 108.38, std_dev = 0.41, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 800.82, median = 800.26, std_dev = 2.32, unit = "µs" }
half_page_down_rtt = { mean = 802.42, median = 801.69, std_dev = 2.39, unit = "µs" }
move_down_rtt = { mean = 804.17, median = 804.30, std_dev = 2.51, unit = "µs" }
move_right_rtt = { mean = 136.18, median = 136.03, std_dev = 0.64, unit = "µs" }
word_forward_rtt = { mean = 136.50, median = 136.44, std_dev = 0.33, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 63.84, median = 63.76, std_dev = 0.69, unit = "µs" }
full_render_1000 = { mean = 62.06, median = 61.91, std_dev = 0.81, unit = "µs" }
full_render_10000 = { mean = 61.76, median = 61.74, std_dev = 0.10, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 118.38, median = 118.38, std_dev = 0.21, unit = "µs" }
viewport_height_24 = { mean = 31.93, median = 31.91, std_dev = 0.10, unit = "µs" }
viewport_height_50 = { mean = 62.62, median = 62.60, std_dev = 0.19, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 1.48, median = 1.48, std_dev = 0.00, unit = "ms" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 16.01, median = 16.03, std_dev = 0.04, unit = "ms" }
edit_navigate_cycle_1k = { mean = 2.92, median = 2.92, std_dev = 0.01, unit = "ms" }
edit_navigate_cycle_50k = { mean = 76.22, median = 76.22, std_dev = 0.10, unit = "ms" }

[results.stress_mode_ops]
insert_escape_move_cycle = { mean = 4.40, median = 4.40, std_dev = 0.01, unit = "ms" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 39.93, median = 39.91, std_dev = 0.22, unit = "ms" }
jump_around_file = { mean = 6.39, median = 6.39, std_dev = 0.01, unit = "ms" }
spam_ctrl_d_10_times = { mean = 8.01, median = 8.00, std_dev = 0.02, unit = "ms" }

[results.stress_worst_case]
all_features_50k_file = { mean = 38.22, median = 38.20, std_dev = 0.09, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 56.54, median = 56.56, std_dev = 0.16, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 110.18, median = 110.15, std_dev = 0.18, unit = "µs" }
height_200 = { mean = 220.85, median = 220.78, std_dev = 0.36, unit = "µs" }
height_24 = { mean = 26.27, median = 26.28, std_dev = 0.10, unit = "µs" }
height_50 = { mean = 56.28, median = 56.28, std_dev = 0.05, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 10.14, median = 10.14, std_dev = 0.01, unit = "µs" }
buffer_lines_100 = { mean = 55.10, median = 54.96, std_dev = 0.62, unit = "µs" }
buffer_lines_1000 = { mean = 55.63, median = 55.35, std_dev = 1.44, unit = "µs" }
buffer_lines_10000 = { mean = 55.41, median = 55.31, std_dev = 0.39, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 56.42, median = 56.40, std_dev = 0.11, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 5.95 µs | 5.95 µs | 0.02 µs | [5.95, 5.96] µs |
| buffer_clone/clone/1000 | 71.64 µs | 71.63 µs | 0.08 µs | [71.61, 71.67] µs |
| buffer_clone/clone/10000 | 724.32 µs | 724.04 µs | 1.17 µs | [723.94, 724.75] µs |
| buffer_clone/clone/50000 | 3.75 ms | 3.75 ms | 0.00 ms | [3.75, 3.76] ms |
| buffer_vec/vec_clone/100 | 5.90 µs | 5.90 µs | 0.00 µs | [5.89, 5.90] µs |
| buffer_vec/vec_clone/1000 | 71.85 µs | 71.80 µs | 0.17 µs | [71.80, 71.92] µs |
| buffer_vec/vec_clone/10000 | 723.89 µs | 723.38 µs | 1.66 µs | [723.37, 724.54] µs |
| buffer_vec/vec_clone/50000 | 3.85 ms | 3.87 ms | 0.04 ms | [3.84, 3.86] ms |
| file_io/buffered_file | 71.81 µs | 70.39 µs | 4.80 µs | [70.40, 73.70] µs |
| file_io/unbuffered_file | 216.25 µs | 216.19 µs | 0.86 µs | [215.95, 216.56] µs |
| file_io_viewport/viewport/100 | 127.87 µs | 127.72 µs | 0.72 µs | [127.62, 128.12] µs |
| file_io_viewport/viewport/24 | 38.47 µs | 38.40 µs | 0.28 µs | [38.37, 38.57] µs |
| file_io_viewport/viewport/50 | 71.77 µs | 71.69 µs | 0.37 µs | [71.64, 71.91] µs |
| input_completion/with_completion_popup | 72.11 µs | 72.16 µs | 0.28 µs | [72.01, 72.20] µs |
| input_completion/without_completion_popup | 61.66 µs | 61.63 µs | 0.11 µs | [61.62, 61.70] µs |
| input_mode_switch/normal_insert_normal | 188.39 µs | 188.34 µs | 0.77 µs | [188.15, 188.69] µs |
| input_scrolling/scroll_10_lines | 7.85 ms | 7.83 ms | 0.04 ms | [7.84, 7.87] ms |
| input_scrolling/scroll_half_page | 792.02 µs | 785.17 µs | 35.77 µs | [785.07, 805.38] µs |
| input_scrolling/scroll_one_line | 793.33 µs | 794.70 µs | 5.67 µs | [791.32, 795.34] µs |
| input_sustained/100_keystrokes_each_rendered | 7.68 ms | 7.68 ms | 0.03 ms | [7.67, 7.69] ms |
| input_typing/burst_10_chars_render | 71.29 µs | 71.27 µs | 0.47 µs | [71.14, 71.47] µs |
| input_typing/single_char_render | 70.68 µs | 70.61 µs | 0.47 µs | [70.53, 70.86] µs |
| io_bytes/full_screen_render | 62.01 µs | 61.90 µs | 0.50 µs | [61.89, 62.21] µs |
| rtt_explorer/close_explorer | 61.84 µs | 61.83 µs | 0.06 µs | [61.82, 61.86] µs |
| rtt_explorer/open_explorer | 63.75 µs | 63.75 µs | 0.30 µs | [63.66, 63.87] µs |
| rtt_explorer/toggle_cycle | 124.96 µs | 124.95 µs | 0.21 µs | [124.89, 125.03] µs |
| rtt_input_lag/backspace_rtt | 107.82 µs | 107.07 µs | 2.21 µs | [107.15, 108.68] µs |
| rtt_input_lag/char_insert_rtt | 108.47 µs | 108.38 µs | 0.41 µs | [108.33, 108.61] µs |
| rtt_movement_lag/goto_top_rtt | 800.82 µs | 800.26 µs | 2.32 µs | [800.01, 801.63] µs |
| rtt_movement_lag/half_page_down_rtt | 802.42 µs | 801.69 µs | 2.39 µs | [801.64, 803.30] µs |
| rtt_movement_lag/move_down_rtt | 804.17 µs | 804.30 µs | 2.51 µs | [803.23, 805.00] µs |
| rtt_movement_lag/move_right_rtt | 136.18 µs | 136.03 µs | 0.64 µs | [135.98, 136.43] µs |
| rtt_movement_lag/word_forward_rtt | 136.50 µs | 136.44 µs | 0.33 µs | [136.38, 136.61] µs |
| screen_io/full_render/100 | 63.84 µs | 63.76 µs | 0.69 µs | [63.67, 64.12] µs |
| screen_io/full_render/1000 | 62.06 µs | 61.91 µs | 0.81 µs | [61.88, 62.38] µs |
| screen_io/full_render/10000 | 61.76 µs | 61.74 µs | 0.10 µs | [61.73, 61.80] µs |
| screen_viewport_io/viewport_height/100 | 118.38 µs | 118.38 µs | 0.21 µs | [118.31, 118.46] µs |
| screen_viewport_io/viewport_height/24 | 31.93 µs | 31.91 µs | 0.10 µs | [31.90, 31.97] µs |
| screen_viewport_io/viewport_height/50 | 62.62 µs | 62.60 µs | 0.19 µs | [62.55, 62.69] µs |
| stress_completion/completion_scroll_100_items | 1.48 ms | 1.48 ms | 0.00 ms | [1.47, 1.48] ms |
| stress_editing/edit_navigate_cycle_10k | 16.01 ms | 16.03 ms | 0.04 ms | [16.00, 16.03] ms |
| stress_editing/edit_navigate_cycle_1k | 2.92 ms | 2.92 ms | 0.01 ms | [2.92, 2.93] ms |
| stress_editing/edit_navigate_cycle_50k | 76.22 ms | 76.22 ms | 0.10 ms | [76.19, 76.26] ms |
| stress_mode_ops/insert_escape_move_cycle | 4.40 ms | 4.40 ms | 0.01 ms | [4.40, 4.40] ms |
| stress_scroll/hold_j_50_lines_10k_file | 39.93 ms | 39.91 ms | 0.22 ms | [39.85, 40.01] ms |
| stress_scroll/jump_around_file | 6.39 ms | 6.39 ms | 0.01 ms | [6.39, 6.39] ms |
| stress_scroll/spam_ctrl_d_10_times | 8.01 ms | 8.00 ms | 0.02 ms | [8.00, 8.01] ms |
| stress_worst_case/all_features_50k_file | 38.22 ms | 38.20 ms | 0.09 ms | [38.19, 38.26] ms |
| throughput/renders_per_second | 56.54 µs | 56.56 µs | 0.16 µs | [56.48, 56.60] µs |
| viewport_size/height/100 | 110.18 µs | 110.15 µs | 0.18 µs | [110.13, 110.25] µs |
| viewport_size/height/200 | 220.85 µs | 220.78 µs | 0.36 µs | [220.74, 220.98] µs |
| viewport_size/height/24 | 26.27 µs | 26.28 µs | 0.10 µs | [26.24, 26.30] µs |
| viewport_size/height/50 | 56.28 µs | 56.28 µs | 0.05 µs | [56.26, 56.30] µs |
| window_render/buffer_lines/10 | 10.14 µs | 10.14 µs | 0.01 µs | [10.13, 10.14] µs |
| window_render/buffer_lines/100 | 55.10 µs | 54.96 µs | 0.62 µs | [54.96, 55.35] µs |
| window_render/buffer_lines/1000 | 55.63 µs | 55.35 µs | 1.44 µs | [55.34, 56.18] µs |
| window_render/buffer_lines/10000 | 55.41 µs | 55.31 µs | 0.39 µs | [55.32, 55.56] µs |
| with_highlights/no_highlights | 56.42 µs | 56.40 µs | 0.11 µs | [56.38, 56.46] µs |
