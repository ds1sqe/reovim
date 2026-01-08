# Performance Benchmarks v0.8.0-dev-priority-channels

> Last updated: 2026-01-08T15:17:55Z | Commit: 2376a8e | Rust: 1.94.0-nightly

## Metadata

```toml
[metadata]
version = "0.8.0-dev-priority-channels"
commit = "2376a8e"
date = "2026-01-08T15:17:55Z"
rust_version = "1.94.0-nightly"
os = "Linux 6.17.9-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| complete_cycle | 1 | 54.47 µs |
| event_bus_bottleneck_blocking | 4 | 29.06 µs |
| event_bus_bottleneck_comparison | 2 | 26.95 ns |
| event_bus_bottleneck_queue | 4 | 1.00 ms |
| event_bus_bottleneck_sequential | 3 | 7.67 ms |
| event_bus_dispatch | 7 | 120.47 ns |
| event_bus_dispatch_consumed | 4 | 27.68 ns |
| event_bus_dispatch_emit | 4 | 90.50 ns |
| event_bus_dispatch_multi_type | 4 | 27.80 ns |
| event_bus_dispatch_priority | 3 | 63.34 ns |
| event_bus_dyn_event_new | 1 | 8.52 ns |
| event_bus_rwlock | 4 | 69.57 ns |
| event_bus_subscribe | 1 | 159.77 ns |
| frame_operations | 3 | 34.83 µs |
| large_file | 2 | 49.71 ms |
| render_data_changelog | 3 | 32.45 µs |
| render_data_jjjjj | 2 | 202.18 µs |
| render_data_real | 4 | 20.25 µs |
| render_data_syntax_overhead | 2 | 20.39 µs |
| throughput | 1 | 25.86 µs |
| treesitter_query_compile | 8 | 11.82 ms |
| treesitter_register_all_languages | 1 | 11.36 µs |
| treesitter_register_language | 4 | 9.35 µs |
| viewport_size | 4 | 49.29 µs |
| window_render | 4 | 20.38 µs |
| with_highlights | 1 | 25.58 µs |

## Results

```toml
[results.complete_cycle]
full_scroll_cycle = { mean = 54.47, median = 50.94, std_dev = 10.87, unit = "µs" }

[results.event_bus_bottleneck_blocking]
block_us_100 = { mean = 100.25, median = 100.11, std_dev = 0.66, unit = "µs" }
block_us_1000 = { mean = 1.00, median = 1.00, std_dev = 0.00, unit = "ms" }
block_us_10000 = { mean = 10.00, median = 10.00, std_dev = 0.00, unit = "ms" }
block_us_5000 = { mean = 5.00, median = 5.00, std_dev = 0.00, unit = "ms" }

[results.event_bus_bottleneck_comparison]
fast_only = { mean = 27.59, median = 26.48, std_dev = 1.71, unit = "ns" }
fast_with_slow_registered = { mean = 26.32, median = 26.28, std_dev = 0.15, unit = "ns" }

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
handlers_0 = { mean = 10.79, median = 10.17, std_dev = 1.92, unit = "ns" }
handlers_1 = { mean = 27.14, median = 26.74, std_dev = 1.58, unit = "ns" }
handlers_10 = { mean = 57.30, median = 57.07, std_dev = 1.40, unit = "ns" }
handlers_100 = { mean = 406.07, median = 392.28, std_dev = 28.70, unit = "ns" }
handlers_20 = { mean = 100.56, median = 91.32, std_dev = 21.28, unit = "ns" }
handlers_5 = { mean = 41.10, median = 40.63, std_dev = 1.50, unit = "ns" }
handlers_50 = { mean = 200.30, median = 200.31, std_dev = 0.42, unit = "ns" }

[results.event_bus_dispatch_consumed]
total_handlers_10 = { mean = 28.87, median = 28.65, std_dev = 2.33, unit = "ns" }
total_handlers_20 = { mean = 26.56, median = 26.54, std_dev = 0.12, unit = "ns" }
total_handlers_5 = { mean = 26.97, median = 26.92, std_dev = 0.43, unit = "ns" }
total_handlers_50 = { mean = 28.33, median = 26.67, std_dev = 2.67, unit = "ns" }

