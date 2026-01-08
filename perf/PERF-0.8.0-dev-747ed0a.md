# Performance Benchmarks v0.8.0-dev-747ed0a

> Last updated: 2026-01-08T10:39:54Z | Commit: 747ed0a | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.8.0-dev-747ed0a"
commit = "747ed0a"
date = "2026-01-08T10:39:54Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| complete_cycle | 1 | 53.10 µs |
| event_bus_bottleneck_blocking | 4 | 29.03 µs |
| event_bus_bottleneck_comparison | 2 | 26.51 ns |
| event_bus_bottleneck_queue | 4 | 1.00 ms |
| event_bus_bottleneck_sequential | 3 | 7.67 ms |
| event_bus_dispatch | 7 | 118.30 ns |
| event_bus_dispatch_consumed | 4 | 27.49 ns |
| event_bus_dispatch_emit | 4 | 58.05 ns |
| event_bus_dispatch_multi_type | 4 | 26.38 ns |
| event_bus_dispatch_priority | 3 | 61.14 ns |
| event_bus_dyn_event_new | 1 | 10.40 ns |
| event_bus_rwlock | 4 | 68.37 ns |
| event_bus_subscribe | 1 | 177.58 ns |
| frame_operations | 3 | 33.33 µs |
| large_file | 2 | 42.99 ms |
| render_data_changelog | 5 | 117.59 µs |
| render_data_jjjjj | 2 | 207.63 µs |
| render_data_real | 4 | 19.59 µs |
| render_data_syntax_overhead | 2 | 19.95 µs |
| throughput | 1 | 24.98 µs |
| treesitter_query_compile | 8 | 10.96 ms |
| treesitter_register_all_languages | 1 | 78.46 ms |
| treesitter_register_language | 4 | 14.64 ms |
| viewport_size | 4 | 45.30 µs |
| window_render | 4 | 19.05 µs |
| with_highlights | 1 | 25.11 µs |

## Results

