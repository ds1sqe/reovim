# Performance Benchmarks v0.14.3

> Last updated: 2026-03-30T07:22:17Z | Commit: f2c3eff9 | Rust: 1.96.0-nightly

## Metadata

```toml
[metadata]
version = "0.14.3"
commit = "f2c3eff9"
date = "2026-03-30T07:22:17Z"
rust_version = "1.96.0-nightly"
os = "Linux 6.19.6-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| bridge_detection_fixed | 4 | 12.21 ns |
| bridge_detection_loop | 4 | 15.73 ns |
| bridge_json_parse | 3 | 22.34 µs |
| bridge_json_serialize | 9 | 21.00 µs |
| bridge_unnecessary_per_keystroke | 3 | 5.10 µs |
| buffer_byte_to_position | 4 | 190.25 ns |
| buffer_clone | 4 | 219.64 µs |
| buffer_content | 4 | 304.55 ns |
| buffer_delete_char | 4 | 442.08 ns |
| buffer_delete_range | 4 | 202.69 ns |
| buffer_from_string | 4 | 31.64 µs |
| buffer_insert_char | 4 | 324.26 ns |
| buffer_insert_line | 4 | 275.85 ns |
| buffer_insert_word | 4 | 315.54 ns |
| buffer_line_access | 4 | 0.05 ns |
| buffer_line_hashes | 4 | 97.92 µs |
| buffer_position_to_byte | 4 | 253.56 ns |
| buffer_vec_char_alloc | 3 | 157.50 µs |
| event_dispatch | 5 | 226.31 ns |
| event_dispatch_0_handlers | 1 | 38.34 ns |
| event_dispatch_multi_type | 1 | 195.63 ns |
| event_dispatch_propagate | 3 | 85.86 ns |
| event_subscribe | 1 | 805.81 ns |
| metrics_counter_get_and_increment | 1 | 70.75 ns |
| metrics_histogram_get_and_record | 1 | 88.61 ns |
| profiler_counter | 1 | 3.41 ns |
| profiler_enabled_check | 1 | 3.36 ns |
| profiler_guard_legacy | 1 | 142.16 ns |
| profiler_histogram | 1 | 3.37 ns |
| profiler_nested_scopes_3 | 1 | 86.57 ns |
| profiler_scope_nop | 1 | 28.88 ns |
| profiler_span_data_new | 1 | 42.09 ns |
| profiler_span_id_new | 1 | 6.73 ns |
| search_backward | 6 | 107.69 µs |
| search_content_materialization | 3 | 45.32 µs |
| search_full | 6 | 50.01 µs |
| search_regex_compile | 4 | 180.01 ns |
| syntax_edit_cycle_current | 4 | 25.83 ms |
| syntax_edit_cycle_optimized | 4 | 73.57 µs |
| syntax_full_parse | 4 | 194.65 µs |
| syntax_highlights_edit_region | 4 | 71.99 µs |
| syntax_highlights_full_file | 4 | 196.78 µs |
| syntax_highlights_viewport | 4 | 393.26 µs |
| syntax_incremental_update | 4 | 40.27 µs |
| syntax_token_span_conversion | 4 | 223.80 µs |

## Results

```toml
[results.bridge_detection_fixed]
bridges_10 = { mean = 7.40, median = 7.39, std_dev = 0.03, unit = "ns" }
bridges_16 = { mean = 9.32, median = 9.33, std_dev = 0.02, unit = "ns" }
bridges_22 = { mean = 14.20, median = 14.19, std_dev = 0.04, unit = "ns" }
bridges_30 = { mean = 17.93, median = 17.92, std_dev = 0.08, unit = "ns" }

[results.bridge_detection_loop]
bridges_10 = { mean = 10.09, median = 10.08, std_dev = 0.04, unit = "ns" }
bridges_16 = { mean = 12.04, median = 12.04, std_dev = 0.03, unit = "ns" }
bridges_22 = { mean = 17.94, median = 17.93, std_dev = 0.02, unit = "ns" }
bridges_30 = { mean = 22.83, median = 22.82, std_dev = 0.05, unit = "ns" }

[results.bridge_json_parse]
bufferline_buffers_10 = { mean = 13.99, median = 13.97, std_dev = 0.06, unit = "µs" }
bufferline_buffers_3 = { mean = 3.85, median = 3.85, std_dev = 0.01, unit = "µs" }
bufferline_buffers_30 = { mean = 49.18, median = 49.14, std_dev = 0.19, unit = "µs" }

[results.bridge_json_serialize]
bufferline_buffers_10 = { mean = 7.67, median = 7.65, std_dev = 0.08, unit = "µs" }
bufferline_buffers_3 = { mean = 2.66, median = 2.66, std_dev = 0.01, unit = "µs" }
bufferline_buffers_30 = { mean = 22.15, median = 22.02, std_dev = 0.77, unit = "µs" }
diagnostics_items_10 = { mean = 6.37, median = 6.24, std_dev = 0.81, unit = "µs" }
diagnostics_items_200 = { mean = 92.03, median = 90.68, std_dev = 4.72, unit = "µs" }
diagnostics_items_50 = { mean = 25.13, median = 23.68, std_dev = 3.52, unit = "µs" }
illuminate_ranges_20 = { mean = 9.01, median = 9.01, std_dev = 0.01, unit = "µs" }
illuminate_ranges_5 = { mean = 2.82, median = 2.81, std_dev = 0.03, unit = "µs" }
illuminate_ranges_50 = { mean = 21.13, median = 21.12, std_dev = 0.04, unit = "µs" }