[results.event_bus_dispatch_emit]
emits_0 = { mean = 29.84, median = 29.29, std_dev = 2.78, unit = "ns" }
emits_1 = { mean = 46.73, median = 44.23, std_dev = 6.63, unit = "ns" }
emits_10 = { mean = 185.18, median = 174.53, std_dev = 20.65, unit = "ns" }
emits_5 = { mean = 100.25, median = 100.16, std_dev = 0.45, unit = "ns" }

[results.event_bus_dispatch_multi_type]
event_types_1 = { mean = 28.80, median = 26.91, std_dev = 4.05, unit = "ns" }
event_types_10 = { mean = 26.89, median = 26.63, std_dev = 0.66, unit = "ns" }
event_types_20 = { mean = 28.68, median = 26.88, std_dev = 4.89, unit = "ns" }
event_types_5 = { mean = 26.84, median = 26.57, std_dev = 0.79, unit = "ns" }

[results.event_bus_dispatch_priority]
handlers_10 = { mean = 56.99, median = 56.43, std_dev = 2.32, unit = "ns" }
handlers_20 = { mean = 91.83, median = 90.51, std_dev = 3.58, unit = "ns" }
handlers_5 = { mean = 41.20, median = 40.43, std_dev = 1.93, unit = "ns" }

[results.event_bus_dyn_event_new]
event_bus_dyn_event_new = { mean = 8.52, median = 8.33, std_dev = 0.59, unit = "ns" }

[results.event_bus_rwlock]
registered_handlers_1 = { mean = 26.69, median = 25.91, std_dev = 1.90, unit = "ns" }
registered_handlers_10 = { mean = 38.82, median = 33.75, std_dev = 10.33, unit = "ns" }
registered_handlers_100 = { mean = 133.35, median = 133.18, std_dev = 1.11, unit = "ns" }
registered_handlers_50 = { mean = 79.43, median = 75.38, std_dev = 7.73, unit = "ns" }

[results.event_bus_subscribe]
event_bus_subscribe = { mean = 159.77, median = 159.05, std_dev = 3.21, unit = "ns" }

[results.frame_operations]
buffer_fill_6000_cells = { mean = 30.71, median = 28.59, std_dev = 6.04, unit = "µs" }
flush_full_screen = { mean = 52.93, median = 48.39, std_dev = 10.54, unit = "µs" }
flush_scroll_simulation = { mean = 20.84, median = 19.13, std_dev = 4.88, unit = "µs" }

[results.large_file]
5000_lines_20_j_presses = { mean = 1.76, median = 1.75, std_dev = 0.04, unit = "ms" }
5000_lines_render_data = { mean = 97.65, median = 89.99, std_dev = 15.04, unit = "µs" }

[results.render_data_changelog]
scroll_pos_0 = { mean = 30.49, median = 30.16, std_dev = 0.98, unit = "µs" }
scroll_pos_100 = { mean = 33.99, median = 31.56, std_dev = 3.59, unit = "µs" }
scroll_pos_500 = { mean = 32.85, median = 30.87, std_dev = 3.17, unit = "µs" }

[results.render_data_jjjjj]
20_j_presses = { mean = 403.36, median = 384.37, std_dev = 49.95, unit = "µs" }
50_j_presses = { mean = 1.01, median = 0.97, std_dev = 0.14, unit = "ms" }

[results.render_data_real]
scroll_pos_0 = { mean = 19.92, median = 19.54, std_dev = 1.88, unit = "µs" }
scroll_pos_150 = { mean = 19.83, median = 19.66, std_dev = 0.73, unit = "µs" }
scroll_pos_300 = { mean = 21.34, median = 19.65, std_dev = 3.23, unit = "µs" }
scroll_pos_50 = { mean = 19.90, median = 19.61, std_dev = 1.02, unit = "µs" }

[results.render_data_syntax_overhead]
with_syntax = { mean = 20.37, median = 19.34, std_dev = 3.22, unit = "µs" }
without_syntax = { mean = 20.41, median = 19.42, std_dev = 2.99, unit = "µs" }

[results.throughput]
renders_per_second = { mean = 25.86, median = 23.94, std_dev = 4.95, unit = "µs" }

