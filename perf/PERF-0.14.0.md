# Performance Benchmarks v0.14.0

> Last updated: 2026-03-15T11:07:29Z | Commit: dea6525b | Rust: 1.96.0-nightly

## Metadata

```toml
[metadata]
version = "0.14.0"
commit = "dea6525b"
date = "2026-03-15T11:07:29Z"
rust_version = "1.96.0-nightly"
os = "Linux 6.19.6-arch1-1"
```

## Summary

| Category | Benchmarks | Avg Time |
|----------|------------|----------|
| buffer_delete_char | 4 | 525.90 ns |
| buffer_delete_range | 3 | 238.24 ns |
| buffer_from_string | 4 | 439.16 ns |
| buffer_insert_char | 4 | 383.33 ns |
| buffer_insert_word | 3 | 340.64 ns |
| buffer_line_access | 3 | 0.05 ns |
| buffer_position_to_byte | 3 | 164.91 ns |
| event_dispatch | 5 | 246.48 ns |
| event_dispatch_0_handlers | 1 | 38.04 ns |
| event_dispatch_multi_type | 1 | 192.61 ns |
| event_dispatch_propagate | 3 | 87.78 ns |
| event_subscribe | 1 | 807.77 ns |
| metrics_counter_get_and_increment | 1 | 66.81 ns |
| metrics_histogram_get_and_record | 1 | 86.73 ns |
| profiler_counter | 1 | 3.74 ns |
| profiler_enabled_check | 1 | 3.74 ns |
| profiler_guard_legacy | 1 | 141.18 ns |
| profiler_histogram | 1 | 3.74 ns |
| profiler_nested_scopes_3 | 1 | 86.08 ns |
| profiler_scope_nop | 1 | 29.11 ns |
| profiler_span_data_new | 1 | 42.21 ns |
| profiler_span_id_new | 1 | 6.73 ns |

## Results