[results.bridge_unnecessary_per_keystroke]
active_1 = { mean = 4.16, median = 4.16, std_dev = 0.01, unit = "µs" }
active_3 = { mean = 5.09, median = 5.08, std_dev = 0.03, unit = "µs" }
active_5 = { mean = 6.05, median = 6.05, std_dev = 0.01, unit = "µs" }

[results.buffer_byte_to_position]
lines_100 = { mean = 74.79, median = 74.71, std_dev = 0.22, unit = "ns" }
lines_1000 = { mean = 599.05, median = 597.71, std_dev = 5.54, unit = "ns" }
lines_10000 = { mean = 6.56, median = 6.56, std_dev = 0.01, unit = "µs" }
lines_100000 = { mean = 80.61, median = 80.70, std_dev = 0.67, unit = "µs" }

[results.buffer_clone]
lines_100 = { mean = 7.06, median = 7.05, std_dev = 0.02, unit = "µs" }
lines_1000 = { mean = 76.81, median = 76.71, std_dev = 0.27, unit = "µs" }
lines_10000 = { mean = 786.80, median = 785.37, std_dev = 8.61, unit = "µs" }
lines_100000 = { mean = 7.92, median = 7.90, std_dev = 0.09, unit = "ms" }

[results.buffer_content]
lines_100 = { mean = 463.67, median = 463.29, std_dev = 1.85, unit = "ns" }
lines_1000 = { mean = 4.80, median = 4.80, std_dev = 0.02, unit = "µs" }
lines_10000 = { mean = 57.93, median = 57.93, std_dev = 0.13, unit = "µs" }
lines_100000 = { mean = 691.78, median = 690.67, std_dev = 4.74, unit = "µs" }

[results.buffer_delete_char]
lines_100 = { mean = 462.76, median = 457.22, std_dev = 19.77, unit = "ns" }
lines_1000 = { mean = 520.37, median = 505.12, std_dev = 57.30, unit = "ns" }
lines_10000 = { mean = 783.90, median = 761.75, std_dev = 83.02, unit = "ns" }
lines_100000 = { mean = 1.30, median = 1.27, std_dev = 0.17, unit = "µs" }

[results.buffer_delete_range]
lines_100 = { mean = 751.24, median = 698.95, std_dev = 546.23, unit = "ns" }
lines_1000 = { mean = 1.08, median = 1.10, std_dev = 0.07, unit = "µs" }
lines_10000 = { mean = 5.51, median = 5.47, std_dev = 0.11, unit = "µs" }
lines_100000 = { mean = 52.94, median = 52.56, std_dev = 0.95, unit = "µs" }

[results.buffer_from_string]
lines_100 = { mean = 10.40, median = 10.10, std_dev = 0.75, unit = "µs" }
lines_1000 = { mean = 105.10, median = 104.97, std_dev = 0.56, unit = "µs" }
lines_10000 = { mean = 1.01, median = 1.01, std_dev = 0.00, unit = "ms" }
lines_100000 = { mean = 10.07, median = 10.02, std_dev = 0.18, unit = "ms" }

[results.buffer_insert_char]
lines_100 = { mean = 307.07, median = 307.01, std_dev = 15.20, unit = "ns" }
lines_1000 = { mean = 370.03, median = 358.13, std_dev = 53.65, unit = "ns" }
lines_10000 = { mean = 618.48, median = 580.55, std_dev = 99.57, unit = "ns" }
lines_100000 = { mean = 1.44, median = 1.41, std_dev = 0.28, unit = "µs" }

[results.buffer_insert_line]
lines_100 = { mean = 343.10, median = 349.46, std_dev = 28.85, unit = "ns" }
lines_1000 = { mean = 697.45, median = 724.34, std_dev = 87.06, unit = "ns" }
lines_10000 = { mean = 4.90, median = 4.87, std_dev = 0.16, unit = "µs" }
lines_100000 = { mean = 57.94, median = 55.13, std_dev = 10.85, unit = "µs" }

[results.buffer_insert_word]
lines_100 = { mean = 323.14, median = 323.96, std_dev = 15.27, unit = "ns" }
lines_1000 = { mean = 375.08, median = 358.04, std_dev = 62.58, unit = "ns" }
lines_10000 = { mean = 562.53, median = 555.57, std_dev = 38.81, unit = "ns" }
lines_100000 = { mean = 1.39, median = 1.24, std_dev = 0.45, unit = "µs" }

