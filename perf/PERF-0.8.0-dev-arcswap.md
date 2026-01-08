# Performance Benchmarks v0.8.0-dev-arcswap

> Last updated: 2026-01-08T15:16:48Z | Commit: 2376a8e | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.8.0-dev-arcswap"
commit = "2376a8e"
date = "2026-01-08T15:16:48Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| complete_cycle | 1 | 52.07 µs |
| event_bus_bottleneck_blocking | 4 | 29.04 µs |
| event_bus_bottleneck_comparison | 2 | 36.59 ns |
| event_bus_bottleneck_queue | 4 | 1.00 ms |
| event_bus_bottleneck_sequential | 3 | 7.67 ms |
| event_bus_dispatch | 7 | 127.02 ns |
| event_bus_dispatch_consumed | 4 | 35.08 ns |
| event_bus_dispatch_emit | 4 | 91.13 ns |
| event_bus_dispatch_multi_type | 4 | 33.69 ns |
| event_bus_dispatch_priority | 3 | 78.16 ns |
| event_bus_dyn_event_new | 1 | 9.87 ns |
| event_bus_rwlock | 4 | 86.05 ns |
| event_bus_subscribe | 1 | 255.36 ns |
| frame_operations | 3 | 34.00 µs |
| large_file | 2 | 45.08 ms |
| render_data_changelog | 3 | 32.03 µs |
| render_data_jjjjj | 2 | 204.95 µs |
| render_data_real | 4 | 19.51 µs |
| render_data_syntax_overhead | 2 | 20.48 µs |
| throughput | 1 | 24.31 µs |
| treesitter_query_compile | 8 | 11.75 ms |
| treesitter_register_all_languages | 1 | 12.38 µs |
| treesitter_register_language | 4 | 9.63 µs |
| viewport_size | 4 | 47.19 µs |
| window_render | 4 | 19.45 µs |
| with_highlights | 1 | 24.66 µs |

## Results

