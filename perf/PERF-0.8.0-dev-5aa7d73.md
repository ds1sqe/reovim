# Performance Benchmarks v0.8.0-dev-5aa7d73

> Last updated: 2026-01-08T14:44:33Z | Commit: 5aa7d73 | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.8.0-dev-5aa7d73"
commit = "5aa7d73"
date = "2026-01-08T14:44:33Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| complete_cycle | 1 | 56.68 µs |
| event_bus_bottleneck_blocking | 4 | 29.03 µs |
| event_bus_bottleneck_comparison | 2 | 38.20 ns |
| event_bus_bottleneck_queue | 4 | 1.00 ms |
| event_bus_bottleneck_sequential | 3 | 7.67 ms |
| event_bus_dispatch | 7 | 122.28 ns |
| event_bus_dispatch_consumed | 4 | 32.51 ns |
| event_bus_dispatch_emit | 4 | 92.05 ns |
| event_bus_dispatch_multi_type | 4 | 30.46 ns |
| event_bus_dispatch_priority | 3 | 69.37 ns |
| event_bus_dyn_event_new | 1 | 9.17 ns |
| event_bus_rwlock | 4 | 76.75 ns |
| event_bus_subscribe | 1 | 184.83 ns |
| frame_operations | 3 | 33.33 µs |
| large_file | 2 | 47.24 ms |
| render_data_changelog | 3 | 31.04 µs |
| render_data_jjjjj | 2 | 206.69 µs |
| render_data_real | 4 | 21.09 µs |
| render_data_syntax_overhead | 2 | 20.28 µs |
| throughput | 1 | 26.90 µs |
| treesitter_query_compile | 8 | 11.39 ms |
| treesitter_register_all_languages | 1 | 12.77 µs |
| treesitter_register_language | 4 | 8.97 µs |
| viewport_size | 4 | 46.04 µs |
| window_render | 4 | 21.53 µs |
| with_highlights | 1 | 26.40 µs |

## Results

