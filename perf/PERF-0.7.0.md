# Performance Benchmarks v0.7.0

> Last updated: 2025-12-25T14:48:22Z | Commit: 63f3e72 | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.7.0"
commit = "63f3e72"
date = "2025-12-25T14:48:22Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| complete_cycle | 1 | 85.42 µs |
| frame_operations | 3 | 69.02 µs |
| large_file | 2 | 88.82 ms |
| render_data_changelog | 5 | 163.01 µs |
| render_data_jjjjj | 2 | 268.43 µs |
| render_data_real | 4 | 27.83 µs |
| render_data_syntax_overhead | 2 | 26.22 µs |
| throughput | 1 | 56.06 µs |
| viewport_size | 4 | 105.26 µs |
| window_render | 4 | 44.72 µs |
| with_highlights | 1 | 55.92 µs |

## Results

```toml
[results.complete_cycle]
full_scroll_cycle = { mean = 85.42, median = 85.43, std_dev = 0.22, unit = "µs" }

[results.frame_operations]
buffer_fill_6000_cells = { mean = 66.20, median = 66.18, std_dev = 0.07, unit = "µs" }
flush_full_screen = { mean = 103.05, median = 102.98, std_dev = 0.20, unit = "µs" }
flush_scroll_simulation = { mean = 37.81, median = 37.80, std_dev = 0.04, unit = "µs" }

[results.large_file]
5000_lines_20_j_presses = { mean = 3.34, median = 3.34, std_dev = 0.00, unit = "ms" }
5000_lines_render_data = { mean = 174.30, median = 174.53, std_dev = 0.60, unit = "µs" }

[results.render_data_changelog]
scroll_pos_0 = { mean = 162.51, median = 162.72, std_dev = 0.80, unit = "µs" }
scroll_pos_100 = { mean = 162.92, median = 162.98, std_dev = 0.81, unit = "µs" }
scroll_pos_1000 = { mean = 162.83, median = 162.95, std_dev = 0.81, unit = "µs" }
scroll_pos_1500 = { mean = 162.96, median = 162.80, std_dev = 2.20, unit = "µs" }
scroll_pos_500 = { mean = 163.85, median = 163.48, std_dev = 1.23, unit = "µs" }

[results.render_data_jjjjj]
20_j_presses = { mean = 535.53, median = 534.80, std_dev = 2.95, unit = "µs" }
50_j_presses = { mean = 1.33, median = 1.33, std_dev = 0.00, unit = "ms" }

[results.render_data_real]
scroll_pos_0 = { mean = 27.74, median = 27.81, std_dev = 0.12, unit = "µs" }
scroll_pos_150 = { mean = 28.04, median = 27.89, std_dev = 0.53, unit = "µs" }
scroll_pos_300 = { mean = 27.59, median = 27.58, std_dev = 0.30, unit = "µs" }
scroll_pos_50 = { mean = 27.93, median = 27.98, std_dev = 0.11, unit = "µs" }

[results.render_data_syntax_overhead]
with_syntax = { mean = 26.23, median = 26.21, std_dev = 0.06, unit = "µs" }
without_syntax = { mean = 26.22, median = 26.20, std_dev = 0.09, unit = "µs" }

[results.throughput]
renders_per_second = { mean = 56.06, median = 56.02, std_dev = 0.13, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 112.57, median = 112.38, std_dev = 1.04, unit = "µs" }
height_200 = { mean = 224.41, median = 223.98, std_dev = 4.10, unit = "µs" }
height_24 = { mean = 26.76, median = 26.77, std_dev = 0.02, unit = "µs" }
height_50 = { mean = 57.30, median = 57.15, std_dev = 0.38, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 10.53, median = 10.53, std_dev = 0.02, unit = "µs" }
buffer_lines_100 = { mean = 55.81, median = 55.78, std_dev = 0.11, unit = "µs" }
buffer_lines_1000 = { mean = 56.39, median = 56.07, std_dev = 1.73, unit = "µs" }
buffer_lines_10000 = { mean = 56.14, median = 56.03, std_dev = 0.56, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 55.92, median = 55.87, std_dev = 0.15, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| complete_cycle/full_scroll_cycle | 85.42 µs | 85.43 µs | 0.22 µs | [85.35, 85.51] µs |
| frame_operations/buffer_fill_6000_cells | 66.20 µs | 66.18 µs | 0.07 µs | [66.18, 66.22] µs |
| frame_operations/flush_full_screen | 103.05 µs | 102.98 µs | 0.20 µs | [102.99, 103.13] µs |
| frame_operations/flush_scroll_simulation | 37.81 µs | 37.80 µs | 0.04 µs | [37.80, 37.83] µs |
| large_file/5000_lines_20_j_presses | 3.34 ms | 3.34 ms | 0.00 ms | [3.33, 3.34] ms |
| large_file/5000_lines_render_data | 174.30 µs | 174.53 µs | 0.60 µs | [174.09, 174.50] µs |
| render_data_changelog/scroll_pos/0 | 162.51 µs | 162.72 µs | 0.80 µs | [162.24, 162.80] µs |
| render_data_changelog/scroll_pos/100 | 162.92 µs | 162.98 µs | 0.81 µs | [162.65, 163.22] µs |
| render_data_changelog/scroll_pos/1000 | 162.83 µs | 162.95 µs | 0.81 µs | [162.58, 163.15] µs |
| render_data_changelog/scroll_pos/1500 | 162.96 µs | 162.80 µs | 2.20 µs | [162.39, 163.84] µs |
| render_data_changelog/scroll_pos/500 | 163.85 µs | 163.48 µs | 1.23 µs | [163.46, 164.33] µs |
| render_data_jjjjj/20_j_presses | 535.53 µs | 534.80 µs | 2.95 µs | [534.56, 536.64] µs |
| render_data_jjjjj/50_j_presses | 1.33 ms | 1.33 ms | 0.00 ms | [1.33, 1.33] ms |
| render_data_real/scroll_pos/0 | 27.74 µs | 27.81 µs | 0.12 µs | [27.70, 27.78] µs |
| render_data_real/scroll_pos/150 | 28.04 µs | 27.89 µs | 0.53 µs | [27.89, 28.25] µs |
| render_data_real/scroll_pos/300 | 27.59 µs | 27.58 µs | 0.30 µs | [27.50, 27.71] µs |
| render_data_real/scroll_pos/50 | 27.93 µs | 27.98 µs | 0.11 µs | [27.89, 27.97] µs |
| render_data_syntax_overhead/with_syntax | 26.23 µs | 26.21 µs | 0.06 µs | [26.21, 26.25] µs |
| render_data_syntax_overhead/without_syntax | 26.22 µs | 26.20 µs | 0.09 µs | [26.19, 26.26] µs |
| throughput/renders_per_second | 56.06 µs | 56.02 µs | 0.13 µs | [56.02, 56.11] µs |
| viewport_size/height/100 | 112.57 µs | 112.38 µs | 1.04 µs | [112.24, 112.96] µs |
| viewport_size/height/200 | 224.41 µs | 223.98 µs | 4.10 µs | [223.12, 225.98] µs |
| viewport_size/height/24 | 26.76 µs | 26.77 µs | 0.02 µs | [26.76, 26.77] µs |
| viewport_size/height/50 | 57.30 µs | 57.15 µs | 0.38 µs | [57.18, 57.44] µs |
| window_render/buffer_lines/10 | 10.53 µs | 10.53 µs | 0.02 µs | [10.53, 10.54] µs |
| window_render/buffer_lines/100 | 55.81 µs | 55.78 µs | 0.11 µs | [55.78, 55.86] µs |
| window_render/buffer_lines/1000 | 56.39 µs | 56.07 µs | 1.73 µs | [56.07, 57.03] µs |
| window_render/buffer_lines/10000 | 56.14 µs | 56.03 µs | 0.56 µs | [56.03, 56.35] µs |
| with_highlights/no_highlights | 55.92 µs | 55.87 µs | 0.15 µs | [55.87, 55.98] µs |