[results.treesitter_query_compile]
highlights_bash = { mean = 2.00, median = 1.99, std_dev = 0.05, unit = "ms" }
highlights_c = { mean = 2.87, median = 2.76, std_dev = 0.22, unit = "ms" }
highlights_javascript = { mean = 3.45, median = 3.18, std_dev = 0.66, unit = "ms" }
highlights_json = { mean = 7.94, median = 8.20, std_dev = 0.82, unit = "µs" }
highlights_markdown = { mean = 1.15, median = 1.06, std_dev = 0.20, unit = "ms" }
highlights_python = { mean = 3.39, median = 3.37, std_dev = 0.07, unit = "ms" }
highlights_rust = { mean = 23.77, median = 23.79, std_dev = 0.40, unit = "ms" }
highlights_toml = { mean = 49.97, median = 45.35, std_dev = 9.89, unit = "µs" }

[results.treesitter_register_all_languages]
treesitter_register_all_languages = { mean = 11.36, median = 11.36, std_dev = 0.05, unit = "µs" }

[results.treesitter_register_language]
c = { mean = 9.42, median = 9.45, std_dev = 0.69, unit = "µs" }
javascript = { mean = 9.84, median = 9.71, std_dev = 0.63, unit = "µs" }
python = { mean = 9.42, median = 9.49, std_dev = 0.24, unit = "µs" }
rust = { mean = 8.70, median = 8.44, std_dev = 0.70, unit = "µs" }

[results.viewport_size]
height_100 = { mean = 51.59, median = 47.73, std_dev = 10.41, unit = "µs" }
height_200 = { mean = 107.65, median = 94.84, std_dev = 23.04, unit = "µs" }
height_24 = { mean = 12.23, median = 11.21, std_dev = 2.66, unit = "µs" }
height_50 = { mean = 25.71, median = 24.34, std_dev = 4.58, unit = "µs" }

[results.window_render]
buffer_lines_10 = { mean = 5.11, median = 4.74, std_dev = 0.90, unit = "µs" }
buffer_lines_100 = { mean = 26.02, median = 23.79, std_dev = 4.75, unit = "µs" }
buffer_lines_1000 = { mean = 25.03, median = 23.51, std_dev = 4.74, unit = "µs" }
buffer_lines_10000 = { mean = 25.37, median = 23.33, std_dev = 5.65, unit = "µs" }

