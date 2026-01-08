# Performance Benchmarks v0.8.0-dev-0351294

> Last updated: 2026-01-08T12:22:48Z | Commit: 0351294 | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.8.0-dev-0351294"
commit = "0351294"
date = "2026-01-08T12:22:48Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| complete_cycle | 1 | 55.11 µs |
| event_bus_bottleneck_blocking | 4 | 29.03 µs |
| event_bus_bottleneck_comparison | 2 | 28.46 ns |
| event_bus_bottleneck_queue | 4 | 1.00 ms |
| event_bus_bottleneck_sequential | 3 | 7.67 ms |
| event_bus_dispatch | 7 | 122.07 ns |
| event_bus_dispatch_consumed | 4 | 26.73 ns |
| event_bus_dispatch_emit | 4 | 92.99 ns |
| event_bus_dispatch_multi_type | 4 | 28.70 ns |
| event_bus_dispatch_priority | 3 | 65.74 ns |
| event_bus_dyn_event_new | 1 | 8.50 ns |
| event_bus_rwlock | 4 | 69.45 ns |
| event_bus_subscribe | 1 | 188.86 ns |
| frame_operations | 3 | 34.01 µs |
| large_file | 2 | 48.63 ms |
| render_data_changelog | 3 | 31.55 µs |
| render_data_jjjjj | 2 | 669.00 µs |
| render_data_real | 4 | 20.52 µs |
| render_data_syntax_overhead | 2 | 20.24 µs |
| throughput | 1 | 23.60 µs |
| treesitter_query_compile | 8 | 11.00 ms |
| treesitter_register_all_languages | 1 | 12.44 µs |
| treesitter_register_language | 4 | 8.83 µs |
| viewport_size | 4 | 48.68 µs |
| window_render | 4 | 18.76 µs |
| with_highlights | 1 | 26.82 µs |

## Results