```toml
[results.complete_cycle]
full_scroll_cycle = { mean = 53.10, median = 53.07, std_dev = 0.10, unit = "µs" }

[results.event_bus_bottleneck_blocking]
block_us_100 = { mean = 100.12, median = 100.11, std_dev = 0.05, unit = "µs" }
block_us_1000 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
block_us_10000 = { mean = 10.00, median = 10.00, std_dev = 0.00, unit = "ms" }
block_us_5000 = { mean = 5.00, median = 5.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_bottleneck_comparison]
fast_only = { mean = 27.71, median = 27.73, std_dev = 0.13, unit = "ns" }
fast_with_slow_registered = { mean = 25.31, median = 24.98, std_dev = 0.75, unit = "ns" }

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
handlers_0 = { mean = 8.91, median = 8.91, std_dev = 0.02, unit = "ns" }
handlers_1 = { mean = 24.68, median = 24.69, std_dev = 0.12, unit = "ns" }
handlers_10 = { mean = 58.76, median = 58.73, std_dev = 0.21, unit = "ns" }
handlers_100 = { mean = 397.01, median = 397.31, std_dev = 3.46, unit = "ns" }
handlers_20 = { mean = 85.14, median = 85.08, std_dev = 0.75, unit = "ns" }
handlers_5 = { mean = 42.08, median = 42.01, std_dev = 0.36, unit = "ns" }
handlers_50 = { mean = 211.52, median = 208.31, std_dev = 7.45, unit = "ns" }

[results.event_bus_dispatch_consumed]
total_handlers_10 = { mean = 27.46, median = 27.46, std_dev = 0.17, unit = "ns" }
total_handlers_20 = { mean = 27.46, median = 27.45, std_dev = 0.16, unit = "ns" }
total_handlers_5 = { mean = 27.44, median = 27.41, std_dev = 0.13, unit = "ns" }
total_handlers_50 = { mean = 27.59, median = 27.59, std_dev = 0.07, unit = "ns" }

[results.event_bus_dispatch_emit]
emits_0 = { mean = 25.07, median = 25.07, std_dev = 0.14, unit = "ns" }
emits_1 = { mean = 32.90, median = 32.43, std_dev = 1.35, unit = "ns" }
emits_10 = { mean = 109.01, median = 109.52, std_dev = 2.19, unit = "ns" }
emits_5 = { mean = 65.22, median = 65.35, std_dev = 1.08, unit = "ns" }

[results.event_bus_dispatch_multi_type]
event_types_1 = { mean = 27.32, median = 27.34, std_dev = 0.21, unit = "ns" }
event_types_10 = { mean = 27.54, median = 27.32, std_dev = 1.01, unit = "ns" }
event_types_20 = { mean = 25.27, median = 24.76, std_dev = 1.01, unit = "ns" }
event_types_5 = { mean = 25.38, median = 24.73, std_dev = 1.18, unit = "ns" }

[results.event_bus_dispatch_priority]
handlers_10 = { mean = 59.96, median = 58.86, std_dev = 1.66, unit = "ns" }
handlers_20 = { mean = 85.56, median = 85.53, std_dev = 0.39, unit = "ns" }
handlers_5 = { mean = 37.90, median = 37.84, std_dev = 0.28, unit = "ns" }

[results.event_bus_dyn_event_new]
event_bus_dyn_event_new = { mean = 10.40, median = 10.53, std_dev = 0.20, unit = "ns" }

[results.event_bus_rwlock]
registered_handlers_1 = { mean = 23.09, median = 23.09, std_dev = 0.10, unit = "ns" }
registered_handlers_10 = { mean = 35.33, median = 35.39, std_dev = 0.26, unit = "ns" }
registered_handlers_100 = { mean = 135.56, median = 135.49, std_dev = 1.04, unit = "ns" }
registered_handlers_50 = { mean = 79.50, median = 79.41, std_dev = 0.41, unit = "ns" }

[results.event_bus_subscribe]
event_bus_subscribe = { mean = 177.58, median = 177.69, std_dev = 0.97, unit = "ns" }

[results.frame_operations]
buffer_fill_6000_cells = { mean = 29.35, median = 29.35, std_dev = 0.11, unit = "µs" }
flush_full_screen = { mean = 51.53, median = 51.06, std_dev = 1.32, unit = "µs" }
flush_scroll_simulation = { mean = 19.09, median = 19.29, std_dev = 0.71, unit = "µs" }

[results.large_file]
5000_lines_20_j_presses = { mean = 1.88, median = 1.90, std_dev = 0.06, unit = "ms" }
5000_lines_render_data = { mean = 84.10, median = 84.10, std_dev = 0.12, unit = "µs" }

[results.render_data_changelog]
scroll_pos_0 = { mean = 118.77, median = 118.39, std_dev = 0.97, unit = "µs" }
scroll_pos_100 = { mean = 119.05, median = 118.90, std_dev = 0.98, unit = "µs" }
scroll_pos_1000 = { mean = 108.05, median = 106.78, std_dev = 2.39, unit = "µs" }
scroll_pos_1500 = { mean = 120.28, median = 118.85, std_dev = 3.77, unit = "µs" }
scroll_pos_500 = { mean = 121.78, median = 121.12, std_dev = 2.30, unit = "µs" }

[results.render_data_jjjjj]
20_j_presses = { mean = 414.22, median = 415.99, std_dev = 7.48, unit = "µs" }
50_j_presses = { mean = 1.05, median = 1.05, std_dev = 0.01, unit = "ms" }

[results.render_data_real]
scroll_pos_0 = { mean = 19.10, median = 19.12, std_dev = 0.09, unit = "µs" }
scroll_pos_150 = { mean = 21.31, median = 21.30, std_dev = 0.03, unit = "µs" }
scroll_pos_300 = { mean = 18.93, median = 18.93, std_dev = 0.10, unit = "µs" }
scroll_pos_50 = { mean = 19.01, median = 19.00, std_dev = 0.10, unit = "µs" }

[results.render_data_syntax_overhead]
with_syntax = { mean = 21.03, median = 20.92, std_dev = 0.24, unit = "µs" }
without_syntax = { mean = 18.86, median = 18.80, std_dev = 0.11, unit = "µs" }

[results.throughput]
renders_per_second = { mean = 24.98, median = 25.02, std_dev = 0.19, unit = "µs" }

[results.treesitter_query_compile]
highlights_bash = { mean = 1.97, median = 1.96, std_dev = 0.02, unit = "ms" }
highlights_c = { mean = 2.70, median = 2.69, std_dev = 0.02, unit = "ms" }
highlights_javascript = { mean = 3.49, median = 3.47, std_dev = 0.05, unit = "ms" }
highlights_json = { mean = 6.85, median = 6.85, std_dev = 0.01, unit = "µs" }
highlights_markdown = { mean = 1.03, median = 1.03, std_dev = 0.01, unit = "ms" }
highlights_python = { mean = 3.61, median = 3.67, std_dev = 0.14, unit = "ms" }
highlights_rust = { mean = 23.06, median = 23.04, std_dev = 0.14, unit = "ms" }
highlights_toml = { mean = 44.96, median = 44.97, std_dev = 0.29, unit = "µs" }

[results.treesitter_register_all_languages]
treesitter_register_all_languages = { mean = 78.46, median = 79.48, std_dev = 3.44, unit = "ms" }

[results.treesitter_register_language]
c = { mean = 8.46, median = 8.12, std_dev = 0.56, unit = "ms" }
javascript = { mean = 8.11, median = 7.99, std_dev = 0.52, unit = "ms" }
python = { mean = 8.17, median = 8.13, std_dev = 0.13, unit = "ms" }
rust = { mean = 33.81, median = 33.78, std_dev = 0.20, unit = "ms" }

[results.viewport_size]
height_100 = { mean = 49.76, median = 50.09, std_dev = 0.85, unit = "µs" }
height_200 = { mean = 97.19, median = 97.66, std_dev = 4.06, unit = "µs" }
height_24 = { mean = 10.52, median = 10.50, std_dev = 0.07, unit = "µs" }
height_50 = { mean = 23.75, median = 22.86, std_dev = 1.31, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 5.02, median = 5.00, std_dev = 0.05, unit = "µs" }
buffer_lines_100 = { mean = 24.14, median = 24.10, std_dev = 0.46, unit = "µs" }
buffer_lines_1000 = { mean = 24.57, median = 24.38, std_dev = 0.53, unit = "µs" }
buffer_lines_10000 = { mean = 22.47, median = 22.46, std_dev = 0.09, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 25.11, median = 24.80, std_dev = 0.73, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| complete_cycle/full_scroll_cycle | 53.10 µs | 53.07 µs | 0.10 µs | [53.07, 53.14] µs |
| event_bus_bottleneck_blocking/block_us/100 | 100.12 µs | 100.11 µs | 0.05 µs | [100.11, 100.14] µs |
| event_bus_bottleneck_blocking/block_us/1000 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_blocking/block_us/10000 | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_blocking/block_us/5000 | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_comparison/fast_only | 27.71 ns | 27.73 ns | 0.13 ns | [27.67, 27.76] ns |
| event_bus_bottleneck_comparison/fast_with_slow_registered | 25.31 ns | 24.98 ns | 0.75 ns | [25.06, 25.59] ns |
| event_bus_bottleneck_queue/queued_events/1 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/10 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/20 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/5 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_sequential/langs/1x5000us | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_sequential/langs/4x2500us | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_sequential/langs/8x1000us | 8.00 ms | 8.00 ms | 0.00 ms | [8.00, 8.00] ms |
| event_bus_dispatch/handlers/0 | 8.91 ns | 8.91 ns | 0.02 ns | [8.91, 8.92] ns |
| event_bus_dispatch/handlers/1 | 24.68 ns | 24.69 ns | 0.12 ns | [24.63, 24.72] ns |
| event_bus_dispatch/handlers/10 | 58.76 ns | 58.73 ns | 0.21 ns | [58.69, 58.83] ns |
| event_bus_dispatch/handlers/100 | 397.01 ns | 397.31 ns | 3.46 ns | [395.85, 398.30] ns |
| event_bus_dispatch/handlers/20 | 85.14 ns | 85.08 ns | 0.75 ns | [84.91, 85.43] ns |
| event_bus_dispatch/handlers/5 | 42.08 ns | 42.01 ns | 0.36 ns | [41.96, 42.21] ns |
| event_bus_dispatch/handlers/50 | 211.52 ns | 208.31 ns | 7.45 ns | [209.11, 214.35] ns |
| event_bus_dispatch_consumed/total_handlers/10 | 27.46 ns | 27.46 ns | 0.17 ns | [27.39, 27.51] ns |
| event_bus_dispatch_consumed/total_handlers/20 | 27.46 ns | 27.45 ns | 0.16 ns | [27.41, 27.52] ns |
| event_bus_dispatch_consumed/total_handlers/5 | 27.44 ns | 27.41 ns | 0.13 ns | [27.40, 27.48] ns |
| event_bus_dispatch_consumed/total_handlers/50 | 27.59 ns | 27.59 ns | 0.07 ns | [27.56, 27.61] ns |
| event_bus_dispatch_emit/emits/0 | 25.07 ns | 25.07 ns | 0.14 ns | [25.02, 25.11] ns |
| event_bus_dispatch_emit/emits/1 | 32.90 ns | 32.43 ns | 1.35 ns | [32.49, 33.42] ns |
| event_bus_dispatch_emit/emits/10 | 109.01 ns | 109.52 ns | 2.19 ns | [108.30, 109.83] ns |
| event_bus_dispatch_emit/emits/5 | 65.22 ns | 65.35 ns | 1.08 ns | [64.80, 65.56] ns |
| event_bus_dispatch_multi_type/event_types/1 | 27.32 ns | 27.34 ns | 0.21 ns | [27.24, 27.39] ns |
| event_bus_dispatch_multi_type/event_types/10 | 27.54 ns | 27.32 ns | 1.01 ns | [27.28, 27.95] ns |
| event_bus_dispatch_multi_type/event_types/20 | 25.27 ns | 24.76 ns | 1.01 ns | [24.93, 25.64] ns |
| event_bus_dispatch_multi_type/event_types/5 | 25.38 ns | 24.73 ns | 1.18 ns | [24.98, 25.81] ns |
| event_bus_dispatch_priority/handlers/10 | 59.96 ns | 58.86 ns | 1.66 ns | [59.40, 60.57] ns |
| event_bus_dispatch_priority/handlers/20 | 85.56 ns | 85.53 ns | 0.39 ns | [85.42, 85.70] ns |
| event_bus_dispatch_priority/handlers/5 | 37.90 ns | 37.84 ns | 0.28 ns | [37.81, 38.00] ns |
| event_bus_dyn_event_new | 10.40 ns | 10.53 ns | 0.20 ns | [10.33, 10.47] ns |
| event_bus_rwlock/registered_handlers/1 | 23.09 ns | 23.09 ns | 0.10 ns | [23.05, 23.13] ns |
| event_bus_rwlock/registered_handlers/10 | 35.33 ns | 35.39 ns | 0.26 ns | [35.23, 35.41] ns |
| event_bus_rwlock/registered_handlers/100 | 135.56 ns | 135.49 ns | 1.04 ns | [135.15, 135.87] ns |
| event_bus_rwlock/registered_handlers/50 | 79.50 ns | 79.41 ns | 0.41 ns | [79.37, 79.66] ns |
| event_bus_subscribe | 177.58 ns | 177.69 ns | 0.97 ns | [177.24, 177.92] ns |
| frame_operations/buffer_fill_6000_cells | 29.35 µs | 29.35 µs | 0.11 µs | [29.32, 29.39] µs |
| frame_operations/flush_full_screen | 51.53 µs | 51.06 µs | 1.32 µs | [51.12, 52.04] µs |
| frame_operations/flush_scroll_simulation | 19.09 µs | 19.29 µs | 0.71 µs | [18.83, 19.33] µs |
| large_file/5000_lines_20_j_presses | 1.88 ms | 1.90 ms | 0.06 ms | [1.86, 1.90] ms |
| large_file/5000_lines_render_data | 84.10 µs | 84.10 µs | 0.12 µs | [84.05, 84.14] µs |
| render_data_changelog/scroll_pos/0 | 118.77 µs | 118.39 µs | 0.97 µs | [118.43, 119.12] µs |
| render_data_changelog/scroll_pos/100 | 119.05 µs | 118.90 µs | 0.98 µs | [118.72, 119.41] µs |
| render_data_changelog/scroll_pos/1000 | 108.05 µs | 106.78 µs | 2.39 µs | [107.26, 108.94] µs |
| render_data_changelog/scroll_pos/1500 | 120.28 µs | 118.85 µs | 3.77 µs | [119.12, 121.74] µs |
| render_data_changelog/scroll_pos/500 | 121.78 µs | 121.12 µs | 2.30 µs | [121.02, 122.63] µs |
| render_data_jjjjj/20_j_presses | 414.22 µs | 415.99 µs | 7.48 µs | [411.17, 416.31] µs |
| render_data_jjjjj/50_j_presses | 1.05 ms | 1.05 ms | 0.01 ms | [1.05, 1.05] ms |
| render_data_real/scroll_pos/0 | 19.10 µs | 19.12 µs | 0.09 µs | [19.07, 19.14] µs |
| render_data_real/scroll_pos/150 | 21.31 µs | 21.30 µs | 0.03 µs | [21.30, 21.32] µs |
| render_data_real/scroll_pos/300 | 18.93 µs | 18.93 µs | 0.10 µs | [18.90, 18.97] µs |
| render_data_real/scroll_pos/50 | 19.01 µs | 19.00 µs | 0.10 µs | [18.98, 19.05] µs |
| render_data_syntax_overhead/with_syntax | 21.03 µs | 20.92 µs | 0.24 µs | [20.95, 21.12] µs |
| render_data_syntax_overhead/without_syntax | 18.86 µs | 18.80 µs | 0.11 µs | [18.82, 18.90] µs |
| throughput/renders_per_second | 24.98 µs | 25.02 µs | 0.19 µs | [24.92, 25.05] µs |
| treesitter_query_compile/highlights/bash | 1.97 ms | 1.96 ms | 0.02 ms | [1.96, 1.98] ms |
| treesitter_query_compile/highlights/c | 2.70 ms | 2.69 ms | 0.02 ms | [2.69, 2.70] ms |
| treesitter_query_compile/highlights/javascript | 3.49 ms | 3.47 ms | 0.05 ms | [3.47, 3.51] ms |
| treesitter_query_compile/highlights/json | 6.85 µs | 6.85 µs | 0.01 µs | [6.85, 6.86] µs |
| treesitter_query_compile/highlights/markdown | 1.03 ms | 1.03 ms | 0.01 ms | [1.03, 1.03] ms |
| treesitter_query_compile/highlights/python | 3.61 ms | 3.67 ms | 0.14 ms | [3.56, 3.65] ms |
| treesitter_query_compile/highlights/rust | 23.06 ms | 23.04 ms | 0.14 ms | [23.01, 23.11] ms |
| treesitter_query_compile/highlights/toml | 44.96 µs | 44.97 µs | 0.29 µs | [44.86, 45.06] µs |
| treesitter_register_all_languages | 78.46 ms | 79.48 ms | 3.44 ms | [77.17, 79.60] ms |
| treesitter_register_language/c | 8.46 ms | 8.12 ms | 0.56 ms | [8.27, 8.66] ms |
| treesitter_register_language/javascript | 8.11 ms | 7.99 ms | 0.52 ms | [7.93, 8.30] ms |
| treesitter_register_language/python | 8.17 ms | 8.13 ms | 0.13 ms | [8.13, 8.22] ms |
| treesitter_register_language/rust | 33.81 ms | 33.78 ms | 0.20 ms | [33.74, 33.88] ms |
| viewport_size/height/100 | 49.76 µs | 50.09 µs | 0.85 µs | [49.42, 50.01] µs |
| viewport_size/height/200 | 97.19 µs | 97.66 µs | 4.06 µs | [95.77, 98.65] µs |
| viewport_size/height/24 | 10.52 µs | 10.50 µs | 0.07 µs | [10.50, 10.54] µs |
| viewport_size/height/50 | 23.75 µs | 22.86 µs | 1.31 µs | [23.30, 24.21] µs |
| window_render/buffer_lines/10 | 5.02 µs | 5.00 µs | 0.05 µs | [5.00, 5.04] µs |
| window_render/buffer_lines/100 | 24.14 µs | 24.10 µs | 0.46 µs | [24.02, 24.33] µs |
| window_render/buffer_lines/1000 | 24.57 µs | 24.38 µs | 0.53 µs | [24.41, 24.78] µs |
| window_render/buffer_lines/10000 | 22.47 µs | 22.46 µs | 0.09 µs | [22.44, 22.51] µs |
| with_highlights/no_highlights | 25.11 µs | 24.80 µs | 0.73 µs | [24.87, 25.39] µs |