[results.with_highlights]
no_highlights = { mean = 25.58, median = 24.20, std_dev = 4.84, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| complete_cycle/full_scroll_cycle | 54.47 µs | 50.94 µs | 10.87 µs | [51.04, 58.72] µs |
| event_bus_bottleneck_blocking/block_us/100 | 100.25 µs | 100.11 µs | 0.66 µs | [100.12, 100.50] µs |
| event_bus_bottleneck_blocking/block_us/1000 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_blocking/block_us/10000 | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_blocking/block_us/5000 | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_comparison/fast_only | 27.59 ns | 26.48 ns | 1.71 ns | [27.01, 28.20] ns |
| event_bus_bottleneck_comparison/fast_with_slow_registered | 26.32 ns | 26.28 ns | 0.15 ns | [26.27, 26.38] ns |
| event_bus_bottleneck_queue/queued_events/1 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/10 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/20 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_queue/queued_events/5 | 1.00 ms | 1.00 ms | 0.00 ms | [1.00, 1.00] ms |
| event_bus_bottleneck_sequential/langs/1x5000us | 5.00 ms | 5.00 ms | 0.00 ms | [5.00, 5.00] ms |
| event_bus_bottleneck_sequential/langs/4x2500us | 10.00 ms | 10.00 ms | 0.00 ms | [10.00, 10.00] ms |
| event_bus_bottleneck_sequential/langs/8x1000us | 8.00 ms | 8.00 ms | 0.00 ms | [8.00, 8.00] ms |
| event_bus_dispatch/handlers/0 | 10.79 ns | 10.17 ns | 1.92 ns | [10.25, 11.57] ns |
| event_bus_dispatch/handlers/1 | 27.14 ns | 26.74 ns | 1.58 ns | [26.69, 27.77] ns |
| event_bus_dispatch/handlers/10 | 57.30 ns | 57.07 ns | 1.40 ns | [56.85, 57.83] ns |
| event_bus_dispatch/handlers/100 | 406.07 ns | 392.28 ns | 28.70 ns | [396.84, 417.01] ns |
| event_bus_dispatch/handlers/20 | 100.56 ns | 91.32 ns | 21.28 ns | [93.84, 108.67] ns |
| event_bus_dispatch/handlers/5 | 41.10 ns | 40.63 ns | 1.50 ns | [40.64, 41.68] ns |
| event_bus_dispatch/handlers/50 | 200.30 ns | 200.31 ns | 0.42 ns | [200.16, 200.45] ns |
| event_bus_dispatch_consumed/total_handlers/10 | 28.87 ns | 28.65 ns | 2.33 ns | [28.10, 29.73] ns |
| event_bus_dispatch_consumed/total_handlers/20 | 26.56 ns | 26.54 ns | 0.12 ns | [26.52, 26.61] ns |
| event_bus_dispatch_consumed/total_handlers/5 | 26.97 ns | 26.92 ns | 0.43 ns | [26.83, 27.13] ns |
| event_bus_dispatch_consumed/total_handlers/50 | 28.33 ns | 26.67 ns | 2.67 ns | [27.44, 29.32] ns |
| event_bus_dispatch_emit/emits/0 | 29.84 ns | 29.29 ns | 2.78 ns | [28.91, 30.86] ns |
| event_bus_dispatch_emit/emits/1 | 46.73 ns | 44.23 ns | 6.63 ns | [44.72, 49.28] ns |
| event_bus_dispatch_emit/emits/10 | 185.18 ns | 174.53 ns | 20.65 ns | [178.46, 192.92] ns |
| event_bus_dispatch_emit/emits/5 | 100.25 ns | 100.16 ns | 0.45 ns | [100.11, 100.43] ns |
| event_bus_dispatch_multi_type/event_types/1 | 28.80 ns | 26.91 ns | 4.05 ns | [27.57, 30.38] ns |
| event_bus_dispatch_multi_type/event_types/10 | 26.89 ns | 26.63 ns | 0.66 ns | [26.67, 27.14] ns |
| event_bus_dispatch_multi_type/event_types/20 | 28.68 ns | 26.88 ns | 4.89 ns | [27.14, 30.60] ns |
| event_bus_dispatch_multi_type/event_types/5 | 26.84 ns | 26.57 ns | 0.79 ns | [26.61, 27.16] ns |
| event_bus_dispatch_priority/handlers/10 | 56.99 ns | 56.43 ns | 2.32 ns | [56.45, 57.91] ns |
| event_bus_dispatch_priority/handlers/20 | 91.83 ns | 90.51 ns | 3.58 ns | [90.71, 93.21] ns |
| event_bus_dispatch_priority/handlers/5 | 41.20 ns | 40.43 ns | 1.93 ns | [40.61, 41.94] ns |
| event_bus_dyn_event_new | 8.52 ns | 8.33 ns | 0.59 ns | [8.35, 8.75] ns |
| event_bus_rwlock/registered_handlers/1 | 26.69 ns | 25.91 ns | 1.90 ns | [26.08, 27.41] ns |
| event_bus_rwlock/registered_handlers/10 | 38.82 ns | 33.75 ns | 10.33 ns | [35.45, 42.71] ns |
| event_bus_rwlock/registered_handlers/100 | 133.35 ns | 133.18 ns | 1.11 ns | [132.97, 133.75] ns |
| event_bus_rwlock/registered_handlers/50 | 79.43 ns | 75.38 ns | 7.73 ns | [76.86, 82.28] ns |
| event_bus_subscribe | 159.77 ns | 159.05 ns | 3.21 ns | [158.79, 161.02] ns |
| frame_operations/buffer_fill_6000_cells | 30.71 µs | 28.59 µs | 6.04 µs | [28.85, 33.09] µs |
| frame_operations/flush_full_screen | 52.93 µs | 48.39 µs | 10.54 µs | [49.71, 57.08] µs |
| frame_operations/flush_scroll_simulation | 20.84 µs | 19.13 µs | 4.88 µs | [19.39, 22.77] µs |
| large_file/5000_lines_20_j_presses | 1.76 ms | 1.75 ms | 0.04 ms | [1.75, 1.78] ms |
| large_file/5000_lines_render_data | 97.65 µs | 89.99 µs | 15.04 µs | [92.72, 103.31] µs |
| render_data_changelog/scroll_pos/0 | 30.49 µs | 30.16 µs | 0.98 µs | [30.20, 30.88] µs |
| render_data_changelog/scroll_pos/100 | 33.99 µs | 31.56 µs | 3.59 µs | [32.76, 35.26] µs |
| render_data_changelog/scroll_pos/500 | 32.85 µs | 30.87 µs | 3.17 µs | [31.77, 34.00] µs |
| render_data_jjjjj/20_j_presses | 403.36 µs | 384.37 µs | 49.95 µs | [388.20, 423.07] µs |
| render_data_jjjjj/50_j_presses | 1.01 ms | 0.97 ms | 0.14 ms | [0.97, 1.07] ms |
| render_data_real/scroll_pos/0 | 19.92 µs | 19.54 µs | 1.88 µs | [19.52, 20.64] µs |
| render_data_real/scroll_pos/150 | 19.83 µs | 19.66 µs | 0.73 µs | [19.61, 20.12] µs |
| render_data_real/scroll_pos/300 | 21.34 µs | 19.65 µs | 3.23 µs | [20.31, 22.56] µs |
| render_data_real/scroll_pos/50 | 19.90 µs | 19.61 µs | 1.02 µs | [19.61, 20.31] µs |
| render_data_syntax_overhead/with_syntax | 20.37 µs | 19.34 µs | 3.22 µs | [19.36, 21.61] µs |
| render_data_syntax_overhead/without_syntax | 20.41 µs | 19.42 µs | 2.99 µs | [19.45, 21.60] µs |
| throughput/renders_per_second | 25.86 µs | 23.94 µs | 4.95 µs | [24.36, 27.80] µs |
| treesitter_query_compile/highlights/bash | 2.00 ms | 1.99 ms | 0.05 ms | [1.99, 2.02] ms |
| treesitter_query_compile/highlights/c | 2.87 ms | 2.76 ms | 0.22 ms | [2.80, 2.95] ms |
| treesitter_query_compile/highlights/javascript | 3.45 ms | 3.18 ms | 0.66 ms | [3.25, 3.71] ms |
| treesitter_query_compile/highlights/json | 7.94 µs | 8.20 µs | 0.82 µs | [7.65, 8.23] µs |
| treesitter_query_compile/highlights/markdown | 1.15 ms | 1.06 ms | 0.20 ms | [1.08, 1.22] ms |
| treesitter_query_compile/highlights/python | 3.39 ms | 3.37 ms | 0.07 ms | [3.37, 3.42] ms |
| treesitter_query_compile/highlights/rust | 23.77 ms | 23.79 ms | 0.40 ms | [23.64, 23.92] ms |
| treesitter_query_compile/highlights/toml | 49.97 µs | 45.35 µs | 9.89 µs | [46.88, 53.75] µs |
| treesitter_register_all_languages | 11.36 µs | 11.36 µs | 0.05 µs | [11.34, 11.38] µs |
| treesitter_register_language/c | 9.42 µs | 9.45 µs | 0.69 µs | [9.18, 9.67] µs |
| treesitter_register_language/javascript | 9.84 µs | 9.71 µs | 0.63 µs | [9.64, 10.09] µs |
| treesitter_register_language/python | 9.42 µs | 9.49 µs | 0.24 µs | [9.33, 9.49] µs |
| treesitter_register_language/rust | 8.70 µs | 8.44 µs | 0.70 µs | [8.49, 8.97] µs |
| viewport_size/height/100 | 51.59 µs | 47.73 µs | 10.41 µs | [48.32, 55.68] µs |
| viewport_size/height/200 | 107.65 µs | 94.84 µs | 23.04 µs | [100.15, 116.23] µs |
| viewport_size/height/24 | 12.23 µs | 11.21 µs | 2.66 µs | [11.40, 13.26] µs |
| viewport_size/height/50 | 25.71 µs | 24.34 µs | 4.58 µs | [24.28, 27.50] µs |
| window_render/buffer_lines/10 | 5.11 µs | 4.74 µs | 0.90 µs | [4.84, 5.46] µs |
| window_render/buffer_lines/100 | 26.02 µs | 23.79 µs | 4.75 µs | [24.55, 27.89] µs |
| window_render/buffer_lines/1000 | 25.03 µs | 23.51 µs | 4.74 µs | [23.54, 26.87] µs |
| window_render/buffer_lines/10000 | 25.37 µs | 23.33 µs | 5.65 µs | [23.65, 27.61] µs |
| with_highlights/no_highlights | 25.58 µs | 24.20 µs | 4.84 µs | [24.23, 27.50] µs |