[results.buffer_line_access]
lines_100 = { mean = 0.05, median = 0.05, std_dev = 0.00, unit = "ns" }
lines_1000 = { mean = 0.05, median = 0.05, std_dev = 0.00, unit = "ns" }
lines_10000 = { mean = 0.05, median = 0.05, std_dev = 0.00, unit = "ns" }
lines_100000 = { mean = 0.05, median = 0.05, std_dev = 0.00, unit = "ns" }

[results.buffer_line_hashes]
lines_100 = { mean = 3.60, median = 3.60, std_dev = 0.01, unit = "µs" }
lines_1000 = { mean = 32.81, median = 32.80, std_dev = 0.34, unit = "µs" }
lines_10000 = { mean = 352.05, median = 329.23, std_dev = 43.59, unit = "µs" }
lines_100000 = { mean = 3.22, median = 3.19, std_dev = 0.12, unit = "ms" }

[results.buffer_position_to_byte]
lines_100 = { mean = 111.88, median = 110.65, std_dev = 4.23, unit = "ns" }
lines_1000 = { mean = 797.59, median = 783.38, std_dev = 116.43, unit = "ns" }
lines_10000 = { mean = 7.61, median = 7.55, std_dev = 0.09, unit = "µs" }
lines_100000 = { mean = 97.15, median = 98.11, std_dev = 1.83, unit = "µs" }

[results.buffer_vec_char_alloc]
1000_chars = { mean = 1.21, median = 1.20, std_dev = 0.03, unit = "µs" }
11_chars = { mean = 182.67, median = 182.60, std_dev = 0.64, unit = "ns" }
99_chars = { mean = 288.63, median = 288.06, std_dev = 4.47, unit = "ns" }

[results.event_dispatch]
handlers_1 = { mean = 65.00, median = 64.57, std_dev = 0.81, unit = "ns" }
handlers_10 = { mean = 108.91, median = 108.59, std_dev = 2.18, unit = "ns" }
handlers_100 = { mean = 556.95, median = 555.98, std_dev = 3.51, unit = "ns" }
handlers_5 = { mean = 87.73, median = 88.12, std_dev = 2.59, unit = "ns" }
handlers_50 = { mean = 312.97, median = 312.63, std_dev = 1.58, unit = "ns" }

[results.event_dispatch_0_handlers]
event_dispatch_0_handlers = { mean = 38.34, median = 38.20, std_dev = 0.59, unit = "ns" }

[results.event_dispatch_multi_type]
event_dispatch_multi_type = { mean = 195.63, median = 195.13, std_dev = 2.27, unit = "ns" }

[results.event_dispatch_propagate]
handlers_1 = { mean = 63.82, median = 63.52, std_dev = 0.57, unit = "ns" }
handlers_10 = { mean = 108.89, median = 108.76, std_dev = 0.57, unit = "ns" }
handlers_5 = { mean = 84.88, median = 84.03, std_dev = 3.72, unit = "ns" }

[results.event_subscribe]
event_subscribe = { mean = 805.81, median = 797.78, std_dev = 30.63, unit = "ns" }

[results.metrics_counter_get_and_increment]
metrics_counter_get_and_increment = { mean = 70.75, median = 67.61, std_dev = 5.67, unit = "ns" }

[results.metrics_histogram_get_and_record]
metrics_histogram_get_and_record = { mean = 88.61, median = 86.14, std_dev = 6.24, unit = "ns" }

[results.profiler_counter]
profiler_counter = { mean = 3.41, median = 3.37, std_dev = 0.10, unit = "ns" }

[results.profiler_enabled_check]
profiler_enabled_check = { mean = 3.36, median = 3.36, std_dev = 0.01, unit = "ns" }

[results.profiler_guard_legacy]
profiler_guard_legacy = { mean = 142.16, median = 141.99, std_dev = 0.84, unit = "ns" }

[results.profiler_histogram]
profiler_histogram = { mean = 3.37, median = 3.36, std_dev = 0.02, unit = "ns" }

[results.profiler_nested_scopes_3]
profiler_nested_scopes_3 = { mean = 86.57, median = 86.39, std_dev = 0.79, unit = "ns" }

[results.profiler_scope_nop]
profiler_scope_nop = { mean = 28.88, median = 28.88, std_dev = 0.08, unit = "ns" }

[results.profiler_span_data_new]
profiler_span_data_new = { mean = 42.09, median = 42.09, std_dev = 0.10, unit = "ns" }

[results.profiler_span_id_new]
profiler_span_id_new = { mean = 6.73, median = 6.72, std_dev = 0.02, unit = "ns" }

[results.search_backward]
collect_all_1000 = { mean = 42.16, median = 42.13, std_dev = 0.15, unit = "µs" }
collect_all_10000 = { mean = 423.19, median = 422.87, std_dev = 1.44, unit = "µs" }
collect_all_100000 = { mean = 4.19, median = 4.19, std_dev = 0.03, unit = "ms" }
early_stop_1000 = { mean = 16.06, median = 16.04, std_dev = 0.09, unit = "µs" }
early_stop_10000 = { mean = 158.95, median = 158.85, std_dev = 0.49, unit = "µs" }
early_stop_100000 = { mean = 1.59, median = 1.59, std_dev = 0.01, unit = "ms" }