```toml
[results.complete_cycle]
full_scroll_cycle = { mean = 56.68, median = 56.55, std_dev = 1.74, unit = "µs" }

[results.event_bus_bottleneck_blocking]
block_us_100 = { mean = 100.13, median = 100.12, std_dev = 0.04, unit = "µs" }
block_us_1000 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
block_us_10000 = { mean = 10.00, median = 10.00, std_dev = 0.00, unit = "ms" }
block_us_5000 = { mean = 5.00, median = 5.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_bottleneck_comparison]
fast_only = { mean = 37.92, median = 38.02, std_dev = 0.74, unit = "ns" }
fast_with_slow_registered = { mean = 38.48, median = 38.54, std_dev = 0.65, unit = "ns" }

[results.event_bus_bottleneck_queue]
queued_events_1 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
queued_events_10 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
queued_events_20 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
queued_events_5 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_bottleneck_sequential]
langs_1x5000us = { mean = 5.00, median = 5.00, std_dev = 0.00, unit = "ms" }
langs_4x2500us = { mean = 10.00, median = 10.00, std_dev = 0.00, unit = "ms" }
langs_8x1000us = { mean = 8.00, median = 8.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_dispatch]
handlers_0 = { mean = 13.33, median = 13.49, std_dev = 0.68, unit = "ns" }
handlers_1 = { mean = 32.39, median = 31.78, std_dev = 1.82, unit = "ns" }
handlers_10 = { mean = 67.68, median = 68.32, std_dev = 2.04, unit = "ns" }
handlers_100 = { mean = 370.41, median = 370.46, std_dev = 2.42, unit = "ns" }
handlers_20 = { mean = 109.11, median = 102.67, std_dev = 17.16, unit = "ns" }
handlers_5 = { mean = 49.42, median = 47.93, std_dev = 3.27, unit = "ns" }
handlers_50 = { mean = 213.62, median = 216.01, std_dev = 6.60, unit = "ns" }

[results.event_bus_dispatch_consumed]
total_handlers_10 = { mean = 32.35, median = 32.20, std_dev = 0.63, unit = "ns" }
total_handlers_20 = { mean = 32.21, median = 31.97, std_dev = 0.78, unit = "ns" }
total_handlers_5 = { mean = 33.40, median = 32.62, std_dev = 1.60, unit = "ns" }
total_handlers_50 = { mean = 32.09, median = 32.10, std_dev = 0.39, unit = "ns" }

[results.event_bus_dispatch_emit]
emits_0 = { mean = 32.71, median = 32.69, std_dev = 0.60, unit = "ns" }
emits_1 = { mean = 54.53, median = 54.06, std_dev = 2.41, unit = "ns" }
emits_10 = { mean = 175.60, median = 173.97, std_dev = 6.26, unit = "ns" }
emits_5 = { mean = 105.34, median = 105.13, std_dev = 1.05, unit = "ns" }

[results.event_bus_dispatch_multi_type]
event_types_1 = { mean = 32.52, median = 32.21, std_dev = 0.99, unit = "ns" }
event_types_10 = { mean = 30.76, median = 31.43, std_dev = 1.34, unit = "ns" }
event_types_20 = { mean = 28.59, median = 28.59, std_dev = 0.19, unit = "ns" }
event_types_5 = { mean = 29.96, median = 28.92, std_dev = 1.90, unit = "ns" }

[results.event_bus_dispatch_priority]
handlers_10 = { mean = 66.69, median = 65.50, std_dev = 2.78, unit = "ns" }
handlers_20 = { mean = 98.17, median = 95.68, std_dev = 6.16, unit = "ns" }
handlers_5 = { mean = 43.26, median = 42.94, std_dev = 1.23, unit = "ns" }

[results.event_bus_dyn_event_new]
event_bus_dyn_event_new = { mean = 9.17, median = 9.17, std_dev = 0.35, unit = "ns" }

[results.event_bus_rwlock]
registered_handlers_1 = { mean = 30.88, median = 31.16, std_dev = 1.15, unit = "ns" }
registered_handlers_10 = { mean = 43.21, median = 42.24, std_dev = 2.68, unit = "ns" }
registered_handlers_100 = { mean = 144.70, median = 144.50, std_dev = 1.33, unit = "ns" }
registered_handlers_50 = { mean = 88.22, median = 85.93, std_dev = 5.25, unit = "ns" }

[results.event_bus_subscribe]
event_bus_subscribe = { mean = 184.83, median = 181.18, std_dev = 8.14, unit = "ns" }

[results.frame_operations]
buffer_fill_6000_cells = { mean = 31.79, median = 31.70, std_dev = 0.48, unit = "µs" }
flush_full_screen = { mean = 49.31, median = 48.25, std_dev = 3.02, unit = "µs" }
flush_scroll_simulation = { mean = 18.88, median = 18.83, std_dev = 0.10, unit = "µs" }

[results.large_file]
5000_lines_20_j_presses = { mean = 1.77, median = 1.77, std_dev = 0.02, unit = "ms" }
5000_lines_render_data = { mean = 92.71, median = 88.74, std_dev = 6.60, unit = "µs" }

[results.render_data_changelog]
scroll_pos_0 = { mean = 30.73, median = 30.16, std_dev = 1.17, unit = "µs" }
scroll_pos_100 = { mean = 30.27, median = 30.30, std_dev = 0.27, unit = "µs" }
scroll_pos_500 = { mean = 32.11, median = 30.58, std_dev = 2.67, unit = "µs" }

[results.render_data_jjjjj]
20_j_presses = { mean = 412.30, median = 396.03, std_dev = 29.39, unit = "µs" }
50_j_presses = { mean = 1.07, median = 1.08, std_dev = 0.04, unit = "ms" }

[results.render_data_real]
scroll_pos_0 = { mean = 21.38, median = 21.37, std_dev = 0.21, unit = "µs" }
scroll_pos_150 = { mean = 21.54, median = 21.20, std_dev = 1.20, unit = "µs" }
scroll_pos_300 = { mean = 21.60, median = 21.50, std_dev = 0.33, unit = "µs" }
scroll_pos_50 = { mean = 19.87, median = 19.73, std_dev = 0.52, unit = "µs" }

[results.render_data_syntax_overhead]
with_syntax = { mean = 21.09, median = 21.27, std_dev = 1.57, unit = "µs" }
without_syntax = { mean = 19.48, median = 19.35, std_dev = 0.57, unit = "µs" }

[results.throughput]
renders_per_second = { mean = 26.90, median = 26.72, std_dev = 0.78, unit = "µs" }

[results.treesitter_query_compile]
highlights_bash = { mean = 1.97, median = 1.96, std_dev = 0.01, unit = "ms" }
highlights_c = { mean = 2.86, median = 2.74, std_dev = 0.23, unit = "ms" }
highlights_javascript = { mean = 3.49, median = 3.48, std_dev = 0.05, unit = "ms" }
highlights_json = { mean = 7.89, median = 7.87, std_dev = 1.02, unit = "µs" }
highlights_markdown = { mean = 1.16, median = 1.15, std_dev = 0.02, unit = "ms" }
highlights_python = { mean = 3.61, median = 3.67, std_dev = 0.14, unit = "ms" }
highlights_rust = { mean = 25.28, median = 25.88, std_dev = 1.19, unit = "ms" }
highlights_toml = { mean = 44.86, median = 44.79, std_dev = 0.27, unit = "µs" }

[results.treesitter_register_all_languages]
treesitter_register_all_languages = { mean = 12.77, median = 12.48, std_dev = 0.46, unit = "µs" }

[results.treesitter_register_language]
c = { mean = 8.58, median = 8.49, std_dev = 0.29, unit = "µs" }
javascript = { mean = 9.58, median = 9.56, std_dev = 0.05, unit = "µs" }
python = { mean = 8.53, median = 8.50, std_dev = 0.10, unit = "µs" }
rust = { mean = 9.20, median = 9.20, std_dev = 0.10, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 51.47, median = 52.40, std_dev = 3.12, unit = "µs" }
height_200 = { mean = 94.14, median = 92.19, std_dev = 4.42, unit = "µs" }
height_24 = { mean = 11.38, median = 11.38, std_dev = 0.10, unit = "µs" }
height_50 = { mean = 27.17, median = 26.91, std_dev = 0.97, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 5.26, median = 5.29, std_dev = 0.15, unit = "µs" }
buffer_lines_100 = { mean = 26.39, median = 24.65, std_dev = 2.53, unit = "µs" }
buffer_lines_1000 = { mean = 28.21, median = 27.67, std_dev = 1.41, unit = "µs" }
buffer_lines_10000 = { mean = 26.25, median = 25.96, std_dev = 1.81, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 26.40, median = 26.46, std_dev = 1.98, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| complete_cycle/full_scroll_cycle | 56.68 µs | 56.55 µs | 1.74 µs | [56.08, 57.30] µs |
| event_bus_bottleneck_blocking/block_us/100 | 100.13 µs | 100.12 µs | 0.04 µs | [100.12, 100.15] µs |
| event_bus_bottleneck_blocking/block_us/1000 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_blocking/block_us/10000 | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_blocking/block_us/5000 | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_comparison/fast_only | 37.92 ns | 38.02 ns | 0.74 ns | [37.65, 38.17] ns |
| event_bus_bottleneck_comparison/fast_with_slow_registered | 38.48 ns | 38.54 ns | 0.65 ns | [38.25, 38.71] ns |
| event_bus_bottleneck_queue/queued_events/1 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/10 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/20 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/5 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_sequential/langs/1x5000us | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_sequential/langs/4x2500us | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_sequential/langs/8x1000us | 8.00 ms | 8.00 ms | 0.00 ms | [8.00, 8.00] ms |
| event_bus_dispatch/handlers/0 | 13.33 ns | 13.49 ns | 0.68 ns | [13.09, 13.56] ns |
| event_bus_dispatch/handlers/1 | 32.39 ns | 31.78 ns | 1.82 ns | [31.83, 33.10] ns |
| event_bus_dispatch/handlers/10 | 67.68 ns | 68.32 ns | 2.04 ns | [66.90, 68.33] ns |
| event_bus_dispatch/handlers/100 | 370.41 ns | 370.46 ns | 2.42 ns | [369.58, 371.27] ns |
| event_bus_dispatch/handlers/20 | 109.11 ns | 102.67 ns | 17.16 ns | [103.98, 115.91] ns |
| event_bus_dispatch/handlers/5 | 49.42 ns | 47.93 ns | 3.27 ns | [48.37, 50.65] ns |
| event_bus_dispatch/handlers/50 | 213.62 ns | 216.01 ns | 6.60 ns | [211.11, 215.73] ns |
| event_bus_dispatch_consumed/total_handlers/10 | 32.35 ns | 32.20 ns | 0.63 ns | [32.14, 32.58] ns |
| event_bus_dispatch_consumed/total_handlers/20 | 32.21 ns | 31.97 ns | 0.78 ns | [31.95, 32.50] ns |
| event_bus_dispatch_consumed/total_handlers/5 | 33.40 ns | 32.62 ns | 1.60 ns | [32.86, 33.98] ns |
| event_bus_dispatch_consumed/total_handlers/50 | 32.09 ns | 32.10 ns | 0.39 ns | [31.95, 32.22] ns |
| event_bus_dispatch_emit/emits/0 | 32.71 ns | 32.69 ns | 0.60 ns | [32.49, 32.92] ns |
| event_bus_dispatch_emit/emits/1 | 54.53 ns | 54.06 ns | 2.41 ns | [53.73, 55.41] ns |
| event_bus_dispatch_emit/emits/10 | 175.60 ns | 173.97 ns | 6.26 ns | [173.83, 178.12] ns |
| event_bus_dispatch_emit/emits/5 | 105.34 ns | 105.13 ns | 1.05 ns | [105.00, 105.73] ns |
| event_bus_dispatch_multi_type/event_types/1 | 32.52 ns | 32.21 ns | 0.99 ns | [32.18, 32.87] ns |
| event_bus_dispatch_multi_type/event_types/10 | 30.76 ns | 31.43 ns | 1.34 ns | [30.28, 31.22] ns |
| event_bus_dispatch_multi_type/event_types/20 | 28.59 ns | 28.59 ns | 0.19 ns | [28.52, 28.66] ns |
| event_bus_dispatch_multi_type/event_types/5 | 29.96 ns | 28.92 ns | 1.90 ns | [29.33, 30.66] ns |
| event_bus_dispatch_priority/handlers/10 | 66.69 ns | 65.50 ns | 2.78 ns | [65.81, 67.76] ns |
| event_bus_dispatch_priority/handlers/20 | 98.17 ns | 95.68 ns | 6.16 ns | [96.08, 100.43] ns |
| event_bus_dispatch_priority/handlers/5 | 43.26 ns | 42.94 ns | 1.23 ns | [42.90, 43.75] ns |
| event_bus_dyn_event_new | 9.17 ns | 9.17 ns | 0.35 ns | [9.05, 9.29] ns |
| event_bus_rwlock/registered_handlers/1 | 30.88 ns | 31.16 ns | 1.15 ns | [30.44, 31.25] ns |
| event_bus_rwlock/registered_handlers/10 | 43.21 ns | 42.24 ns | 2.68 ns | [42.35, 44.22] ns |
| event_bus_rwlock/registered_handlers/100 | 144.70 ns | 144.50 ns | 1.33 ns | [144.25, 145.18] ns |
| event_bus_rwlock/registered_handlers/50 | 88.22 ns | 85.93 ns | 5.25 ns | [86.53, 90.22] ns |
| event_bus_subscribe | 184.83 ns | 181.18 ns | 8.14 ns | [182.19, 187.88] ns |
| frame_operations/buffer_fill_6000_cells | 31.79 µs | 31.70 µs | 0.48 µs | [31.63, 31.97] µs |
| frame_operations/flush_full_screen | 49.31 µs | 48.25 µs | 3.02 µs | [48.39, 50.49] µs |
| frame_operations/flush_scroll_simulation | 18.88 µs | 18.83 µs | 0.10 µs | [18.85, 18.92] µs |
| large_file/5000_lines_20_j_presses | 1.77 ms | 1.77 ms | 0.02 ms | [1.77, 1.78] ms |
| large_file/5000_lines_render_data | 92.71 µs | 88.74 µs | 6.60 µs | [90.47, 95.11] µs |
| render_data_changelog/scroll_pos/0 | 30.73 µs | 30.16 µs | 1.17 µs | [30.34, 31.16] µs |
| render_data_changelog/scroll_pos/100 | 30.27 µs | 30.30 µs | 0.27 µs | [30.18, 30.37] µs |
| render_data_changelog/scroll_pos/500 | 32.11 µs | 30.58 µs | 2.67 µs | [31.19, 33.06] µs |
| render_data_jjjjj/20_j_presses | 412.30 µs | 396.03 µs | 29.39 µs | [402.25, 422.83] µs |
| render_data_jjjjj/50_j_presses | 1.07 ms | 1.08 ms | 0.04 ms | [1.06, 1.08] ms |
| render_data_real/scroll_pos/0 | 21.38 µs | 21.37 µs | 0.21 µs | [21.30, 21.45] µs |
| render_data_real/scroll_pos/150 | 21.54 µs | 21.20 µs | 1.20 µs | [21.15, 22.00] µs |
| render_data_real/scroll_pos/300 | 21.60 µs | 21.50 µs | 0.33 µs | [21.48, 21.72] µs |
| render_data_real/scroll_pos/50 | 19.87 µs | 19.73 µs | 0.52 µs | [19.71, 20.07] µs |
| render_data_syntax_overhead/with_syntax | 21.09 µs | 21.27 µs | 1.57 µs | [20.54, 21.64] µs |
| render_data_syntax_overhead/without_syntax | 19.48 µs | 19.35 µs | 0.57 µs | [19.32, 19.71] µs |
| throughput/renders_per_second | 26.90 µs | 26.72 µs | 0.78 µs | [26.65, 27.20] µs |
| treesitter_query_compile/highlights/bash | 1.97 ms | 1.96 ms | 0.01 ms | [1.96, 1.97] ms |
| treesitter_query_compile/highlights/c | 2.86 ms | 2.74 ms | 0.23 ms | [2.79, 2.95] ms |
| treesitter_query_compile/highlights/javascript | 3.49 ms | 3.48 ms | 0.05 ms | [3.47, 3.50] ms |
| treesitter_query_compile/highlights/json | 7.89 µs | 7.87 µs | 1.02 µs | [7.54, 8.26] µs |
| treesitter_query_compile/highlights/markdown | 1.16 ms | 1.15 ms | 0.02 ms | [1.15, 1.16] ms |
| treesitter_query_compile/highlights/python | 3.61 ms | 3.67 ms | 0.14 ms | [3.56, 3.66] ms |
| treesitter_query_compile/highlights/rust | 25.28 ms | 25.88 ms | 1.19 ms | [24.86, 25.70] ms |
| treesitter_query_compile/highlights/toml | 44.86 µs | 44.79 µs | 0.27 µs | [44.77, 44.96] µs |
| treesitter_register_all_languages | 12.77 µs | 12.48 µs | 0.46 µs | [12.61, 12.94] µs |
| treesitter_register_language/c | 8.58 µs | 8.49 µs | 0.29 µs | [8.50, 8.69] µs |
| treesitter_register_language/javascript | 9.58 µs | 9.56 µs | 0.05 µs | [9.56, 9.60] µs |
| treesitter_register_language/python | 8.53 µs | 8.50 µs | 0.10 µs | [8.50, 8.57] µs |
| treesitter_register_language/rust | 9.20 µs | 9.20 µs | 0.10 µs | [9.17, 9.23] µs |
| viewport_size/height/100 | 51.47 µs | 52.40 µs | 3.12 µs | [50.36, 52.54] µs |
| viewport_size/height/200 | 94.14 µs | 92.19 µs | 4.42 µs | [92.73, 95.82] µs |
| viewport_size/height/24 | 11.38 µs | 11.38 µs | 0.10 µs | [11.34, 11.41] µs |
| viewport_size/height/50 | 27.17 µs | 26.91 µs | 0.97 µs | [26.84, 27.52] µs |
| window_render/buffer_lines/10 | 5.26 µs | 5.29 µs | 0.15 µs | [5.20, 5.31] µs |
| window_render/buffer_lines/100 | 26.39 µs | 24.65 µs | 2.53 µs | [25.52, 27.30] µs |
| window_render/buffer_lines/1000 | 28.21 µs | 27.67 µs | 1.41 µs | [27.76, 28.75] µs |
| window_render/buffer_lines/10000 | 26.25 µs | 25.96 µs | 1.81 µs | [25.64, 26.91] µs |
| with_highlights/no_highlights | 26.40 µs | 26.46 µs | 1.98 µs | [25.72, 27.11] µs |
