# Performance Benchmarks v0.8.0

> Last updated: 2026-01-08T17:01:15Z | Commit: a605b33 | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.8.0"
commit = "a605b33"
date = "2026-01-08T17:01:15Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| complete_cycle | 1 | 47.67 µs |
| event_bus_bottleneck_blocking | 4 | 29.03 µs |
| event_bus_bottleneck_comparison | 2 | 33.68 ns |
| event_bus_bottleneck_queue | 4 | 1.00 ms |
| event_bus_bottleneck_sequential | 3 | 7.67 ms |
| event_bus_dispatch | 7 | 123.87 ns |
| event_bus_dispatch_consumed | 4 | 33.39 ns |
| event_bus_dispatch_emit | 4 | 97.08 ns |
| event_bus_dispatch_multi_type | 4 | 31.88 ns |
| event_bus_dispatch_priority | 3 | 71.08 ns |
| event_bus_dyn_event_new | 1 | 7.86 ns |
| event_bus_rwlock | 4 | 71.78 ns |
| event_bus_subscribe | 1 | 288.32 ns |
| frame_operations | 3 | 31.78 µs |
| large_file | 2 | 44.18 ms |
| render_data_changelog | 2 | 6.83 µs |
| render_data_jjjjj | 2 | 685.47 µs |
| render_data_real | 4 | 21.07 µs |
| render_data_syntax_overhead | 2 | 20.39 µs |
| throughput | 1 | 24.80 µs |
| treesitter_query_compile | 8 | 11.69 ms |
| treesitter_register_all_languages | 1 | 11.26 µs |
| treesitter_register_language | 4 | 8.94 µs |
| viewport_size | 4 | 44.21 µs |
| window_render | 4 | 19.75 µs |
| with_highlights | 1 | 26.32 µs |

## Results