[results.search_content_materialization]
lines_1000 = { mean = 10.51, median = 10.55, std_dev = 0.19, unit = "µs" }
lines_10000 = { mean = 123.88, median = 122.49, std_dev = 4.48, unit = "µs" }
lines_100000 = { mean = 1.56, median = 1.54, std_dev = 0.16, unit = "ms" }

[results.search_full]
current_1000 = { mean = 14.85, median = 14.85, std_dev = 0.04, unit = "µs" }
current_10000 = { mean = 129.45, median = 129.22, std_dev = 1.05, unit = "µs" }
current_100000 = { mean = 1.48, median = 1.48, std_dev = 0.01, unit = "ms" }
optimal_1000 = { mean = 51.99, median = 51.68, std_dev = 0.90, unit = "ns" }
optimal_10000 = { mean = 51.29, median = 51.15, std_dev = 0.58, unit = "ns" }
optimal_100000 = { mean = 50.98, median = 50.94, std_dev = 0.20, unit = "ns" }

[results.search_regex_compile]
cached_complex_match = { mean = 214.84, median = 214.72, std_dev = 0.62, unit = "ns" }
char_class = { mean = 17.25, median = 17.24, std_dev = 0.11, unit = "µs" }
complex = { mean = 483.48, median = 472.81, std_dev = 44.11, unit = "µs" }
simple_word = { mean = 4.48, median = 4.48, std_dev = 0.04, unit = "µs" }

[results.syntax_edit_cycle_current]
lines_100 = { mean = 1.66, median = 1.65, std_dev = 0.05, unit = "ms" }
lines_1000 = { mean = 15.48, median = 15.45, std_dev = 0.17, unit = "ms" }
lines_500 = { mean = 7.74, median = 7.71, std_dev = 0.12, unit = "ms" }
lines_5000 = { mean = 78.45, median = 78.18, std_dev = 1.26, unit = "ms" }

[results.syntax_edit_cycle_optimized]
lines_100 = { mean = 284.82, median = 282.15, std_dev = 9.65, unit = "µs" }
lines_1000 = { mean = 1.82, median = 1.81, std_dev = 0.03, unit = "ms" }
lines_500 = { mean = 1.26, median = 1.26, std_dev = 0.01, unit = "ms" }
lines_5000 = { mean = 6.39, median = 6.20, std_dev = 0.61, unit = "ms" }

[results.syntax_full_parse]
lines_100 = { mean = 729.95, median = 729.23, std_dev = 3.98, unit = "µs" }
lines_1000 = { mean = 7.34, median = 7.34, std_dev = 0.02, unit = "ms" }
lines_500 = { mean = 3.65, median = 3.65, std_dev = 0.01, unit = "ms" }
lines_5000 = { mean = 37.66, median = 37.43, std_dev = 0.63, unit = "ms" }

[results.syntax_highlights_edit_region]
lines_100 = { mean = 68.20, median = 68.13, std_dev = 0.31, unit = "µs" }
lines_1000 = { mean = 69.13, median = 68.99, std_dev = 0.92, unit = "µs" }
lines_500 = { mean = 75.54, median = 75.01, std_dev = 1.42, unit = "µs" }
lines_5000 = { mean = 75.07, median = 74.94, std_dev = 0.73, unit = "µs" }

[results.syntax_highlights_full_file]
lines_100 = { mean = 739.45, median = 739.30, std_dev = 2.11, unit = "µs" }
lines_1000 = { mean = 7.39, median = 7.39, std_dev = 0.03, unit = "ms" }
lines_500 = { mean = 3.67, median = 3.66, std_dev = 0.01, unit = "ms" }
lines_5000 = { mean = 36.60, median = 36.60, std_dev = 0.07, unit = "ms" }

[results.syntax_highlights_viewport]
lines_100 = { mean = 370.84, median = 370.65, std_dev = 1.17, unit = "µs" }
lines_1000 = { mean = 397.76, median = 396.97, std_dev = 2.34, unit = "µs" }
lines_500 = { mean = 409.83, median = 409.40, std_dev = 1.86, unit = "µs" }
lines_5000 = { mean = 394.62, median = 394.38, std_dev = 1.47, unit = "µs" }

[results.syntax_incremental_update]
lines_100 = { mean = 152.07, median = 149.70, std_dev = 7.22, unit = "µs" }
lines_1000 = { mean = 1.78, median = 1.74, std_dev = 0.10, unit = "ms" }
lines_500 = { mean = 1.15, median = 1.14, std_dev = 0.02, unit = "ms" }
lines_5000 = { mean = 6.10, median = 6.06, std_dev = 0.20, unit = "ms" }

