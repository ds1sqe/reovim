# Performance Benchmarks v0.7.10

> Last updated: 2026-01-06T11:53:57Z | Commit: 9dd1415 | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.7.10"
commit = "9dd1415"
date = "2026-01-06T11:53:57Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| complete_cycle | 1 | 54.99 µs |
| frame_operations | 3 | 34.52 µs |
| large_file | 2 | 44.69 ms |
| render_data_changelog | 5 | 107.48 µs |
| render_data_jjjjj | 2 | 673.81 µs |
| render_data_real | 4 | 21.38 µs |
| render_data_syntax_overhead | 2 | 20.56 µs |
| throughput | 1 | 25.98 µs |
| viewport_size | 4 | 45.59 µs |
| window_render | 4 | 21.02 µs |
| with_highlights | 1 | 27.24 µs |

## Results

```toml
[results.complete_cycle]
full_scroll_cycle = { mean = 54.99, median = 54.88, std_dev = 0.82, unit = "µs" }

[results.frame_operations]
buffer_fill_6000_cells = { mean = 30.62, median = 30.31, std_dev = 2.44, unit = "µs" }
flush_full_screen = { mean = 51.69, median = 52.93, std_dev = 2.15, unit = "µs" }
flush_scroll_simulation = { mean = 21.25, median = 20.81, std_dev = 1.25, unit = "µs" }

[results.large_file]
5000_lines_20_j_presses = { mean = 1.95, median = 1.95, std_dev = 0.03, unit = "ms" }
5000_lines_render_data = { mean = 87.44, median = 87.35, std_dev = 1.79, unit = "µs" }

[results.render_data_changelog]
scroll_pos_0 = { mean = 101.41, median = 100.22, std_dev = 4.35, unit = "µs" }
scroll_pos_100 = { mean = 100.56, median = 100.25, std_dev = 0.90, unit = "µs" }
scroll_pos_1000 = { mean = 115.49, median = 114.46, std_dev = 4.68, unit = "µs" }
scroll_pos_1500 = { mean = 113.40, median = 110.97, std_dev = 4.87, unit = "µs" }
scroll_pos_500 = { mean = 106.52, median = 102.21, std_dev = 7.76, unit = "µs" }

[results.render_data_jjjjj]
20_j_presses = { mean = 392.36, median = 385.62, std_dev = 17.35, unit = "µs" }
50_j_presses = { mean = 955.27, median = 956.64, std_dev = 13.02, unit = "µs" }

[results.render_data_real]
scroll_pos_0 = { mean = 21.02, median = 21.10, std_dev = 0.71, unit = "µs" }
scroll_pos_150 = { mean = 21.85, median = 21.54, std_dev = 0.75, unit = "µs" }
scroll_pos_300 = { mean = 21.22, median = 21.47, std_dev = 0.87, unit = "µs" }
scroll_pos_50 = { mean = 21.45, median = 21.41, std_dev = 0.30, unit = "µs" }

[results.render_data_syntax_overhead]
with_syntax = { mean = 19.65, median = 19.62, std_dev = 0.59, unit = "µs" }
without_syntax = { mean = 21.47, median = 21.07, std_dev = 1.20, unit = "µs" }

[results.throughput]
renders_per_second = { mean = 25.98, median = 25.91, std_dev = 0.26, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 51.40, median = 50.80, std_dev = 1.71, unit = "µs" }
height_200 = { mean = 91.87, median = 90.09, std_dev = 5.40, unit = "µs" }
height_24 = { mean = 12.43, median = 12.32, std_dev = 0.34, unit = "µs" }
height_50 = { mean = 26.65, median = 26.13, std_dev = 1.20, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 5.29, median = 5.27, std_dev = 0.07, unit = "µs" }
buffer_lines_100 = { mean = 25.73, median = 25.51, std_dev = 0.73, unit = "µs" }
buffer_lines_1000 = { mean = 26.69, median = 26.30, std_dev = 0.95, unit = "µs" }
buffer_lines_10000 = { mean = 26.39, median = 25.98, std_dev = 1.08, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 27.24, median = 26.46, std_dev = 1.56, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| complete_cycle/full_scroll_cycle | 54.99 µs | 54.88 µs | 0.82 µs | [54.70, 55.28] µs |
| frame_operations/buffer_fill_6000_cells | 30.62 µs | 30.31 µs | 2.44 µs | [29.80, 31.51] µs |
| frame_operations/flush_full_screen | 51.69 µs | 52.93 µs | 2.15 µs | [50.92, 52.43] µs |
| frame_operations/flush_scroll_simulation | 21.25 µs | 20.81 µs | 1.25 µs | [20.86, 21.73] µs |
| large_file/5000_lines_20_j_presses | 1.95 ms | 1.95 ms | 0.03 ms | [1.93, 1.96] ms |
| large_file/5000_lines_render_data | 87.44 µs | 87.35 µs | 1.79 µs | [86.81, 88.07] µs |
| render_data_changelog/scroll_pos/0 | 101.41 µs | 100.22 µs | 4.35 µs | [100.07, 103.11] µs |
| render_data_changelog/scroll_pos/100 | 100.56 µs | 100.25 µs | 0.90 µs | [100.25, 100.88] µs |
| render_data_changelog/scroll_pos/1000 | 115.49 µs | 114.46 µs | 4.68 µs | [113.94, 117.23] µs |
| render_data_changelog/scroll_pos/1500 | 113.40 µs | 110.97 µs | 4.87 µs | [111.86, 115.25] µs |
| render_data_changelog/scroll_pos/500 | 106.52 µs | 102.21 µs | 7.76 µs | [103.90, 109.33] µs |
| render_data_jjjjj/20_j_presses | 392.36 µs | 385.62 µs | 17.35 µs | [386.75, 398.87] µs |
| render_data_jjjjj/50_j_presses | 955.27 µs | 956.64 µs | 13.02 µs | [951.02, 960.18] µs |
| render_data_real/scroll_pos/0 | 21.02 µs | 21.10 µs | 0.71 µs | [20.75, 21.25] µs |
| render_data_real/scroll_pos/150 | 21.85 µs | 21.54 µs | 0.75 µs | [21.61, 22.13] µs |
| render_data_real/scroll_pos/300 | 21.22 µs | 21.47 µs | 0.87 µs | [20.89, 21.51] µs |
| render_data_real/scroll_pos/50 | 21.45 µs | 21.41 µs | 0.30 µs | [21.34, 21.55] µs |
| render_data_syntax_overhead/with_syntax | 19.65 µs | 19.62 µs | 0.59 µs | [19.49, 19.89] µs |
| render_data_syntax_overhead/without_syntax | 21.47 µs | 21.07 µs | 1.20 µs | [21.10, 21.93] µs |
| throughput/renders_per_second | 25.98 µs | 25.91 µs | 0.26 µs | [25.90, 26.08] µs |
| viewport_size/height/100 | 51.40 µs | 50.80 µs | 1.71 µs | [50.91, 52.08] µs |
| viewport_size/height/200 | 91.87 µs | 90.09 µs | 5.40 µs | [90.35, 94.03] µs |
| viewport_size/height/24 | 12.43 µs | 12.32 µs | 0.34 µs | [12.33, 12.57] µs |
| viewport_size/height/50 | 26.65 µs | 26.13 µs | 1.20 µs | [26.26, 27.10] µs |
| window_render/buffer_lines/10 | 5.29 µs | 5.27 µs | 0.07 µs | [5.27, 5.32] µs |
| window_render/buffer_lines/100 | 25.73 µs | 25.51 µs | 0.73 µs | [25.54, 26.04] µs |
| window_render/buffer_lines/1000 | 26.69 µs | 26.30 µs | 0.95 µs | [26.38, 27.05] µs |
| window_render/buffer_lines/10000 | 26.39 µs | 25.98 µs | 1.08 µs | [26.05, 26.80] µs |
| with_highlights/no_highlights | 27.24 µs | 26.46 µs | 1.56 µs | [26.73, 27.82] µs |