```toml
[results.complete_cycle]
full_scroll_cycle = { mean = 55.11, median = 54.86, std_dev = 1.56, unit = "µs" }

[results.event_bus_bottleneck_blocking]
block_us_100 = { mean = 100.13, median = 100.12, std_dev = 0.05, unit = "µs" }
block_us_1000 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
block_us_10000 = { mean = 10.00, median = 10.00, std_dev = 0.00, unit = "ms" }
block_us_5000 = { mean = 5.00, median = 5.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_bottleneck_comparison]
fast_only = { mean = 29.70, median = 29.62, std_dev = 0.84, unit = "ns" }
fast_with_slow_registered = { mean = 27.23, median = 26.21, std_dev = 1.98, unit = "ns" }

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
handlers_0 = { mean = 10.22, median = 10.53, std_dev = 0.42, unit = "ns" }
handlers_1 = { mean = 29.85, median = 29.27, std_dev = 0.94, unit = "ns" }
handlers_10 = { mean = 61.81, median = 61.54, std_dev = 1.30, unit = "ns" }
handlers_100 = { mean = 409.99, median = 397.18, std_dev = 19.61, unit = "ns" }
handlers_20 = { mean = 102.10, median = 102.07, std_dev = 1.07, unit = "ns" }
handlers_5 = { mean = 39.90, median = 39.86, std_dev = 0.30, unit = "ns" }
handlers_50 = { mean = 200.64, median = 206.14, std_dev = 8.47, unit = "ns" }

[results.event_bus_dispatch_consumed]
total_handlers_10 = { mean = 26.71, median = 26.73, std_dev = 0.10, unit = "ns" }
total_handlers_20 = { mean = 26.63, median = 26.62, std_dev = 0.11, unit = "ns" }
total_handlers_5 = { mean = 26.94, median = 26.95, std_dev = 0.08, unit = "ns" }
total_handlers_50 = { mean = 26.62, median = 26.63, std_dev = 0.11, unit = "ns" }

[results.event_bus_dispatch_emit]
emits_0 = { mean = 27.13, median = 27.14, std_dev = 0.10, unit = "ns" }
emits_1 = { mean = 48.93, median = 48.54, std_dev = 1.29, unit = "ns" }
emits_10 = { mean = 186.03, median = 184.72, std_dev = 5.25, unit = "ns" }
emits_5 = { mean = 109.86, median = 109.81, std_dev = 0.27, unit = "ns" }

[results.event_bus_dispatch_multi_type]
event_types_1 = { mean = 29.06, median = 29.14, std_dev = 0.40, unit = "ns" }
event_types_10 = { mean = 29.90, median = 29.33, std_dev = 1.24, unit = "ns" }
event_types_20 = { mean = 29.14, median = 27.93, std_dev = 2.15, unit = "ns" }
event_types_5 = { mean = 26.70, median = 26.49, std_dev = 0.56, unit = "ns" }

[results.event_bus_dispatch_priority]
handlers_10 = { mean = 55.67, median = 55.65, std_dev = 0.32, unit = "ns" }
handlers_20 = { mean = 98.90, median = 96.26, std_dev = 4.72, unit = "ns" }
handlers_5 = { mean = 42.64, median = 43.61, std_dev = 1.80, unit = "ns" }

[results.event_bus_dyn_event_new]
event_bus_dyn_event_new = { mean = 8.50, median = 8.49, std_dev = 0.03, unit = "ns" }

[results.event_bus_rwlock]
registered_handlers_1 = { mean = 25.48, median = 25.46, std_dev = 0.13, unit = "ns" }
registered_handlers_10 = { mean = 38.29, median = 36.81, std_dev = 2.48, unit = "ns" }
registered_handlers_100 = { mean = 140.89, median = 138.65, std_dev = 5.25, unit = "ns" }
registered_handlers_50 = { mean = 73.14, median = 73.00, std_dev = 0.91, unit = "ns" }

[results.event_bus_subscribe]
event_bus_subscribe = { mean = 188.86, median = 190.61, std_dev = 6.83, unit = "ns" }

[results.frame_operations]
buffer_fill_6000_cells = { mean = 30.98, median = 30.92, std_dev = 0.36, unit = "µs" }
flush_full_screen = { mean = 51.97, median = 47.28, std_dev = 6.90, unit = "µs" }
flush_scroll_simulation = { mean = 19.08, median = 19.33, std_dev = 0.68, unit = "µs" }

[results.large_file]
5000_lines_20_j_presses = { mean = 1.75, median = 1.74, std_dev = 0.03, unit = "ms" }
5000_lines_render_data = { mean = 95.52, median = 95.04, std_dev = 1.62, unit = "µs" }

[results.render_data_changelog]
scroll_pos_0 = { mean = 29.95, median = 29.76, std_dev = 1.08, unit = "µs" }
scroll_pos_100 = { mean = 32.95, median = 32.98, std_dev = 0.25, unit = "µs" }
scroll_pos_500 = { mean = 31.74, median = 32.31, std_dev = 1.18, unit = "µs" }

[results.render_data_jjjjj]
20_j_presses = { mean = 382.22, median = 382.54, std_dev = 3.94, unit = "µs" }
50_j_presses = { mean = 955.78, median = 954.76, std_dev = 7.34, unit = "µs" }

[results.render_data_real]
scroll_pos_0 = { mean = 20.19, median = 19.07, std_dev = 1.79, unit = "µs" }
scroll_pos_150 = { mean = 21.05, median = 21.08, std_dev = 0.16, unit = "µs" }
scroll_pos_300 = { mean = 21.77, median = 21.53, std_dev = 0.76, unit = "µs" }
scroll_pos_50 = { mean = 19.07, median = 19.04, std_dev = 0.17, unit = "µs" }

[results.render_data_syntax_overhead]
with_syntax = { mean = 20.63, median = 20.96, std_dev = 0.79, unit = "µs" }
without_syntax = { mean = 19.85, median = 18.65, std_dev = 1.92, unit = "µs" }

[results.throughput]
renders_per_second = { mean = 23.60, median = 23.47, std_dev = 0.57, unit = "µs" }

[results.treesitter_query_compile]
highlights_bash = { mean = 2.04, median = 1.99, std_dev = 0.12, unit = "ms" }
highlights_c = { mean = 2.71, median = 2.70, std_dev = 0.02, unit = "ms" }
highlights_javascript = { mean = 3.45, median = 3.46, std_dev = 0.25, unit = "ms" }
highlights_json = { mean = 7.29, median = 7.42, std_dev = 0.27, unit = "µs" }
highlights_markdown = { mean = 1.17, median = 1.14, std_dev = 0.06, unit = "ms" }
highlights_python = { mean = 3.33, median = 3.33, std_dev = 0.02, unit = "ms" }
highlights_rust = { mean = 23.55, median = 23.59, std_dev = 0.17, unit = "ms" }
highlights_toml = { mean = 44.46, median = 44.42, std_dev = 0.43, unit = "µs" }

[results.treesitter_register_all_languages]
treesitter_register_all_languages = { mean = 12.44, median = 12.21, std_dev = 0.59, unit = "µs" }

[results.treesitter_register_language]
c = { mean = 8.45, median = 8.44, std_dev = 0.04, unit = "µs" }
javascript = { mean = 8.55, median = 8.55, std_dev = 0.01, unit = "µs" }
python = { mean = 9.28, median = 9.33, std_dev = 0.19, unit = "µs" }
rust = { mean = 9.03, median = 9.12, std_dev = 0.25, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 53.51, median = 52.35, std_dev = 2.56, unit = "µs" }
height_200 = { mean = 104.91, median = 105.26, std_dev = 1.53, unit = "µs" }
height_24 = { mean = 12.34, median = 12.36, std_dev = 0.17, unit = "µs" }
height_50 = { mean = 23.95, median = 23.86, std_dev = 0.52, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 4.71, median = 4.70, std_dev = 0.03, unit = "µs" }
buffer_lines_100 = { mean = 23.20, median = 23.15, std_dev = 0.20, unit = "µs" }
buffer_lines_1000 = { mean = 23.51, median = 23.55, std_dev = 0.11, unit = "µs" }
buffer_lines_10000 = { mean = 23.62, median = 23.34, std_dev = 0.96, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 26.82, median = 26.83, std_dev = 0.87, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| complete_cycle/full_scroll_cycle | 55.11 µs | 54.86 µs | 1.56 µs | [54.59, 55.67] µs |
| event_bus_bottleneck_blocking/block_us/100 | 100.13 µs | 100.12 µs | 0.05 µs | [100.12, 100.15] µs |
| event_bus_bottleneck_blocking/block_us/1000 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_blocking/block_us/10000 | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_blocking/block_us/5000 | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_comparison/fast_only | 29.70 ns | 29.62 ns | 0.84 ns | [29.44, 30.02] ns |
| event_bus_bottleneck_comparison/fast_with_slow_registered | 27.23 ns | 26.21 ns | 1.98 ns | [26.58, 27.97] ns |
| event_bus_bottleneck_queue/queued_events/1 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/10 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/20 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/5 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_sequential/langs/1x5000us | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_sequential/langs/4x2500us | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_sequential/langs/8x1000us | 8.00 ms | 8.00 ms | 0.00 ms | [8.00, 8.00] ms |
| event_bus_dispatch/handlers/0 | 10.22 ns | 10.53 ns | 0.42 ns | [10.07, 10.36] ns |
| event_bus_dispatch/handlers/1 | 29.85 ns | 29.27 ns | 0.94 ns | [29.53, 30.20] ns |
| event_bus_dispatch/handlers/10 | 61.81 ns | 61.54 ns | 1.30 ns | [61.41, 62.32] ns |
| event_bus_dispatch/handlers/100 | 409.99 ns | 397.18 ns | 19.61 ns | [403.47, 417.21] ns |
| event_bus_dispatch/handlers/20 | 102.10 ns | 102.07 ns | 1.07 ns | [101.73, 102.48] ns |
| event_bus_dispatch/handlers/5 | 39.90 ns | 39.86 ns | 0.30 ns | [39.80, 40.01] ns |
| event_bus_dispatch/handlers/50 | 200.64 ns | 206.14 ns | 8.47 ns | [197.60, 203.56] ns |
| event_bus_dispatch_consumed/total_handlers/10 | 26.71 ns | 26.73 ns | 0.10 ns | [26.67, 26.74] ns |
| event_bus_dispatch_consumed/total_handlers/20 | 26.63 ns | 26.62 ns | 0.11 ns | [26.60, 26.67] ns |
| event_bus_dispatch_consumed/total_handlers/5 | 26.94 ns | 26.95 ns | 0.08 ns | [26.91, 26.97] ns |
| event_bus_dispatch_consumed/total_handlers/50 | 26.62 ns | 26.63 ns | 0.11 ns | [26.59, 26.66] ns |
| event_bus_dispatch_emit/emits/0 | 27.13 ns | 27.14 ns | 0.10 ns | [27.09, 27.16] ns |
| event_bus_dispatch_emit/emits/1 | 48.93 ns | 48.54 ns | 1.29 ns | [48.52, 49.42] ns |
| event_bus_dispatch_emit/emits/10 | 186.03 ns | 184.72 ns | 5.25 ns | [184.55, 188.11] ns |
| event_bus_dispatch_emit/emits/5 | 109.86 ns | 109.81 ns | 0.27 ns | [109.76, 109.95] ns |
| event_bus_dispatch_multi_type/event_types/1 | 29.06 ns | 29.14 ns | 0.40 ns | [28.90, 29.18] ns |
| event_bus_dispatch_multi_type/event_types/10 | 29.90 ns | 29.33 ns | 1.24 ns | [29.50, 30.37] ns |
| event_bus_dispatch_multi_type/event_types/20 | 29.14 ns | 27.93 ns | 2.15 ns | [28.40, 29.91] ns |
| event_bus_dispatch_multi_type/event_types/5 | 26.70 ns | 26.49 ns | 0.56 ns | [26.53, 26.92] ns |
| event_bus_dispatch_priority/handlers/10 | 55.67 ns | 55.65 ns | 0.32 ns | [55.56, 55.78] ns |
| event_bus_dispatch_priority/handlers/20 | 98.90 ns | 96.26 ns | 4.72 ns | [97.35, 100.64] ns |
| event_bus_dispatch_priority/handlers/5 | 42.64 ns | 43.61 ns | 1.80 ns | [41.99, 43.25] ns |
| event_bus_dyn_event_new | 8.50 ns | 8.49 ns | 0.03 ns | [8.48, 8.51] ns |
| event_bus_rwlock/registered_handlers/1 | 25.48 ns | 25.46 ns | 0.13 ns | [25.44, 25.53] ns |
| event_bus_rwlock/registered_handlers/10 | 38.29 ns | 36.81 ns | 2.48 ns | [37.48, 39.22] ns |
| event_bus_rwlock/registered_handlers/100 | 140.89 ns | 138.65 ns | 5.25 ns | [139.21, 142.88] ns |
| event_bus_rwlock/registered_handlers/50 | 73.14 ns | 73.00 ns | 0.91 ns | [72.87, 73.50] ns |
| event_bus_subscribe | 188.86 ns | 190.61 ns | 6.83 ns | [186.36, 191.16] ns |
| frame_operations/buffer_fill_6000_cells | 30.98 µs | 30.92 µs | 0.36 µs | [30.87, 31.12] µs |
| frame_operations/flush_full_screen | 51.97 µs | 47.28 µs | 6.90 µs | [49.62, 54.48] µs |
| frame_operations/flush_scroll_simulation | 19.08 µs | 19.33 µs | 0.68 µs | [18.82, 19.30] µs |
| large_file/5000_lines_20_j_presses | 1.75 ms | 1.74 ms | 0.03 ms | [1.74, 1.77] ms |
| large_file/5000_lines_render_data | 95.52 µs | 95.04 µs | 1.62 µs | [95.00, 96.14] µs |
| render_data_changelog/scroll_pos/0 | 29.95 µs | 29.76 µs | 1.08 µs | [29.66, 30.39] µs |
| render_data_changelog/scroll_pos/100 | 32.95 µs | 32.98 µs | 0.25 µs | [32.87, 33.04] µs |
| render_data_changelog/scroll_pos/500 | 31.74 µs | 32.31 µs | 1.18 µs | [31.30, 32.12] µs |
| render_data_jjjjj/20_j_presses | 382.22 µs | 382.54 µs | 3.94 µs | [380.93, 383.69] µs |
| render_data_jjjjj/50_j_presses | 955.78 µs | 954.76 µs | 7.34 µs | [953.32, 958.49] µs |
| render_data_real/scroll_pos/0 | 20.19 µs | 19.07 µs | 1.79 µs | [19.59, 20.84] µs |
| render_data_real/scroll_pos/150 | 21.05 µs | 21.08 µs | 0.16 µs | [20.99, 21.09] µs |
| render_data_real/scroll_pos/300 | 21.77 µs | 21.53 µs | 0.76 µs | [21.52, 22.05] µs |
| render_data_real/scroll_pos/50 | 19.07 µs | 19.04 µs | 0.17 µs | [19.02, 19.13] µs |
| render_data_syntax_overhead/with_syntax | 20.63 µs | 20.96 µs | 0.79 µs | [20.33, 20.88] µs |
| render_data_syntax_overhead/without_syntax | 19.85 µs | 18.65 µs | 1.92 µs | [19.21, 20.56] µs |
| throughput/renders_per_second | 23.60 µs | 23.47 µs | 0.57 µs | [23.44, 23.83] µs |
| treesitter_query_compile/highlights/bash | 2.04 ms | 1.99 ms | 0.12 ms | [2.00, 2.08] ms |
| treesitter_query_compile/highlights/c | 2.71 ms | 2.70 ms | 0.02 ms | [2.70, 2.71] ms |
| treesitter_query_compile/highlights/javascript | 3.45 ms | 3.46 ms | 0.25 ms | [3.36, 3.54] ms |
| treesitter_query_compile/highlights/json | 7.29 µs | 7.42 µs | 0.27 µs | [7.19, 7.38] µs |
| treesitter_query_compile/highlights/markdown | 1.17 ms | 1.14 ms | 0.06 ms | [1.15, 1.19] ms |
| treesitter_query_compile/highlights/python | 3.33 ms | 3.33 ms | 0.02 ms | [3.33, 3.34] ms |
| treesitter_query_compile/highlights/rust | 23.55 ms | 23.59 ms | 0.17 ms | [23.49, 23.60] ms |
| treesitter_query_compile/highlights/toml | 44.46 µs | 44.42 µs | 0.43 µs | [44.32, 44.62] µs |
| treesitter_register_all_languages | 12.44 µs | 12.21 µs | 0.59 µs | [12.27, 12.68] µs |
| treesitter_register_language/c | 8.45 µs | 8.44 µs | 0.04 µs | [8.43, 8.46] µs |
| treesitter_register_language/javascript | 8.55 µs | 8.55 µs | 0.01 µs | [8.55, 8.56] µs |
| treesitter_register_language/python | 9.28 µs | 9.33 µs | 0.19 µs | [9.21, 9.34] µs |
| treesitter_register_language/rust | 9.03 µs | 9.12 µs | 0.25 µs | [8.94, 9.11] µs |
| viewport_size/height/100 | 53.51 µs | 52.35 µs | 2.56 µs | [52.67, 54.47] µs |
| viewport_size/height/200 | 104.91 µs | 105.26 µs | 1.53 µs | [104.35, 105.43] µs |
| viewport_size/height/24 | 12.34 µs | 12.36 µs | 0.17 µs | [12.29, 12.41] µs |
| viewport_size/height/50 | 23.95 µs | 23.86 µs | 0.52 µs | [23.79, 24.16] µs |
| window_render/buffer_lines/10 | 4.71 µs | 4.70 µs | 0.03 µs | [4.70, 4.72] µs |
| window_render/buffer_lines/100 | 23.20 µs | 23.15 µs | 0.20 µs | [23.13, 23.27] µs |
| window_render/buffer_lines/1000 | 23.51 µs | 23.55 µs | 0.11 µs | [23.46, 23.55] µs |
| window_render/buffer_lines/10000 | 23.62 µs | 23.34 µs | 0.96 µs | [23.32, 23.99] µs |
| with_highlights/no_highlights | 26.82 µs | 26.83 µs | 0.87 µs | [26.49, 27.11] µs |