[results.syntax_token_span_conversion]
tokens_100 = { mean = 6.19, median = 6.18, std_dev = 0.01, unit = "µs" }
tokens_10000 = { mean = 711.69, median = 711.56, std_dev = 0.67, unit = "µs" }
tokens_2000 = { mean = 141.95, median = 141.92, std_dev = 0.10, unit = "µs" }
tokens_500 = { mean = 35.36, median = 35.35, std_dev = 0.04, unit = "µs" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| bridge_detection_fixed/bridges/10 | 7.40 ns | 7.39 ns | 0.03 ns | [7.39, 7.41] ns |
| bridge_detection_fixed/bridges/16 | 9.32 ns | 9.33 ns | 0.02 ns | [9.32, 9.32] ns |
| bridge_detection_fixed/bridges/22 | 14.20 ns | 14.19 ns | 0.04 ns | [14.20, 14.21] ns |
| bridge_detection_fixed/bridges/30 | 17.93 ns | 17.92 ns | 0.08 ns | [17.92, 17.95] ns |
| bridge_detection_loop/bridges/10 | 10.09 ns | 10.08 ns | 0.04 ns | [10.08, 10.10] ns |
| bridge_detection_loop/bridges/16 | 12.04 ns | 12.04 ns | 0.03 ns | [12.04, 12.05] ns |
| bridge_detection_loop/bridges/22 | 17.94 ns | 17.93 ns | 0.02 ns | [17.93, 17.94] ns |
| bridge_detection_loop/bridges/30 | 22.83 ns | 22.82 ns | 0.05 ns | [22.82, 22.84] ns |
| bridge_json_parse/bufferline_buffers/10 | 13.99 µs | 13.97 µs | 0.06 µs | [13.98, 14.00] µs |
| bridge_json_parse/bufferline_buffers/3 | 3.85 µs | 3.85 µs | 0.01 µs | [3.85, 3.85] µs |
| bridge_json_parse/bufferline_buffers/30 | 49.18 µs | 49.14 µs | 0.19 µs | [49.14, 49.22] µs |
| bridge_json_serialize/bufferline_buffers/10 | 7.67 µs | 7.65 µs | 0.08 µs | [7.66, 7.69] µs |
| bridge_json_serialize/bufferline_buffers/3 | 2.66 µs | 2.66 µs | 0.01 µs | [2.66, 2.66] µs |
| bridge_json_serialize/bufferline_buffers/30 | 22.15 µs | 22.02 µs | 0.77 µs | [22.02, 22.32] µs |
| bridge_json_serialize/diagnostics_items/10 | 6.37 µs | 6.24 µs | 0.81 µs | [6.21, 6.53] µs |
| bridge_json_serialize/diagnostics_items/200 | 92.03 µs | 90.68 µs | 4.72 µs | [91.21, 93.00] µs |
| bridge_json_serialize/diagnostics_items/50 | 25.13 µs | 23.68 µs | 3.52 µs | [24.49, 25.87] µs |
| bridge_json_serialize/illuminate_ranges/20 | 9.01 µs | 9.01 µs | 0.01 µs | [9.01, 9.02] µs |
| bridge_json_serialize/illuminate_ranges/5 | 2.82 µs | 2.81 µs | 0.03 µs | [2.82, 2.83] µs |
| bridge_json_serialize/illuminate_ranges/50 | 21.13 µs | 21.12 µs | 0.04 µs | [21.13, 21.14] µs |
| bridge_unnecessary_per_keystroke/active_1 | 4.16 µs | 4.16 µs | 0.01 µs | [4.16, 4.16] µs |
| bridge_unnecessary_per_keystroke/active_3 | 5.09 µs | 5.08 µs | 0.03 µs | [5.08, 5.10] µs |
| bridge_unnecessary_per_keystroke/active_5 | 6.05 µs | 6.05 µs | 0.01 µs | [6.05, 6.05] µs |
| buffer_byte_to_position/lines/100 | 74.79 ns | 74.71 ns | 0.22 ns | [74.74, 74.83] ns |
| buffer_byte_to_position/lines/1000 | 599.05 ns | 597.71 ns | 5.54 ns | [598.11, 600.25] ns |
| buffer_byte_to_position/lines/10000 | 6.56 µs | 6.56 µs | 0.01 µs | [6.56, 6.56] µs |
| buffer_byte_to_position/lines/100000 | 80.61 µs | 80.70 µs | 0.67 µs | [80.48, 80.73] µs |
| buffer_clone/lines/100 | 7.06 µs | 7.05 µs | 0.02 µs | [7.06, 7.06] µs |
| buffer_clone/lines/1000 | 76.81 µs | 76.71 µs | 0.27 µs | [76.76, 76.86] µs |
| buffer_clone/lines/10000 | 786.80 µs | 785.37 µs | 8.61 µs | [785.52, 788.76] µs |
| buffer_clone/lines/100000 | 7.92 ms | 7.90 ms | 0.09 ms | [7.90, 7.93] ms |
| buffer_content/lines/100 | 463.67 ns | 463.29 ns | 1.85 ns | [463.32, 464.04] ns |
| buffer_content/lines/1000 | 4.80 µs | 4.80 µs | 0.02 µs | [4.80, 4.81] µs |
| buffer_content/lines/10000 | 57.93 µs | 57.93 µs | 0.13 µs | [57.90, 57.95] µs |
| buffer_content/lines/100000 | 691.78 µs | 690.67 µs | 4.74 µs | [690.98, 692.80] µs |
| buffer_delete_char/lines/100 | 462.76 ns | 457.22 ns | 19.77 ns | [459.24, 466.95] ns |
| buffer_delete_char/lines/1000 | 520.37 ns | 505.12 ns | 57.30 ns | [510.23, 532.24] ns |
| buffer_delete_char/lines/10000 | 783.90 ns | 761.75 ns | 83.02 ns | [769.13, 801.49] ns |
| buffer_delete_char/lines/100000 | 1.30 µs | 1.27 µs | 0.17 µs | [1.27, 1.34] µs |
| buffer_delete_range/lines/100 | 751.24 ns | 698.95 ns | 546.23 ns | [693.75, 862.67] ns |
| buffer_delete_range/lines/1000 | 1.08 µs | 1.10 µs | 0.07 µs | [1.07, 1.10] µs |
| buffer_delete_range/lines/10000 | 5.51 µs | 5.47 µs | 0.11 µs | [5.49, 5.53] µs |
| buffer_delete_range/lines/100000 | 52.94 µs | 52.56 µs | 0.95 µs | [52.77, 53.14] µs |
| buffer_from_string/lines/100 | 10.40 µs | 10.10 µs | 0.75 µs | [10.27, 10.56] µs |
| buffer_from_string/lines/1000 | 105.10 µs | 104.97 µs | 0.56 µs | [105.01, 105.22] µs |
| buffer_from_string/lines/10000 | 1.01 ms | 1.01 ms | 0.00 ms | [1.01, 1.01] ms |
| buffer_from_string/lines/100000 | 10.07 ms | 10.02 ms | 0.18 ms | [10.04, 10.11] ms |
| buffer_insert_char/lines/100 | 307.07 ns | 307.01 ns | 15.20 ns | [304.42, 310.51] ns |
| buffer_insert_char/lines/1000 | 370.03 ns | 358.13 ns | 53.65 ns | [360.94, 382.30] ns |
| buffer_insert_char/lines/10000 | 618.48 ns | 580.55 ns | 99.57 ns | [600.47, 639.09] ns |
| buffer_insert_char/lines/100000 | 1.44 µs | 1.41 µs | 0.28 µs | [1.39, 1.49] µs |
| buffer_insert_line/lines/100 | 343.10 ns | 349.46 ns | 28.85 ns | [337.47, 348.63] ns |
| buffer_insert_line/lines/1000 | 697.45 ns | 724.34 ns | 87.06 ns | [680.32, 714.28] ns |
| buffer_insert_line/lines/10000 | 4.90 µs | 4.87 µs | 0.16 µs | [4.88, 4.94] µs |
| buffer_insert_line/lines/100000 | 57.94 µs | 55.13 µs | 10.85 µs | [55.96, 60.18] µs |
| buffer_insert_word/lines/100 | 323.14 ns | 323.96 ns | 15.27 ns | [320.51, 326.47] ns |
| buffer_insert_word/lines/1000 | 375.08 ns | 358.04 ns | 62.58 ns | [364.30, 388.33] ns |
| buffer_insert_word/lines/10000 | 562.53 ns | 555.57 ns | 38.81 ns | [555.47, 570.73] ns |
| buffer_insert_word/lines/100000 | 1.39 µs | 1.24 µs | 0.45 µs | [1.31, 1.48] µs |
| buffer_line_access/lines/100 | 0.05 ns | 0.05 ns | 0.00 ns | [0.05, 0.05] ns |
| buffer_line_access/lines/1000 | 0.05 ns | 0.05 ns | 0.00 ns | [0.05, 0.05] ns |
| buffer_line_access/lines/10000 | 0.05 ns | 0.05 ns | 0.00 ns | [0.05, 0.05] ns |
| buffer_line_access/lines/100000 | 0.05 ns | 0.05 ns | 0.00 ns | [0.05, 0.05] ns |
| buffer_line_hashes/lines/100 | 3.60 µs | 3.60 µs | 0.01 µs | [3.60, 3.61] µs |
| buffer_line_hashes/lines/1000 | 32.81 µs | 32.80 µs | 0.34 µs | [32.75, 32.88] µs |
| buffer_line_hashes/lines/10000 | 352.05 µs | 329.23 µs | 43.59 µs | [343.85, 360.76] µs |
| buffer_line_hashes/lines/100000 | 3.22 ms | 3.19 ms | 0.12 ms | [3.20, 3.25] ms |
| buffer_position_to_byte/lines/100 | 111.88 ns | 110.65 ns | 4.23 ns | [111.16, 112.78] ns |
| buffer_position_to_byte/lines/1000 | 797.59 ns | 783.38 ns | 116.43 ns | [783.37, 823.15] ns |
| buffer_position_to_byte/lines/10000 | 7.61 µs | 7.55 µs | 0.09 µs | [7.59, 7.63] µs |
| buffer_position_to_byte/lines/100000 | 97.15 µs | 98.11 µs | 1.83 µs | [96.79, 97.50] µs |
| buffer_vec_char_alloc/1000_chars | 1.21 µs | 1.20 µs | 0.03 µs | [1.20, 1.21] µs |
| buffer_vec_char_alloc/11_chars | 182.67 ns | 182.60 ns | 0.64 ns | [182.57, 182.81] ns |
| buffer_vec_char_alloc/99_chars | 288.63 ns | 288.06 ns | 4.47 ns | [288.11, 289.58] ns |
| event_dispatch/handlers/1 | 65.00 ns | 64.57 ns | 0.81 ns | [64.85, 65.17] ns |
| event_dispatch/handlers/10 | 108.91 ns | 108.59 ns | 2.18 ns | [108.59, 109.39] ns |
| event_dispatch/handlers/100 | 556.95 ns | 555.98 ns | 3.51 ns | [556.32, 557.69] ns |
| event_dispatch/handlers/5 | 87.73 ns | 88.12 ns | 2.59 ns | [87.26, 88.26] ns |
| event_dispatch/handlers/50 | 312.97 ns | 312.63 ns | 1.58 ns | [312.66, 313.29] ns |
| event_dispatch_0_handlers | 38.34 ns | 38.20 ns | 0.59 ns | [38.24, 38.47] ns |
| event_dispatch_multi_type | 195.63 ns | 195.13 ns | 2.27 ns | [195.25, 196.13] ns |
| event_dispatch_propagate/handlers/1 | 63.82 ns | 63.52 ns | 0.57 ns | [63.71, 63.94] ns |
| event_dispatch_propagate/handlers/10 | 108.89 ns | 108.76 ns | 0.57 ns | [108.79, 109.01] ns |
| event_dispatch_propagate/handlers/5 | 84.88 ns | 84.03 ns | 3.72 ns | [84.29, 85.71] ns |
| event_subscribe | 805.81 ns | 797.78 ns | 30.63 ns | [800.59, 812.48] ns |
| metrics_counter_get_and_increment | 70.75 ns | 67.61 ns | 5.67 ns | [69.69, 71.90] ns |
| metrics_histogram_get_and_record | 88.61 ns | 86.14 ns | 6.24 ns | [87.42, 89.87] ns |
| profiler_counter | 3.41 ns | 3.37 ns | 0.10 ns | [3.39, 3.43] ns |
| profiler_enabled_check | 3.36 ns | 3.36 ns | 0.01 ns | [3.36, 3.36] ns |
| profiler_guard_legacy | 142.16 ns | 141.99 ns | 0.84 ns | [142.02, 142.34] ns |
| profiler_histogram | 3.37 ns | 3.36 ns | 0.02 ns | [3.36, 3.37] ns |
| profiler_nested_scopes_3 | 86.57 ns | 86.39 ns | 0.79 ns | [86.43, 86.73] ns |
| profiler_scope_nop | 28.88 ns | 28.88 ns | 0.08 ns | [28.86, 28.89] ns |
| profiler_span_data_new | 42.09 ns | 42.09 ns | 0.10 ns | [42.07, 42.11] ns |
| profiler_span_id_new | 6.73 ns | 6.72 ns | 0.02 ns | [6.73, 6.73] ns |
| search_backward/collect_all/1000 | 42.16 µs | 42.13 µs | 0.15 µs | [42.13, 42.19] µs |
| search_backward/collect_all/10000 | 423.19 µs | 422.87 µs | 1.44 µs | [422.94, 423.49] µs |
| search_backward/collect_all/100000 | 4.19 ms | 4.19 ms | 0.03 ms | [4.19, 4.20] ms |
| search_backward/early_stop/1000 | 16.06 µs | 16.04 µs | 0.09 µs | [16.04, 16.08] µs |
| search_backward/early_stop/10000 | 158.95 µs | 158.85 µs | 0.49 µs | [158.86, 159.05] µs |
| search_backward/early_stop/100000 | 1.59 ms | 1.59 ms | 0.01 ms | [1.59, 1.59] ms |
| search_content_materialization/lines/1000 | 10.51 µs | 10.55 µs | 0.19 µs | [10.47, 10.55] µs |
| search_content_materialization/lines/10000 | 123.88 µs | 122.49 µs | 4.48 µs | [123.05, 124.78] µs |
| search_content_materialization/lines/100000 | 1.56 ms | 1.54 ms | 0.16 ms | [1.53, 1.59] ms |
| search_full/current/1000 | 14.85 µs | 14.85 µs | 0.04 µs | [14.84, 14.86] µs |
| search_full/current/10000 | 129.45 µs | 129.22 µs | 1.05 µs | [129.27, 129.68] µs |
| search_full/current/100000 | 1.48 ms | 1.48 ms | 0.01 ms | [1.48, 1.48] ms |
| search_full/optimal/1000 | 51.99 ns | 51.68 ns | 0.90 ns | [51.83, 52.17] ns |
| search_full/optimal/10000 | 51.29 ns | 51.15 ns | 0.58 ns | [51.20, 51.42] ns |
| search_full/optimal/100000 | 50.98 ns | 50.94 ns | 0.20 ns | [50.94, 51.02] ns |
| search_regex_compile/cached_complex_match | 214.84 ns | 214.72 ns | 0.62 ns | [214.73, 214.97] ns |
| search_regex_compile/char_class | 17.25 µs | 17.24 µs | 0.11 µs | [17.23, 17.28] µs |
| search_regex_compile/complex | 483.48 µs | 472.81 µs | 44.11 µs | [476.02, 493.13] µs |
| search_regex_compile/simple_word | 4.48 µs | 4.48 µs | 0.04 µs | [4.47, 4.49] µs |
| syntax_edit_cycle_current/lines/100 | 1.66 ms | 1.65 ms | 0.05 ms | [1.66, 1.67] ms |
| syntax_edit_cycle_current/lines/1000 | 15.48 ms | 15.45 ms | 0.17 ms | [15.46, 15.52] ms |
| syntax_edit_cycle_current/lines/500 | 7.74 ms | 7.71 ms | 0.12 ms | [7.72, 7.76] ms |
| syntax_edit_cycle_current/lines/5000 | 78.45 ms | 78.18 ms | 1.26 ms | [78.22, 78.71] ms |
| syntax_edit_cycle_optimized/lines/100 | 284.82 µs | 282.15 µs | 9.65 µs | [283.12, 286.88] µs |
| syntax_edit_cycle_optimized/lines/1000 | 1.82 ms | 1.81 ms | 0.03 ms | [1.82, 1.83] ms |
| syntax_edit_cycle_optimized/lines/500 | 1.26 ms | 1.26 ms | 0.01 ms | [1.26, 1.27] ms |
| syntax_edit_cycle_optimized/lines/5000 | 6.39 ms | 6.20 ms | 0.61 ms | [6.28, 6.51] ms |
| syntax_full_parse/lines/100 | 729.95 µs | 729.23 µs | 3.98 µs | [729.27, 730.81] µs |
| syntax_full_parse/lines/1000 | 7.34 ms | 7.34 ms | 0.02 ms | [7.33, 7.34] ms |
| syntax_full_parse/lines/500 | 3.65 ms | 3.65 ms | 0.01 ms | [3.65, 3.65] ms |
| syntax_full_parse/lines/5000 | 37.66 ms | 37.43 ms | 0.63 ms | [37.54, 37.79] ms |
| syntax_highlights_edit_region/lines/100 | 68.20 µs | 68.13 µs | 0.31 µs | [68.14, 68.26] µs |
| syntax_highlights_edit_region/lines/1000 | 69.13 µs | 68.99 µs | 0.92 µs | [69.00, 69.33] µs |
| syntax_highlights_edit_region/lines/500 | 75.54 µs | 75.01 µs | 1.42 µs | [75.28, 75.83] µs |
| syntax_highlights_edit_region/lines/5000 | 75.07 µs | 74.94 µs | 0.73 µs | [74.95, 75.23] µs |
| syntax_highlights_full_file/lines/100 | 739.45 µs | 739.30 µs | 2.11 µs | [739.06, 739.90] µs |
| syntax_highlights_full_file/lines/1000 | 7.39 ms | 7.39 ms | 0.03 ms | [7.38, 7.40] ms |
| syntax_highlights_full_file/lines/500 | 3.67 ms | 3.66 ms | 0.01 ms | [3.67, 3.67] ms |
| syntax_highlights_full_file/lines/5000 | 36.60 ms | 36.60 ms | 0.07 ms | [36.59, 36.62] ms |
| syntax_highlights_viewport/lines/100 | 370.84 µs | 370.65 µs | 1.17 µs | [370.62, 371.08] µs |
| syntax_highlights_viewport/lines/1000 | 397.76 µs | 396.97 µs | 2.34 µs | [397.32, 398.24] µs |
| syntax_highlights_viewport/lines/500 | 409.83 µs | 409.40 µs | 1.86 µs | [409.52, 410.24] µs |
| syntax_highlights_viewport/lines/5000 | 394.62 µs | 394.38 µs | 1.47 µs | [394.34, 394.92] µs |
| syntax_incremental_update/lines/100 | 152.07 µs | 149.70 µs | 7.22 µs | [150.77, 153.62] µs |
| syntax_incremental_update/lines/1000 | 1.78 ms | 1.74 ms | 0.10 ms | [1.76, 1.80] ms |
| syntax_incremental_update/lines/500 | 1.15 ms | 1.14 ms | 0.02 ms | [1.14, 1.15] ms |
| syntax_incremental_update/lines/5000 | 6.10 ms | 6.06 ms | 0.20 ms | [6.06, 6.14] ms |
| syntax_token_span_conversion/tokens/100 | 6.19 µs | 6.18 µs | 0.01 µs | [6.18, 6.19] µs |
| syntax_token_span_conversion/tokens/10000 | 711.69 µs | 711.56 µs | 0.67 µs | [711.56, 711.82] µs |
| syntax_token_span_conversion/tokens/2000 | 141.95 µs | 141.92 µs | 0.10 µs | [141.93, 141.97] µs |
| syntax_token_span_conversion/tokens/500 | 35.36 µs | 35.35 µs | 0.04 µs | [35.35, 35.37] µs |