```toml
[results.buffer_delete_char]
lines_10 = { mean = 440.53, median = 441.23, std_dev = 6.44, unit = "ns" }
lines_100 = { mean = 462.64, median = 457.56, std_dev = 20.56, unit = "ns" }
lines_1000 = { mean = 510.19, median = 490.01, std_dev = 91.01, unit = "ns" }
lines_10000 = { mean = 690.24, median = 681.19, std_dev = 49.16, unit = "ns" }

[results.buffer_delete_range]
lines_100 = { mean = 708.01, median = 708.27, std_dev = 45.12, unit = "ns" }
lines_1000 = { mean = 1.08, median = 1.10, std_dev = 0.08, unit = "µs" }
lines_10000 = { mean = 5.64, median = 5.62, std_dev = 0.08, unit = "µs" }

[results.buffer_from_string]
lines_10 = { mean = 657.55, median = 657.26, std_dev = 1.82, unit = "ns" }
lines_100 = { mean = 8.33, median = 8.33, std_dev = 0.02, unit = "µs" }
lines_1000 = { mean = 98.68, median = 98.65, std_dev = 0.20, unit = "µs" }
lines_10000 = { mean = 992.07, median = 991.21, std_dev = 4.10, unit = "µs" }

[results.buffer_insert_char]
lines_10 = { mean = 284.13, median = 285.64, std_dev = 3.28, unit = "ns" }
lines_100 = { mean = 332.81, median = 326.53, std_dev = 58.01, unit = "ns" }
lines_1000 = { mean = 372.09, median = 352.58, std_dev = 84.34, unit = "ns" }
lines_10000 = { mean = 544.29, median = 530.64, std_dev = 57.91, unit = "ns" }

[results.buffer_insert_word]
lines_10 = { mean = 313.27, median = 314.92, std_dev = 5.17, unit = "ns" }
lines_100 = { mean = 329.02, median = 330.77, std_dev = 15.93, unit = "ns" }
lines_1000 = { mean = 379.64, median = 359.42, std_dev = 75.05, unit = "ns" }

[results.buffer_line_access]
lines_100 = { mean = 0.05, median = 0.05, std_dev = 0.01, unit = "ns" }
lines_1000 = { mean = 0.05, median = 0.05, std_dev = 0.00, unit = "ns" }
lines_10000 = { mean = 0.05, median = 0.05, std_dev = 0.00, unit = "ns" }

[results.buffer_position_to_byte]
lines_100 = { mean = 72.66, median = 72.64, std_dev = 0.21, unit = "ns" }
lines_1000 = { mean = 418.29, median = 418.18, std_dev = 0.89, unit = "ns" }
lines_10000 = { mean = 3.79, median = 3.79, std_dev = 0.01, unit = "µs" }

[results.event_dispatch]
handlers_1 = { mean = 63.57, median = 63.49, std_dev = 0.40, unit = "ns" }
handlers_10 = { mean = 113.51, median = 113.46, std_dev = 0.29, unit = "ns" }
handlers_100 = { mean = 620.94, median = 620.84, std_dev = 2.43, unit = "ns" }
handlers_5 = { mean = 85.57, median = 85.52, std_dev = 0.33, unit = "ns" }
handlers_50 = { mean = 348.81, median = 348.76, std_dev = 1.54, unit = "ns" }

[results.event_dispatch_0_handlers]
event_dispatch_0_handlers = { mean = 38.04, median = 38.03, std_dev = 0.08, unit = "ns" }

[results.event_dispatch_multi_type]
event_dispatch_multi_type = { mean = 192.61, median = 192.58, std_dev = 0.30, unit = "ns" }

[results.event_dispatch_propagate]
handlers_1 = { mean = 63.61, median = 63.61, std_dev = 0.07, unit = "ns" }
handlers_10 = { mean = 113.59, median = 113.56, std_dev = 0.44, unit = "ns" }
handlers_5 = { mean = 86.13, median = 85.72, std_dev = 0.82, unit = "ns" }

[results.event_subscribe]
event_subscribe = { mean = 807.77, median = 807.71, std_dev = 0.51, unit = "ns" }

[results.metrics_counter_get_and_increment]
metrics_counter_get_and_increment = { mean = 66.81, median = 66.95, std_dev = 0.23, unit = "ns" }

[results.metrics_histogram_get_and_record]
metrics_histogram_get_and_record = { mean = 86.73, median = 86.81, std_dev = 0.37, unit = "ns" }

[results.profiler_counter]
profiler_counter = { mean = 3.74, median = 3.73, std_dev = 0.01, unit = "ns" }

[results.profiler_enabled_check]
profiler_enabled_check = { mean = 3.74, median = 3.73, std_dev = 0.02, unit = "ns" }

[results.profiler_guard_legacy]
profiler_guard_legacy = { mean = 141.18, median = 141.01, std_dev = 0.91, unit = "ns" }

[results.profiler_histogram]
profiler_histogram = { mean = 3.74, median = 3.74, std_dev = 0.00, unit = "ns" }

[results.profiler_nested_scopes_3]
profiler_nested_scopes_3 = { mean = 86.08, median = 85.98, std_dev = 0.57, unit = "ns" }

[results.profiler_scope_nop]
profiler_scope_nop = { mean = 29.11, median = 29.12, std_dev = 0.22, unit = "ns" }

[results.profiler_span_data_new]
profiler_span_data_new = { mean = 42.21, median = 42.19, std_dev = 0.14, unit = "ns" }

[results.profiler_span_id_new]
profiler_span_id_new = { mean = 6.73, median = 6.73, std_dev = 0.02, unit = "ns" }

```

## Detailed Results

