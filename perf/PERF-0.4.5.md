# Performance Benchmarks v0.4.5

> Last updated: 2025-12-17T15:32:00Z | Commit: ee81b5b | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.4.5"
commit = "ee81b5b"
date = "2025-12-17T15:32:00Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.5-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| file_io | 2 | 66.54 µs |
| file_io_viewport | 3 | 12.03 µs |
| input_completion | 2 | 13.15 µs |
| input_mode_switch | 1 | 41.93 µs |
| input_scrolling | 3 | 427.12 ms |
| input_sustained | 1 | 1.50 ms |
| input_typing | 2 | 12.08 µs |
| io_bytes | 1 | 9.04 µs |
| rtt_explorer | 2 | 11.52 µs |
| screen_io | 3 | 10.42 µs |
| screen_viewport_io | 3 | 8.14 µs |
| stress_completion | 1 | 352.88 µs |
| stress_scroll | 3 | 10.41 ms |
| throughput | 1 | 4.08 µs |
| viewport_size | 4 | 14.27 µs |
| window_render | 4 | 2.81 µs |
| with_highlights | 1 | 10.86 µs |

## Results

```toml
[results.file_io]
buffered_file = { mean = 10.61, median = 10.60, std_dev = 0.61, unit = "µs" }
unbuffered_file = { mean = 122.47, median = 122.67, std_dev = 3.65, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 17.91, median = 18.02, std_dev = 0.77, unit = "µs" }
viewport_24 = { mean = 7.67, median = 7.66, std_dev = 0.50, unit = "µs" }
viewport_50 = { mean = 10.52, median = 10.46, std_dev = 0.55, unit = "µs" }

[results.input_completion]
with_completion_popup = { mean = 17.41, median = 16.91, std_dev = 1.01, unit = "µs" }
without_completion_popup = { mean = 8.90, median = 8.97, std_dev = 0.66, unit = "µs" }

[results.input_mode_switch]
normal_insert_normal = { mean = 41.93, median = 41.07, std_dev = 6.00, unit = "µs" }

[results.input_scrolling]
scroll_10_lines = { mean = 7.56, median = 7.33, std_dev = 1.24, unit = "ms" }
scroll_half_page = { mean = 571.38, median = 581.47, std_dev = 50.10, unit = "µs" }
scroll_one_line = { mean = 702.40, median = 725.06, std_dev = 40.60, unit = "µs" }

[results.input_sustained]
100_keystrokes_each_rendered = { mean = 1.50, median = 1.34, std_dev = 0.30, unit = "ms" }

[results.input_typing]
burst_10_chars_render = { mean = 14.51, median = 12.79, std_dev = 3.37, unit = "µs" }
single_char_render = { mean = 9.65, median = 9.57, std_dev = 0.43, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 9.04, median = 9.35, std_dev = 0.49, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 10.34, median = 10.33, std_dev = 0.62, unit = "µs" }
open_explorer = { mean = 12.71, median = 11.86, std_dev = 2.64, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 11.17, median = 10.92, std_dev = 3.13, unit = "µs" }
full_render_1000 = { mean = 9.35, median = 9.37, std_dev = 0.53, unit = "µs" }
full_render_10000 = { mean = 10.73, median = 10.91, std_dev = 0.42, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 13.82, median = 14.72, std_dev = 1.92, unit = "µs" }
viewport_height_24 = { mean = 4.38, median = 4.80, std_dev = 0.70, unit = "µs" }
viewport_height_50 = { mean = 6.22, median = 6.25, std_dev = 0.14, unit = "µs" }

[results.stress_completion]
completion_scroll_100_items = { mean = 352.88, median = 347.56, std_dev = 15.74, unit = "µs" }

[results.stress_scroll]
hold_j_50_lines_10k_file = { mean = 22.51, median = 22.27, std_dev = 3.34, unit = "ms" }
jump_around_file = { mean = 3.95, median = 3.96, std_dev = 0.48, unit = "ms" }
spam_ctrl_d_10_times = { mean = 4.77, median = 4.95, std_dev = 0.73, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 4.08, median = 4.10, std_dev = 0.35, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 18.71, median = 19.27, std_dev = 3.39, unit = "µs" }
height_200 = { mean = 28.53, median = 28.19, std_dev = 7.84, unit = "µs" }
height_24 = { mean = 1.91, median = 2.03, std_dev = 0.23, unit = "µs" }
height_50 = { mean = 7.93, median = 6.16, std_dev = 3.20, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 1.01, median = 0.96, std_dev = 0.11, unit = "µs" }
buffer_lines_100 = { mean = 2.83, median = 2.84, std_dev = 0.11, unit = "µs" }
buffer_lines_1000 = { mean = 4.02, median = 4.08, std_dev = 0.16, unit = "µs" }
buffer_lines_10000 = { mean = 3.38, median = 3.63, std_dev = 0.45, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 10.86, median = 10.61, std_dev = 1.01, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| file_io/buffered_file | 10.61 µs | 10.60 µs | 0.61 µs | [10.49, 10.73] µs |
| file_io/unbuffered_file | 122.47 µs | 122.67 µs | 3.65 µs | [121.76, 123.18] µs |
| file_io_viewport/viewport/100 | 17.91 µs | 18.02 µs | 0.77 µs | [17.76, 18.06] µs |
| file_io_viewport/viewport/24 | 7.67 µs | 7.66 µs | 0.50 µs | [7.57, 7.77] µs |
| file_io_viewport/viewport/50 | 10.52 µs | 10.46 µs | 0.55 µs | [10.42, 10.63] µs |
| input_completion/with_completion_popup | 17.41 µs | 16.91 µs | 1.01 µs | [17.21, 17.61] µs |
| input_completion/without_completion_popup | 8.90 µs | 8.97 µs | 0.66 µs | [8.77, 9.02] µs |
| input_mode_switch/normal_insert_normal | 41.93 µs | 41.07 µs | 6.00 µs | [40.79, 43.14] µs |
| input_scrolling/scroll_10_lines | 7.56 ms | 7.33 ms | 1.24 ms | [7.33, 7.81] ms |
| input_scrolling/scroll_half_page | 571.38 µs | 581.47 µs | 50.10 µs | [561.59, 581.17] µs |
| input_scrolling/scroll_one_line | 702.40 µs | 725.06 µs | 40.60 µs | [694.43, 710.22] µs |
| input_sustained/100_keystrokes_each_rendered | 1.50 ms | 1.34 ms | 0.30 ms | [1.42, 1.59] ms |
| input_typing/burst_10_chars_render | 14.51 µs | 12.79 µs | 3.37 µs | [13.87, 15.19] µs |
| input_typing/single_char_render | 9.65 µs | 9.57 µs | 0.43 µs | [9.57, 9.74] µs |
| io_bytes/full_screen_render | 9.04 µs | 9.35 µs | 0.49 µs | [8.95, 9.14] µs |
| rtt_explorer/close_explorer | 10.34 µs | 10.33 µs | 0.62 µs | [10.22, 10.46] µs |
| rtt_explorer/open_explorer | 12.71 µs | 11.86 µs | 2.64 µs | [12.22, 13.24] µs |
| screen_io/full_render/100 | 11.17 µs | 10.92 µs | 3.13 µs | [10.57, 11.79] µs |
| screen_io/full_render/1000 | 9.35 µs | 9.37 µs | 0.53 µs | [9.25, 9.45] µs |
| screen_io/full_render/10000 | 10.73 µs | 10.91 µs | 0.42 µs | [10.65, 10.81] µs |
| screen_viewport_io/viewport_height/100 | 13.82 µs | 14.72 µs | 1.92 µs | [13.44, 14.19] µs |
| screen_viewport_io/viewport_height/24 | 4.38 µs | 4.80 µs | 0.70 µs | [4.24, 4.51] µs |
| screen_viewport_io/viewport_height/50 | 6.22 µs | 6.25 µs | 0.14 µs | [6.19, 6.24] µs |
| stress_completion/completion_scroll_100_items | 352.88 µs | 347.56 µs | 15.74 µs | [347.41, 358.44] µs |
| stress_scroll/hold_j_50_lines_10k_file | 22.51 ms | 22.27 ms | 3.34 ms | [21.33, 23.68] ms |
| stress_scroll/jump_around_file | 3.95 ms | 3.96 ms | 0.48 ms | [3.78, 4.12] ms |
| stress_scroll/spam_ctrl_d_10_times | 4.77 ms | 4.95 ms | 0.73 ms | [4.51, 5.03] ms |
| throughput/renders_per_second | 4.08 µs | 4.10 µs | 0.35 µs | [4.01, 4.14] µs |
| viewport_size/height/100 | 18.71 µs | 19.27 µs | 3.39 µs | [18.04, 19.36] µs |
| viewport_size/height/200 | 28.53 µs | 28.19 µs | 7.84 µs | [27.11, 30.15] µs |
| viewport_size/height/24 | 1.91 µs | 2.03 µs | 0.23 µs | [1.87, 1.95] µs |
| viewport_size/height/50 | 7.93 µs | 6.16 µs | 3.20 µs | [7.32, 8.58] µs |
| window_render/buffer_lines/10 | 1.01 µs | 0.96 µs | 0.11 µs | [0.99, 1.04] µs |
| window_render/buffer_lines/100 | 2.83 µs | 2.84 µs | 0.11 µs | [2.80, 2.85] µs |
| window_render/buffer_lines/1000 | 4.02 µs | 4.08 µs | 0.16 µs | [3.99, 4.05] µs |
| window_render/buffer_lines/10000 | 3.38 µs | 3.63 µs | 0.45 µs | [3.29, 3.47] µs |
| with_highlights/no_highlights | 10.86 µs | 10.61 µs | 1.01 µs | [10.67, 11.07] µs |