```toml
[results.complete_cycle]
full_scroll_cycle = { mean = 47.67, median = 47.71, std_dev = 0.17, unit = "µs" }

[results.event_bus_bottleneck_blocking]
block_us_100 = { mean = 100.13, median = 100.13, std_dev = 0.01, unit = "µs" }
block_us_1000 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
block_us_10000 = { mean = 10.00, median = 10.00, std_dev = 0.00, unit = "ms" }
block_us_5000 = { mean = 5.00, median = 5.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_bottleneck_comparison]
fast_only = { mean = 32.03, median = 32.10, std_dev = 0.28, unit = "ns" }
fast_with_slow_registered = { mean = 35.34, median = 35.06, std_dev = 1.00, unit = "ns" }

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
handlers_0 = { mean = 13.23, median = 13.10, std_dev = 0.68, unit = "ns" }
handlers_1 = { mean = 34.90, median = 34.87, std_dev = 0.09, unit = "ns" }
handlers_10 = { mean = 67.74, median = 67.71, std_dev = 0.28, unit = "ns" }
handlers_100 = { mean = 404.51, median = 397.76, std_dev = 11.51, unit = "ns" }
handlers_20 = { mean = 104.33, median = 104.03, std_dev = 1.38, unit = "ns" }
handlers_5 = { mean = 51.30, median = 50.96, std_dev = 1.69, unit = "ns" }
handlers_50 = { mean = 191.09, median = 191.21, std_dev = 1.09, unit = "ns" }

[results.event_bus_dispatch_consumed]
total_handlers_10 = { mean = 35.41, median = 34.81, std_dev = 1.22, unit = "ns" }
total_handlers_20 = { mean = 35.31, median = 35.18, std_dev = 0.36, unit = "ns" }
total_handlers_5 = { mean = 31.60, median = 31.25, std_dev = 1.20, unit = "ns" }
total_handlers_50 = { mean = 31.24, median = 31.23, std_dev = 0.17, unit = "ns" }

[results.event_bus_dispatch_emit]
emits_0 = { mean = 35.10, median = 35.18, std_dev = 0.48, unit = "ns" }
emits_1 = { mean = 47.98, median = 47.93, std_dev = 0.21, unit = "ns" }
emits_10 = { mean = 190.69, median = 190.17, std_dev = 2.09, unit = "ns" }
emits_5 = { mean = 114.57, median = 114.60, std_dev = 0.51, unit = "ns" }

[results.event_bus_dispatch_multi_type]
event_types_1 = { mean = 33.22, median = 34.37, std_dev = 1.68, unit = "ns" }
event_types_10 = { mean = 31.39, median = 31.36, std_dev = 0.34, unit = "ns" }
event_types_20 = { mean = 31.29, median = 31.32, std_dev = 0.17, unit = "ns" }
event_types_5 = { mean = 31.61, median = 31.61, std_dev = 0.19, unit = "ns" }

[results.event_bus_dispatch_priority]
handlers_10 = { mean = 62.11, median = 62.09, std_dev = 0.24, unit = "ns" }
handlers_20 = { mean = 104.92, median = 103.80, std_dev = 2.82, unit = "ns" }
handlers_5 = { mean = 46.20, median = 46.10, std_dev = 0.87, unit = "ns" }

[results.event_bus_dyn_event_new]
event_bus_dyn_event_new = { mean = 7.86, median = 7.86, std_dev = 0.05, unit = "ns" }

[results.event_bus_rwlock]
registered_handlers_1 = { mean = 31.44, median = 31.39, std_dev = 0.37, unit = "ns" }
registered_handlers_10 = { mean = 39.15, median = 39.00, std_dev = 1.92, unit = "ns" }
registered_handlers_100 = { mean = 137.79, median = 133.49, std_dev = 8.68, unit = "ns" }
registered_handlers_50 = { mean = 78.76, median = 78.64, std_dev = 0.52, unit = "ns" }

[results.event_bus_subscribe]
event_bus_subscribe = { mean = 288.32, median = 294.93, std_dev = 11.62, unit = "ns" }

[results.frame_operations]
buffer_fill_6000_cells = { mean = 28.57, median = 28.43, std_dev = 0.61, unit = "µs" }
flush_full_screen = { mean = 48.27, median = 45.58, std_dev = 3.23, unit = "µs" }
flush_scroll_simulation = { mean = 18.48, median = 19.12, std_dev = 0.86, unit = "µs" }

[results.large_file]
5000_lines_20_j_presses = { mean = 1.72, median = 1.72, std_dev = 0.01, unit = "ms" }
5000_lines_render_data = { mean = 86.65, median = 86.38, std_dev = 0.60, unit = "µs" }

[results.render_data_changelog]
scroll_pos_0 = { mean = 6.33, median = 6.34, std_dev = 0.03, unit = "µs" }
scroll_pos_100 = { mean = 7.34, median = 7.21, std_dev = 0.29, unit = "µs" }

[results.render_data_jjjjj]
20_j_presses = { mean = 417.91, median = 422.11, std_dev = 11.69, unit = "µs" }
50_j_presses = { mean = 953.02, median = 948.74, std_dev = 6.96, unit = "µs" }

[results.render_data_real]
scroll_pos_0 = { mean = 20.83, median = 21.12, std_dev = 0.68, unit = "µs" }
scroll_pos_150 = { mean = 21.21, median = 21.28, std_dev = 0.24, unit = "µs" }
scroll_pos_300 = { mean = 21.10, median = 21.14, std_dev = 0.29, unit = "µs" }
scroll_pos_50 = { mean = 21.14, median = 21.04, std_dev = 0.24, unit = "µs" }

[results.render_data_syntax_overhead]
with_syntax = { mean = 21.73, median = 21.54, std_dev = 0.67, unit = "µs" }
without_syntax = { mean = 19.05, median = 19.02, std_dev = 0.11, unit = "µs" }

[results.throughput]
renders_per_second = { mean = 24.80, median = 24.22, std_dev = 1.37, unit = "µs" }

[results.treesitter_query_compile]
highlights_bash = { mean = 2.17, median = 2.17, std_dev = 0.01, unit = "ms" }
highlights_c = { mean = 2.96, median = 2.98, std_dev = 0.08, unit = "ms" }
highlights_javascript = { mean = 3.10, median = 3.10, std_dev = 0.01, unit = "ms" }
highlights_json = { mean = 7.67, median = 7.65, std_dev = 0.07, unit = "µs" }
highlights_markdown = { mean = 1.14, median = 1.14, std_dev = 0.01, unit = "ms" }
highlights_python = { mean = 3.71, median = 3.70, std_dev = 0.08, unit = "ms" }
highlights_rust = { mean = 23.26, median = 23.26, std_dev = 0.10, unit = "ms" }
highlights_toml = { mean = 49.48, median = 49.37, std_dev = 0.30, unit = "µs" }

[results.treesitter_register_all_languages]
treesitter_register_all_languages = { mean = 11.26, median = 11.07, std_dev = 0.44, unit = "µs" }

[results.treesitter_register_language]
c = { mean = 8.52, median = 8.46, std_dev = 0.18, unit = "µs" }
javascript = { mean = 9.55, median = 9.53, std_dev = 0.06, unit = "µs" }
python = { mean = 8.49, median = 8.49, std_dev = 0.04, unit = "µs" }
rust = { mean = 9.19, median = 9.20, std_dev = 0.06, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 45.69, median = 45.70, std_dev = 0.10, unit = "µs" }
height_200 = { mean = 96.40, median = 91.35, std_dev = 7.92, unit = "µs" }
height_24 = { mean = 11.00, median = 10.99, std_dev = 0.08, unit = "µs" }
height_50 = { mean = 23.73, median = 23.75, std_dev = 0.06, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 5.10, median = 5.07, std_dev = 0.15, unit = "µs" }
buffer_lines_100 = { mean = 25.33, median = 25.27, std_dev = 0.29, unit = "µs" }
buffer_lines_1000 = { mean = 25.27, median = 25.22, std_dev = 0.32, unit = "µs" }
buffer_lines_10000 = { mean = 23.31, median = 23.31, std_dev = 0.11, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 26.32, median = 26.70, std_dev = 0.90, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| complete_cycle/full_scroll_cycle | 47.67 µs | 47.71 µs | 0.17 µs | [47.61, 47.73] µs |
| event_bus_bottleneck_blocking/block_us/100 | 100.13 µs | 100.13 µs | 0.01 µs | [100.12, 100.13] µs |
| event_bus_bottleneck_blocking/block_us/1000 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_blocking/block_us/10000 | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_blocking/block_us/5000 | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_comparison/fast_only | 32.03 ns | 32.10 ns | 0.28 ns | [31.93, 32.13] ns |
| event_bus_bottleneck_comparison/fast_with_slow_registered | 35.34 ns | 35.06 ns | 1.00 ns | [35.03, 35.73] ns |
| event_bus_bottleneck_queue/queued_events/1 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/10 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/20 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/5 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_sequential/langs/1x5000us | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_sequential/langs/4x2500us | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_sequential/langs/8x1000us | 8.00 ms | 8.00 ms | 0.00 ms | [8.00, 8.00] ms |
| event_bus_dispatch/handlers/0 | 13.23 ns | 13.10 ns | 0.68 ns | [13.00, 13.48] ns |
| event_bus_dispatch/handlers/1 | 34.90 ns | 34.87 ns | 0.09 ns | [34.86, 34.93] ns |
| event_bus_dispatch/handlers/10 | 67.74 ns | 67.71 ns | 0.28 ns | [67.64, 67.84] ns |
| event_bus_dispatch/handlers/100 | 404.51 ns | 397.76 ns | 11.51 ns | [400.68, 408.78] ns |
| event_bus_dispatch/handlers/20 | 104.33 ns | 104.03 ns | 1.38 ns | [103.90, 104.86] ns |
| event_bus_dispatch/handlers/5 | 51.30 ns | 50.96 ns | 1.69 ns | [50.90, 51.98] ns |
| event_bus_dispatch/handlers/50 | 191.09 ns | 191.21 ns | 1.09 ns | [190.71, 191.47] ns |
| event_bus_dispatch_consumed/total_handlers/10 | 35.41 ns | 34.81 ns | 1.22 ns | [35.00, 35.86] ns |
| event_bus_dispatch_consumed/total_handlers/20 | 35.31 ns | 35.18 ns | 0.36 ns | [35.20, 35.44] ns |
| event_bus_dispatch_consumed/total_handlers/5 | 31.60 ns | 31.25 ns | 1.20 ns | [31.25, 32.08] ns |
| event_bus_dispatch_consumed/total_handlers/50 | 31.24 ns | 31.23 ns | 0.17 ns | [31.18, 31.30] ns |
| event_bus_dispatch_emit/emits/0 | 35.10 ns | 35.18 ns | 0.48 ns | [34.91, 35.24] ns |
| event_bus_dispatch_emit/emits/1 | 47.98 ns | 47.93 ns | 0.21 ns | [47.91, 48.06] ns |
| event_bus_dispatch_emit/emits/10 | 190.69 ns | 190.17 ns | 2.09 ns | [190.12, 191.53] ns |
| event_bus_dispatch_emit/emits/5 | 114.57 ns | 114.60 ns | 0.51 ns | [114.42, 114.77] ns |
| event_bus_dispatch_multi_type/event_types/1 | 33.22 ns | 34.37 ns | 1.68 ns | [32.62, 33.80] ns |
| event_bus_dispatch_multi_type/event_types/10 | 31.39 ns | 31.36 ns | 0.34 ns | [31.29, 31.52] ns |
| event_bus_dispatch_multi_type/event_types/20 | 31.29 ns | 31.32 ns | 0.17 ns | [31.23, 31.35] ns |
| event_bus_dispatch_multi_type/event_types/5 | 31.61 ns | 31.61 ns | 0.19 ns | [31.55, 31.68] ns |
| event_bus_dispatch_priority/handlers/10 | 62.11 ns | 62.09 ns | 0.24 ns | [62.03, 62.20] ns |
| event_bus_dispatch_priority/handlers/20 | 104.92 ns | 103.80 ns | 2.82 ns | [104.06, 106.02] ns |
| event_bus_dispatch_priority/handlers/5 | 46.20 ns | 46.10 ns | 0.87 ns | [45.95, 46.54] ns |
| event_bus_dyn_event_new | 7.86 ns | 7.86 ns | 0.05 ns | [7.84, 7.88] ns |
| event_bus_rwlock/registered_handlers/1 | 31.44 ns | 31.39 ns | 0.37 ns | [31.32, 31.57] ns |
| event_bus_rwlock/registered_handlers/10 | 39.15 ns | 39.00 ns | 1.92 ns | [38.48, 39.82] ns |
| event_bus_rwlock/registered_handlers/100 | 137.79 ns | 133.49 ns | 8.68 ns | [134.94, 140.98] ns |
| event_bus_rwlock/registered_handlers/50 | 78.76 ns | 78.64 ns | 0.52 ns | [78.62, 78.97] ns |
| event_bus_subscribe | 288.32 ns | 294.93 ns | 11.62 ns | [283.96, 292.04] ns |
| frame_operations/buffer_fill_6000_cells | 28.57 µs | 28.43 µs | 0.61 µs | [28.39, 28.81] µs |
| frame_operations/flush_full_screen | 48.27 µs | 45.58 µs | 3.23 µs | [47.15, 49.42] µs |
| frame_operations/flush_scroll_simulation | 18.48 µs | 19.12 µs | 0.86 µs | [18.17, 18.78] µs |
| large_file/5000_lines_20_j_presses | 1.72 ms | 1.72 ms | 0.01 ms | [1.71, 1.72] ms |
| large_file/5000_lines_render_data | 86.65 µs | 86.38 µs | 0.60 µs | [86.44, 86.87] µs |
| render_data_changelog/scroll_pos/0 | 6.33 µs | 6.34 µs | 0.03 µs | [6.32, 6.34] µs |
| render_data_changelog/scroll_pos/100 | 7.34 µs | 7.21 µs | 0.29 µs | [7.24, 7.45] µs |
| render_data_jjjjj/20_j_presses | 417.91 µs | 422.11 µs | 11.69 µs | [413.30, 421.52] µs |
| render_data_jjjjj/50_j_presses | 953.02 µs | 948.74 µs | 6.96 µs | [950.68, 955.57] µs |
| render_data_real/scroll_pos/0 | 20.83 µs | 21.12 µs | 0.68 µs | [20.57, 21.04] µs |
| render_data_real/scroll_pos/150 | 21.21 µs | 21.28 µs | 0.24 µs | [21.12, 21.28] µs |
| render_data_real/scroll_pos/300 | 21.10 µs | 21.14 µs | 0.29 µs | [20.98, 21.18] µs |
| render_data_real/scroll_pos/50 | 21.14 µs | 21.04 µs | 0.24 µs | [21.06, 21.23] µs |
| render_data_syntax_overhead/with_syntax | 21.73 µs | 21.54 µs | 0.67 µs | [21.52, 21.99] µs |
| render_data_syntax_overhead/without_syntax | 19.05 µs | 19.02 µs | 0.11 µs | [19.01, 19.09] µs |
| throughput/renders_per_second | 24.80 µs | 24.22 µs | 1.37 µs | [24.33, 25.30] µs |
| treesitter_query_compile/highlights/bash | 2.17 ms | 2.17 ms | 0.01 ms | [2.16, 2.17] ms |
| treesitter_query_compile/highlights/c | 2.96 ms | 2.98 ms | 0.08 ms | [2.93, 2.98] ms |
| treesitter_query_compile/highlights/javascript | 3.10 ms | 3.10 ms | 0.01 ms | [3.09, 3.10] ms |
| treesitter_query_compile/highlights/json | 7.67 µs | 7.65 µs | 0.07 µs | [7.65, 7.70] µs |
| treesitter_query_compile/highlights/markdown | 1.14 ms | 1.14 ms | 0.01 ms | [1.14, 1.15] ms |
| treesitter_query_compile/highlights/python | 3.71 ms | 3.70 ms | 0.08 ms | [3.68, 3.74] ms |
| treesitter_query_compile/highlights/rust | 23.26 ms | 23.26 ms | 0.10 ms | [23.23, 23.30] ms |
| treesitter_query_compile/highlights/toml | 49.48 µs | 49.37 µs | 0.30 µs | [49.38, 49.59] µs |
| treesitter_register_all_languages | 11.26 µs | 11.07 µs | 0.44 µs | [11.12, 11.43] µs |
| treesitter_register_language/c | 8.52 µs | 8.46 µs | 0.18 µs | [8.47, 8.59] µs |
| treesitter_register_language/javascript | 9.55 µs | 9.53 µs | 0.06 µs | [9.54, 9.58] µs |
| treesitter_register_language/python | 8.49 µs | 8.49 µs | 0.04 µs | [8.48, 8.50] µs |
| treesitter_register_language/rust | 9.19 µs | 9.20 µs | 0.06 µs | [9.16, 9.20] µs |
| viewport_size/height/100 | 45.69 µs | 45.70 µs | 0.10 µs | [45.66, 45.72] µs |
| viewport_size/height/200 | 96.40 µs | 91.35 µs | 7.92 µs | [93.68, 99.24] µs |
| viewport_size/height/24 | 11.00 µs | 10.99 µs | 0.08 µs | [10.97, 11.03] µs |
| viewport_size/height/50 | 23.73 µs | 23.75 µs | 0.06 µs | [23.71, 23.75] µs |
| window_render/buffer_lines/10 | 5.10 µs | 5.07 µs | 0.15 µs | [5.06, 5.16] µs |
| window_render/buffer_lines/100 | 25.33 µs | 25.27 µs | 0.29 µs | [25.25, 25.45] µs |
| window_render/buffer_lines/1000 | 25.27 µs | 25.22 µs | 0.32 µs | [25.16, 25.38] µs |
| window_render/buffer_lines/10000 | 23.31 µs | 23.31 µs | 0.11 µs | [23.27, 23.34] µs |
| with_highlights/no_highlights | 26.32 µs | 26.70 µs | 0.90 µs | [25.97, 26.61] µs |
