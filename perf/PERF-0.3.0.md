# Performance Benchmarks v0.3.0

> Last updated: 2025-12-17T02:38:02Z | Commit: b84e6a2 | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.3.0"
commit = "b84e6a2"
date = "2025-12-17T02:38:02Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.5-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_clone | 4 | 175.14 µs |
| file_io | 2 | 329.05 µs |
| file_io_viewport | 3 | 43.53 µs |
| io_bytes | 1 | 21.54 µs |
| rtt_explorer | 3 | 21.16 µs |
| rtt_input_lag | 2 | 81.76 µs |
| rtt_movement_lag | 5 | 65.58 ms |
| screen_io | 3 | 23.68 µs |
| screen_viewport_io | 3 | 39.09 µs |
| stress_editing | 3 | 44.28 ms |
| throughput | 1 | 8.61 µs |
| viewport_size | 4 | 15.40 µs |
| window_render | 4 | 5.17 µs |
| with_highlights | 1 | 9.19 µs |

## Results

```toml
[results.buffer_clone]
clone_100 = { mean = 3.06, median = 2.97, std_dev = 0.26, unit = "µs" }
clone_1000 = { mean = 45.75, median = 45.90, std_dev = 1.62, unit = "µs" }
clone_10000 = { mean = 647.86, median = 641.45, std_dev = 37.56, unit = "µs" }
clone_50000 = { mean = 3.90, median = 3.70, std_dev = 0.59, unit = "ms" }

[results.file_io]
buffered_file = { mean = 61.56, median = 62.11, std_dev = 4.31, unit = "µs" }
unbuffered_file = { mean = 596.54, median = 637.80, std_dev = 139.05, unit = "µs" }

[results.file_io_viewport]
viewport_100 = { mean = 70.07, median = 62.35, std_dev = 33.29, unit = "µs" }
viewport_24 = { mean = 23.72, median = 24.14, std_dev = 5.71, unit = "µs" }
viewport_50 = { mean = 36.81, median = 39.33, std_dev = 5.35, unit = "µs" }

[results.io_bytes]
full_screen_render = { mean = 21.54, median = 21.50, std_dev = 0.84, unit = "µs" }

[results.rtt_explorer]
close_explorer = { mean = 11.56, median = 11.26, std_dev = 0.67, unit = "µs" }
open_explorer = { mean = 13.78, median = 13.20, std_dev = 2.19, unit = "µs" }
toggle_cycle = { mean = 38.14, median = 38.71, std_dev = 2.45, unit = "µs" }

[results.rtt_input_lag]
backspace_rtt = { mean = 91.51, median = 90.55, std_dev = 6.05, unit = "µs" }
char_insert_rtt = { mean = 72.00, median = 67.59, std_dev = 10.83, unit = "µs" }

[results.rtt_movement_lag]
goto_top_rtt = { mean = 3.61, median = 3.15, std_dev = 1.54, unit = "ms" }
half_page_down_rtt = { mean = 4.32, median = 3.29, std_dev = 1.74, unit = "ms" }
move_down_rtt = { mean = 1.35, median = 1.29, std_dev = 0.14, unit = "ms" }
move_right_rtt = { mean = 151.52, median = 146.17, std_dev = 14.13, unit = "µs" }
word_forward_rtt = { mean = 167.09, median = 143.82, std_dev = 53.50, unit = "µs" }

[results.screen_io]
full_render_100 = { mean = 17.55, median = 17.02, std_dev = 1.38, unit = "µs" }
full_render_1000 = { mean = 25.82, median = 24.81, std_dev = 2.95, unit = "µs" }
full_render_10000 = { mean = 27.68, median = 27.93, std_dev = 2.72, unit = "µs" }

[results.screen_viewport_io]
viewport_height_100 = { mean = 62.84, median = 59.93, std_dev = 4.30, unit = "µs" }
viewport_height_24 = { mean = 21.32, median = 22.11, std_dev = 1.32, unit = "µs" }
viewport_height_50 = { mean = 33.12, median = 33.25, std_dev = 3.18, unit = "µs" }

[results.stress_editing]
edit_navigate_cycle_10k = { mean = 19.09, median = 17.69, std_dev = 2.72, unit = "ms" }
edit_navigate_cycle_1k = { mean = 2.00, median = 2.12, std_dev = 0.19, unit = "ms" }
edit_navigate_cycle_50k = { mean = 111.77, median = 116.67, std_dev = 15.02, unit = "ms" }

[results.throughput]
renders_per_second = { mean = 8.61, median = 8.39, std_dev = 1.14, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 16.51, median = 16.82, std_dev = 0.88, unit = "µs" }
height_200 = { mean = 33.41, median = 32.83, std_dev = 2.81, unit = "µs" }
height_24 = { mean = 3.76, median = 3.85, std_dev = 0.14, unit = "µs" }
height_50 = { mean = 7.91, median = 8.03, std_dev = 0.28, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 1.67, median = 1.79, std_dev = 0.38, unit = "µs" }
buffer_lines_100 = { mean = 6.27, median = 5.74, std_dev = 1.31, unit = "µs" }
buffer_lines_1000 = { mean = 6.16, median = 6.13, std_dev = 0.55, unit = "µs" }
buffer_lines_10000 = { mean = 6.60, median = 6.06, std_dev = 0.91, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 9.19, median = 9.08, std_dev = 0.35, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_clone/clone/100 | 3.06 µs | 2.97 µs | 0.26 µs | [3.01, 3.11] µs |
| buffer_clone/clone/1000 | 45.75 µs | 45.90 µs | 1.62 µs | [45.43, 46.06] µs |
| buffer_clone/clone/10000 | 647.86 µs | 641.45 µs | 37.56 µs | [640.91, 655.58] µs |
| buffer_clone/clone/50000 | 3.90 ms | 3.70 ms | 0.59 ms | [3.79, 4.02] ms |
| file_io/buffered_file | 61.56 µs | 62.11 µs | 4.31 µs | [60.71, 62.40] µs |
| file_io/unbuffered_file | 596.54 µs | 637.80 µs | 139.05 µs | [569.10, 623.52] µs |
| file_io_viewport/viewport/100 | 70.07 µs | 62.35 µs | 33.29 µs | [63.63, 76.58] µs |
| file_io_viewport/viewport/24 | 23.72 µs | 24.14 µs | 5.71 µs | [22.60, 24.83] µs |
| file_io_viewport/viewport/50 | 36.81 µs | 39.33 µs | 5.35 µs | [35.75, 37.83] µs |
| io_bytes/full_screen_render | 21.54 µs | 21.50 µs | 0.84 µs | [21.38, 21.71] µs |
| rtt_explorer/close_explorer | 11.56 µs | 11.26 µs | 0.67 µs | [11.43, 11.69] µs |
| rtt_explorer/open_explorer | 13.78 µs | 13.20 µs | 2.19 µs | [13.35, 14.21] µs |
| rtt_explorer/toggle_cycle | 38.14 µs | 38.71 µs | 2.45 µs | [37.63, 38.58] µs |
| rtt_input_lag/backspace_rtt | 91.51 µs | 90.55 µs | 6.05 µs | [90.35, 92.71] µs |
| rtt_input_lag/char_insert_rtt | 72.00 µs | 67.59 µs | 10.83 µs | [69.94, 74.14] µs |
| rtt_movement_lag/goto_top_rtt | 3.61 ms | 3.15 ms | 1.54 ms | [3.32, 3.92] ms |
| rtt_movement_lag/half_page_down_rtt | 4.32 ms | 3.29 ms | 1.74 ms | [4.00, 4.68] ms |
| rtt_movement_lag/move_down_rtt | 1.35 ms | 1.29 ms | 0.14 ms | [1.32, 1.37] ms |
| rtt_movement_lag/move_right_rtt | 151.52 µs | 146.17 µs | 14.13 µs | [148.82, 154.36] µs |
| rtt_movement_lag/word_forward_rtt | 167.09 µs | 143.82 µs | 53.50 µs | [157.39, 178.23] µs |
| screen_io/full_render/100 | 17.55 µs | 17.02 µs | 1.38 µs | [17.30, 17.83] µs |
| screen_io/full_render/1000 | 25.82 µs | 24.81 µs | 2.95 µs | [25.25, 26.40] µs |
| screen_io/full_render/10000 | 27.68 µs | 27.93 µs | 2.72 µs | [27.15, 28.21] µs |
| screen_viewport_io/viewport_height/100 | 62.84 µs | 59.93 µs | 4.30 µs | [62.01, 63.69] µs |
| screen_viewport_io/viewport_height/24 | 21.32 µs | 22.11 µs | 1.32 µs | [21.06, 21.58] µs |
| screen_viewport_io/viewport_height/50 | 33.12 µs | 33.25 µs | 3.18 µs | [32.50, 33.74] µs |
| stress_editing/edit_navigate_cycle_10k | 19.09 ms | 17.69 ms | 2.72 ms | [18.17, 20.07] ms |
| stress_editing/edit_navigate_cycle_1k | 2.00 ms | 2.12 ms | 0.19 ms | [1.93, 2.06] ms |
| stress_editing/edit_navigate_cycle_50k | 111.77 ms | 116.67 ms | 15.02 ms | [106.46, 116.96] ms |
| throughput/renders_per_second | 8.61 µs | 8.39 µs | 1.14 µs | [8.39, 8.83] µs |
| viewport_size/height/100 | 16.51 µs | 16.82 µs | 0.88 µs | [16.33, 16.68] µs |
| viewport_size/height/200 | 33.41 µs | 32.83 µs | 2.81 µs | [32.91, 34.00] µs |
| viewport_size/height/24 | 3.76 µs | 3.85 µs | 0.14 µs | [3.73, 3.79] µs |
| viewport_size/height/50 | 7.91 µs | 8.03 µs | 0.28 µs | [7.86, 7.97] µs |
| window_render/buffer_lines/10 | 1.67 µs | 1.79 µs | 0.38 µs | [1.59, 1.74] µs |
| window_render/buffer_lines/100 | 6.27 µs | 5.74 µs | 1.31 µs | [6.02, 6.53] µs |
| window_render/buffer_lines/1000 | 6.16 µs | 6.13 µs | 0.55 µs | [6.05, 6.27] µs |
| window_render/buffer_lines/10000 | 6.60 µs | 6.06 µs | 0.91 µs | [6.43, 6.78] µs |
| with_highlights/no_highlights | 9.19 µs | 9.08 µs | 0.35 µs | [9.13, 9.26] µs |