| Benchmark | Mean | Median | Std Dev | CI (95%) |
|-----------|------|--------|---------|----------|
| buffer_delete_char/lines/10 | 440.53 ns | 441.23 ns | 6.44 ns | [439.28, 441.79] ns |
| buffer_delete_char/lines/100 | 462.64 ns | 457.56 ns | 20.56 ns | [459.12, 467.15] ns |
| buffer_delete_char/lines/1000 | 510.19 ns | 490.01 ns | 91.01 ns | [495.38, 530.21] ns |
| buffer_delete_char/lines/10000 | 690.24 ns | 681.19 ns | 49.16 ns | [681.82, 700.88] ns |
| buffer_delete_range/lines/100 | 708.01 ns | 708.27 ns | 45.12 ns | [700.96, 718.15] ns |
| buffer_delete_range/lines/1000 | 1.08 µs | 1.10 µs | 0.08 µs | [1.07, 1.10] µs |
| buffer_delete_range/lines/10000 | 5.64 µs | 5.62 µs | 0.08 µs | [5.62, 5.66] µs |
| buffer_from_string/lines/10 | 657.55 ns | 657.26 ns | 1.82 ns | [657.26, 657.95] ns |
| buffer_from_string/lines/100 | 8.33 µs | 8.33 µs | 0.02 µs | [8.33, 8.34] µs |
| buffer_from_string/lines/1000 | 98.68 µs | 98.65 µs | 0.20 µs | [98.65, 98.72] µs |
| buffer_from_string/lines/10000 | 992.07 µs | 991.21 µs | 4.10 µs | [991.44, 993.00] µs |
| buffer_insert_char/lines/10 | 284.13 ns | 285.64 ns | 3.28 ns | [283.52, 284.77] ns |
| buffer_insert_char/lines/100 | 332.81 ns | 326.53 ns | 58.01 ns | [323.56, 345.51] ns |
| buffer_insert_char/lines/1000 | 372.09 ns | 352.58 ns | 84.34 ns | [358.71, 390.38] ns |
| buffer_insert_char/lines/10000 | 544.29 ns | 530.64 ns | 57.91 ns | [534.13, 556.34] ns |
| buffer_insert_word/lines/10 | 313.27 ns | 314.92 ns | 5.17 ns | [312.37, 314.38] ns |
| buffer_insert_word/lines/100 | 329.02 ns | 330.77 ns | 15.93 ns | [326.24, 332.49] ns |
| buffer_insert_word/lines/1000 | 379.64 ns | 359.42 ns | 75.05 ns | [367.20, 396.30] ns |
| buffer_line_access/lines/100 | 0.05 ns | 0.05 ns | 0.01 ns | [0.05, 0.05] ns |
| buffer_line_access/lines/1000 | 0.05 ns | 0.05 ns | 0.00 ns | [0.05, 0.05] ns |
| buffer_line_access/lines/10000 | 0.05 ns | 0.05 ns | 0.00 ns | [0.05, 0.05] ns |
| buffer_position_to_byte/lines/100 | 72.66 ns | 72.64 ns | 0.21 ns | [72.62, 72.70] ns |
| buffer_position_to_byte/lines/1000 | 418.29 ns | 418.18 ns | 0.89 ns | [418.15, 418.49] ns |
| buffer_position_to_byte/lines/10000 | 3.79 µs | 3.79 µs | 0.01 µs | [3.79, 3.79] µs |
| event_dispatch/handlers/1 | 63.57 ns | 63.49 ns | 0.40 ns | [63.50, 63.66] ns |
| event_dispatch/handlers/10 | 113.51 ns | 113.46 ns | 0.29 ns | [113.46, 113.57] ns |
| event_dispatch/handlers/100 | 620.94 ns | 620.84 ns | 2.43 ns | [620.47, 621.43] ns |
| event_dispatch/handlers/5 | 85.57 ns | 85.52 ns | 0.33 ns | [85.51, 85.64] ns |
| event_dispatch/handlers/50 | 348.81 ns | 348.76 ns | 1.54 ns | [348.53, 349.12] ns |
| event_dispatch_0_handlers | 38.04 ns | 38.03 ns | 0.08 ns | [38.03, 38.06] ns |
| event_dispatch_multi_type | 192.61 ns | 192.58 ns | 0.30 ns | [192.57, 192.68] ns |
| event_dispatch_propagate/handlers/1 | 63.61 ns | 63.61 ns | 0.07 ns | [63.60, 63.63] ns |
| event_dispatch_propagate/handlers/10 | 113.59 ns | 113.56 ns | 0.44 ns | [113.53, 113.69] ns |
| event_dispatch_propagate/handlers/5 | 86.13 ns | 85.72 ns | 0.82 ns | [85.97, 86.30] ns |
| event_subscribe | 807.77 ns | 807.71 ns | 0.51 ns | [807.67, 807.87] ns |
| metrics_counter_get_and_increment | 66.81 ns | 66.95 ns | 0.23 ns | [66.76, 66.85] ns |
| metrics_histogram_get_and_record | 86.73 ns | 86.81 ns | 0.37 ns | [86.66, 86.81] ns |
| profiler_counter | 3.74 ns | 3.73 ns | 0.01 ns | [3.73, 3.74] ns |
| profiler_enabled_check | 3.74 ns | 3.73 ns | 0.02 ns | [3.73, 3.74] ns |
| profiler_guard_legacy | 141.18 ns | 141.01 ns | 0.91 ns | [141.03, 141.38] ns |
| profiler_histogram | 3.74 ns | 3.74 ns | 0.00 ns | [3.74, 3.74] ns |
| profiler_nested_scopes_3 | 86.08 ns | 85.98 ns | 0.57 ns | [85.99, 86.21] ns |
| profiler_scope_nop | 29.11 ns | 29.12 ns | 0.22 ns | [29.07, 29.15] ns |
| profiler_span_data_new | 42.21 ns | 42.19 ns | 0.14 ns | [42.18, 42.23] ns |
| profiler_span_id_new | 6.73 ns | 6.73 ns | 0.02 ns | [6.73, 6.73] ns |