```toml
[results.complete_cycle]
full_scroll_cycle = { mean = 52.07, median = 49.97, std_dev = 4.14, unit = "µs" }

[results.event_bus_bottleneck_blocking]
block_us_100 = { mean = 100.14, median = 100.13, std_dev = 0.04, unit = "µs" }
block_us_1000 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
block_us_10000 = { mean = 10.00, median = 10.00, std_dev = 0.00, unit = "ms" }
block_us_5000 = { mean = 5.00, median = 5.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_bottleneck_comparison]
fast_only = { mean = 36.81, median = 31.94, std_dev = 7.93, unit = "ns" }
fast_with_slow_registered = { mean = 36.37, median = 34.56, std_dev = 4.33, unit = "ns" }

[results.event_bus_bottleneck_queue]
queued_events_1 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
queued_events_10 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
queued_events_20 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
queued_events_5 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_bottleneck_sequential]
langs_1x5000us = { mean = 5.00, median = 5.00, std_dev = 0.00, unit = "ms" }
langs_4x2500us = { mean = 10.01, median = 10.00, std_dev = 0.02, unit = "ms" }
langs_8x1000us = { mean = 8.00, median = 8.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_dispatch]
handlers_0 = { mean = 13.25, median = 11.98, std_dev = 3.44, unit = "ns" }
handlers_1 = { mean = 34.16, median = 31.77, std_dev = 5.76, unit = "ns" }
handlers_10 = { mean = 62.67, median = 62.60, std_dev = 1.36, unit = "ns" }
handlers_100 = { mean = 400.12, median = 393.40, std_dev = 17.33, unit = "ns" }
handlers_20 = { mean = 108.42, median = 107.24, std_dev = 4.64, unit = "ns" }
handlers_5 = { mean = 46.10, median = 45.95, std_dev = 0.54, unit = "ns" }
handlers_50 = { mean = 224.44, median = 224.31, std_dev = 3.66, unit = "ns" }

[results.event_bus_dispatch_consumed]
total_handlers_10 = { mean = 33.85, median = 32.17, std_dev = 2.92, unit = "ns" }
total_handlers_20 = { mean = 34.12, median = 31.93, std_dev = 5.81, unit = "ns" }
total_handlers_5 = { mean = 37.65, median = 32.83, std_dev = 7.53, unit = "ns" }
total_handlers_50 = { mean = 34.69, median = 32.46, std_dev = 4.85, unit = "ns" }

[results.event_bus_dispatch_emit]
emits_0 = { mean = 32.17, median = 31.98, std_dev = 0.58, unit = "ns" }
emits_1 = { mean = 49.05, median = 48.16, std_dev = 3.02, unit = "ns" }
emits_10 = { mean = 175.71, median = 175.51, std_dev = 1.14, unit = "ns" }
emits_5 = { mean = 107.61, median = 105.50, std_dev = 6.18, unit = "ns" }

[results.event_bus_dispatch_multi_type]
event_types_1 = { mean = 31.76, median = 31.74, std_dev = 0.19, unit = "ns" }
event_types_10 = { mean = 35.49, median = 35.57, std_dev = 0.73, unit = "ns" }
event_types_20 = { mean = 31.74, median = 31.44, std_dev = 1.23, unit = "ns" }
event_types_5 = { mean = 35.78, median = 35.34, std_dev = 1.30, unit = "ns" }

[results.event_bus_dispatch_priority]
handlers_10 = { mean = 68.61, median = 68.34, std_dev = 1.48, unit = "ns" }
handlers_20 = { mean = 111.79, median = 111.96, std_dev = 4.63, unit = "ns" }
handlers_5 = { mean = 54.07, median = 53.04, std_dev = 3.94, unit = "ns" }

[results.event_bus_dyn_event_new]
event_bus_dyn_event_new = { mean = 9.87, median = 8.58, std_dev = 2.53, unit = "ns" }

[results.event_bus_rwlock]
registered_handlers_1 = { mean = 31.98, median = 29.01, std_dev = 6.06, unit = "ns" }
registered_handlers_10 = { mean = 42.89, median = 37.70, std_dev = 9.70, unit = "ns" }
registered_handlers_100 = { mean = 174.97, median = 150.33, std_dev = 41.83, unit = "ns" }
registered_handlers_50 = { mean = 94.34, median = 88.55, std_dev = 18.29, unit = "ns" }

[results.event_bus_subscribe]
event_bus_subscribe = { mean = 255.36, median = 254.59, std_dev = 5.12, unit = "ns" }

[results.frame_operations]
buffer_fill_6000_cells = { mean = 28.78, median = 28.59, std_dev = 0.44, unit = "µs" }
flush_full_screen = { mean = 54.22, median = 49.57, std_dev = 9.86, unit = "µs" }
flush_scroll_simulation = { mean = 19.00, median = 18.98, std_dev = 0.21, unit = "µs" }

[results.large_file]
5000_lines_20_j_presses = { mean = 1.91, median = 1.84, std_dev = 0.17, unit = "ms" }
5000_lines_render_data = { mean = 88.25, median = 88.50, std_dev = 0.84, unit = "µs" }

[results.render_data_changelog]
scroll_pos_0 = { mean = 30.58, median = 30.16, std_dev = 1.24, unit = "µs" }
scroll_pos_100 = { mean = 31.51, median = 31.52, std_dev = 0.19, unit = "µs" }
scroll_pos_500 = { mean = 34.00, median = 31.88, std_dev = 5.29, unit = "µs" }

[results.render_data_jjjjj]
20_j_presses = { mean = 408.87, median = 385.09, std_dev = 40.31, unit = "µs" }
50_j_presses = { mean = 1.03, median = 1.03, std_dev = 0.04, unit = "ms" }

[results.render_data_real]
scroll_pos_0 = { mean = 19.27, median = 19.18, std_dev = 0.38, unit = "µs" }
scroll_pos_150 = { mean = 19.60, median = 19.22, std_dev = 1.27, unit = "µs" }
scroll_pos_300 = { mean = 19.96, median = 20.02, std_dev = 0.24, unit = "µs" }
scroll_pos_50 = { mean = 19.19, median = 19.08, std_dev = 0.59, unit = "µs" }

[results.render_data_syntax_overhead]
with_syntax = { mean = 20.06, median = 19.54, std_dev = 1.40, unit = "µs" }
without_syntax = { mean = 20.91, median = 19.84, std_dev = 2.03, unit = "µs" }

[results.throughput]
renders_per_second = { mean = 24.31, median = 23.84, std_dev = 1.33, unit = "µs" }

[results.treesitter_query_compile]
highlights_bash = { mean = 2.15, median = 2.05, std_dev = 0.30, unit = "ms" }
highlights_c = { mean = 2.80, median = 2.77, std_dev = 0.13, unit = "ms" }
highlights_javascript = { mean = 3.33, median = 3.16, std_dev = 0.54, unit = "ms" }
highlights_json = { mean = 7.03, median = 6.91, std_dev = 0.36, unit = "µs" }
highlights_markdown = { mean = 1.10, median = 1.04, std_dev = 0.21, unit = "ms" }
highlights_python = { mean = 4.13, median = 3.77, std_dev = 0.87, unit = "ms" }
highlights_rust = { mean = 26.37, median = 23.92, std_dev = 5.00, unit = "ms" }
highlights_toml = { mean = 47.04, median = 46.23, std_dev = 2.97, unit = "µs" }

[results.treesitter_register_all_languages]
treesitter_register_all_languages = { mean = 12.38, median = 11.17, std_dev = 2.86, unit = "µs" }

[results.treesitter_register_language]
c = { mean = 9.32, median = 8.55, std_dev = 1.84, unit = "µs" }
javascript = { mean = 9.32, median = 8.64, std_dev = 1.74, unit = "µs" }
python = { mean = 9.36, median = 8.62, std_dev = 1.60, unit = "µs" }
rust = { mean = 10.51, median = 8.37, std_dev = 3.09, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 59.83, median = 48.45, std_dev = 18.12, unit = "µs" }
height_200 = { mean = 93.61, median = 93.24, std_dev = 2.05, unit = "µs" }
height_24 = { mean = 11.43, median = 11.38, std_dev = 0.19, unit = "µs" }
height_50 = { mean = 23.90, median = 23.55, std_dev = 1.07, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 4.76, median = 4.61, std_dev = 0.39, unit = "µs" }
buffer_lines_100 = { mean = 24.69, median = 23.17, std_dev = 5.17, unit = "µs" }
buffer_lines_1000 = { mean = 24.91, median = 23.78, std_dev = 2.16, unit = "µs" }
buffer_lines_10000 = { mean = 23.45, median = 23.36, std_dev = 0.41, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 24.66, median = 23.18, std_dev = 5.14, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| complete_cycle/full_scroll_cycle | 52.07 µs | 49.97 µs | 4.14 µs | [50.74, 53.62] µs |
| event_bus_bottleneck_blocking/block_us/100 | 100.14 µs | 100.13 µs | 0.04 µs | [100.13, 100.16] µs |
| event_bus_bottleneck_blocking/block_us/1000 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_blocking/block_us/10000 | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_blocking/block_us/5000 | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_comparison/fast_only | 36.81 ns | 31.94 ns | 7.93 ns | [34.17, 39.70] ns |
| event_bus_bottleneck_comparison/fast_with_slow_registered | 36.37 ns | 34.56 ns | 4.33 ns | [35.03, 38.06] ns |
| event_bus_bottleneck_queue/queued_events/1 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/10 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/20 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/5 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_sequential/langs/1x5000us | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_sequential/langs/4x2500us | 10.01 ms | 10.00 ms | 0.02 ms | [10.00, 10.01] ms |
| event_bus_bottleneck_sequential/langs/8x1000us | 8.00 ms | 8.00 ms | 0.00 ms | [8.00, 8.00] ms |
| event_bus_dispatch/handlers/0 | 13.25 ns | 11.98 ns | 3.44 ns | [12.16, 14.62] ns |
| event_bus_dispatch/handlers/1 | 34.16 ns | 31.77 ns | 5.76 ns | [32.35, 36.36] ns |
| event_bus_dispatch/handlers/10 | 62.67 ns | 62.60 ns | 1.36 ns | [62.31, 63.22] ns |
| event_bus_dispatch/handlers/100 | 400.12 ns | 393.40 ns | 17.33 ns | [394.50, 406.65] ns |
| event_bus_dispatch/handlers/20 | 108.42 ns | 107.24 ns | 4.64 ns | [106.85, 110.10] ns |
| event_bus_dispatch/handlers/5 | 46.10 ns | 45.95 ns | 0.54 ns | [45.94, 46.32] ns |
| event_bus_dispatch/handlers/50 | 224.44 ns | 224.31 ns | 3.66 ns | [223.05, 225.64] ns |
| event_bus_dispatch_consumed/total_handlers/10 | 33.85 ns | 32.17 ns | 2.92 ns | [32.88, 34.94] ns |
| event_bus_dispatch_consumed/total_handlers/20 | 34.12 ns | 31.93 ns | 5.81 ns | [32.32, 36.35] ns |
| event_bus_dispatch_consumed/total_handlers/5 | 37.65 ns | 32.83 ns | 7.53 ns | [35.11, 40.42] ns |
| event_bus_dispatch_consumed/total_handlers/50 | 34.69 ns | 32.46 ns | 4.85 ns | [33.14, 36.52] ns |
| event_bus_dispatch_emit/emits/0 | 32.17 ns | 31.98 ns | 0.58 ns | [31.98, 32.39] ns |
| event_bus_dispatch_emit/emits/1 | 49.05 ns | 48.16 ns | 3.02 ns | [48.16, 50.24] ns |
| event_bus_dispatch_emit/emits/10 | 175.71 ns | 175.51 ns | 1.14 ns | [175.32, 176.12] ns |
| event_bus_dispatch_emit/emits/5 | 107.61 ns | 105.50 ns | 6.18 ns | [105.76, 110.03] ns |
| event_bus_dispatch_multi_type/event_types/1 | 31.76 ns | 31.74 ns | 0.19 ns | [31.70, 31.83] ns |
| event_bus_dispatch_multi_type/event_types/10 | 35.49 ns | 35.57 ns | 0.73 ns | [35.24, 35.75] ns |
| event_bus_dispatch_multi_type/event_types/20 | 31.74 ns | 31.44 ns | 1.23 ns | [31.41, 32.24] ns |
| event_bus_dispatch_multi_type/event_types/5 | 35.78 ns | 35.34 ns | 1.30 ns | [35.35, 36.27] ns |
| event_bus_dispatch_priority/handlers/10 | 68.61 ns | 68.34 ns | 1.48 ns | [68.12, 69.17] ns |
| event_bus_dispatch_priority/handlers/20 | 111.79 ns | 111.96 ns | 4.63 ns | [110.15, 113.41] ns |
| event_bus_dispatch_priority/handlers/5 | 54.07 ns | 53.04 ns | 3.94 ns | [52.71, 55.47] ns |
| event_bus_dyn_event_new | 9.87 ns | 8.58 ns | 2.53 ns | [9.03, 10.81] ns |
| event_bus_rwlock/registered_handlers/1 | 31.98 ns | 29.01 ns | 6.06 ns | [30.05, 34.30] ns |
| event_bus_rwlock/registered_handlers/10 | 42.89 ns | 37.70 ns | 9.70 ns | [39.76, 46.57] ns |
| event_bus_rwlock/registered_handlers/100 | 174.97 ns | 150.33 ns | 41.83 ns | [160.92, 190.27] ns |
| event_bus_rwlock/registered_handlers/50 | 94.34 ns | 88.55 ns | 18.29 ns | [88.49, 101.23] ns |
| event_bus_subscribe | 255.36 ns | 254.59 ns | 5.12 ns | [254.07, 257.44] ns |
| frame_operations/buffer_fill_6000_cells | 28.78 µs | 28.59 µs | 0.44 µs | [28.64, 28.95] µs |
| frame_operations/flush_full_screen | 54.22 µs | 49.57 µs | 9.86 µs | [51.21, 58.02] µs |
| frame_operations/flush_scroll_simulation | 19.00 µs | 18.98 µs | 0.21 µs | [18.92, 19.08] µs |
| large_file/5000_lines_20_j_presses | 1.91 ms | 1.84 ms | 0.17 ms | [1.85, 1.97] ms |
| large_file/5000_lines_render_data | 88.25 µs | 88.50 µs | 0.84 µs | [87.93, 88.51] µs |
| render_data_changelog/scroll_pos/0 | 30.58 µs | 30.16 µs | 1.24 µs | [30.22, 31.07] µs |
| render_data_changelog/scroll_pos/100 | 31.51 µs | 31.52 µs | 0.19 µs | [31.45, 31.58] µs |
| render_data_changelog/scroll_pos/500 | 34.00 µs | 31.88 µs | 5.29 µs | [32.33, 36.03] µs |
| render_data_jjjjj/20_j_presses | 408.87 µs | 385.09 µs | 40.31 µs | [395.47, 423.72] µs |
| render_data_jjjjj/50_j_presses | 1.03 ms | 1.03 ms | 0.04 ms | [1.01, 1.04] ms |
| render_data_real/scroll_pos/0 | 19.27 µs | 19.18 µs | 0.38 µs | [19.15, 19.42] µs |
| render_data_real/scroll_pos/150 | 19.60 µs | 19.22 µs | 1.27 µs | [19.26, 20.12] µs |
| render_data_real/scroll_pos/300 | 19.96 µs | 20.02 µs | 0.24 µs | [19.87, 20.04] µs |
| render_data_real/scroll_pos/50 | 19.19 µs | 19.08 µs | 0.59 µs | [19.05, 19.43] µs |
| render_data_syntax_overhead/with_syntax | 20.06 µs | 19.54 µs | 1.40 µs | [19.62, 20.60] µs |
| render_data_syntax_overhead/without_syntax | 20.91 µs | 19.84 µs | 2.03 µs | [20.24, 21.66] µs |
| throughput/renders_per_second | 24.31 µs | 23.84 µs | 1.33 µs | [23.91, 24.82] µs |
| treesitter_query_compile/highlights/bash | 2.15 ms | 2.05 ms | 0.30 ms | [2.06, 2.27] ms |
| treesitter_query_compile/highlights/c | 2.80 ms | 2.77 ms | 0.13 ms | [2.77, 2.86] ms |
| treesitter_query_compile/highlights/javascript | 3.33 ms | 3.16 ms | 0.54 ms | [3.17, 3.55] ms |
| treesitter_query_compile/highlights/json | 7.03 µs | 6.91 µs | 0.36 µs | [6.93, 7.17] µs |
| treesitter_query_compile/highlights/markdown | 1.10 ms | 1.04 ms | 0.21 ms | [1.04, 1.19] ms |
| treesitter_query_compile/highlights/python | 4.13 ms | 3.77 ms | 0.87 ms | [3.86, 4.47] ms |
| treesitter_query_compile/highlights/rust | 26.37 ms | 23.92 ms | 5.00 ms | [24.81, 28.30] ms |
| treesitter_query_compile/highlights/toml | 47.04 µs | 46.23 µs | 2.97 µs | [46.22, 48.25] µs |
| treesitter_register_all_languages | 12.38 µs | 11.17 µs | 2.86 µs | [11.48, 13.47] µs |
| treesitter_register_language/c | 9.32 µs | 8.55 µs | 1.84 µs | [8.75, 10.02] µs |
| treesitter_register_language/javascript | 9.32 µs | 8.64 µs | 1.74 µs | [8.78, 9.99] µs |
| treesitter_register_language/python | 9.36 µs | 8.62 µs | 1.60 µs | [8.85, 9.97] µs |
| treesitter_register_language/rust | 10.51 µs | 8.37 µs | 3.09 µs | [9.47, 11.64] µs |
| viewport_size/height/100 | 59.83 µs | 48.45 µs | 18.12 µs | [53.75, 66.39] µs |
| viewport_size/height/200 | 93.61 µs | 93.24 µs | 2.05 µs | [92.97, 94.41] µs |
| viewport_size/height/24 | 11.43 µs | 11.38 µs | 0.19 µs | [11.37, 11.50] µs |
| viewport_size/height/50 | 23.90 µs | 23.55 µs | 1.07 µs | [23.60, 24.34] µs |
| window_render/buffer_lines/10 | 4.76 µs | 4.61 µs | 0.39 µs | [4.63, 4.91] µs |
| window_render/buffer_lines/100 | 24.69 µs | 23.17 µs | 5.17 µs | [23.13, 26.73] µs |
| window_render/buffer_lines/1000 | 24.91 µs | 23.78 µs | 2.16 µs | [24.19, 25.70] µs |
| window_render/buffer_lines/10000 | 23.45 µs | 23.36 µs | 0.41 µs | [23.31, 23.60] µs |
| with_highlights/no_highlights | 24.66 µs | 23.18 µs | 5.14 µs | [23.23, 26.74] µs |
