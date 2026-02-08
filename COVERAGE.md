# Workspace MC/DC Coverage Report

Generated: 2026-02-08 16:08 | Mode: mcdc | Target: 100%

**Overall**: 94.6% lines (101498/107329) | 360 files

## Crate Summary

| Crate | Files | Lines | Hit | Miss | Line % | Status |
|-------|------:|------:|----:|-----:|-------:|--------|
| reovim-client-cli | 3 | 673 | 232 | 441 | 34.5% | LOW |
| reovim-testing | 7 | 1107 | 507 | 600 | 45.8% | LOW |
| reovim-app | 2 | 506 | 280 | 226 | 55.3% | LOW |
| reovim-driver-clipboard | 3 | 339 | 276 | 63 | 81.4% | 70%+ |
| reovim-driver-undo | 3 | 648 | 565 | 83 | 87.2% | 70%+ |
| reovim-driver-log | 3 | 874 | 765 | 109 | 87.5% | 70%+ |
| reovim-driver-command | 8 | 2005 | 1765 | 240 | 88.0% | 70%+ |
| reovim-client-tui | 26 | 7556 | 6717 | 839 | 88.9% | 70%+ |
| reovim-driver-net | 5 | 875 | 807 | 68 | 92.2% | 90%+ |
| reovim-module-clipboard | 3 | 452 | 421 | 31 | 93.1% | 90%+ |
| reovim-driver-session | 19 | 6240 | 5920 | 320 | 94.9% | 90%+ |
| reovim-module-editor | 14 | 7197 | 6830 | 367 | 94.9% | 90%+ |
| reovim-module-vim | 43 | 22492 | 21433 | 1059 | 95.3% | 90%+ |
| reovim-driver-ffi | 4 | 512 | 489 | 23 | 95.5% | 90%+ |
| reovim-module-vfs-local | 1 | 160 | 153 | 7 | 95.6% | 90%+ |
| reovim-driver-tui | 8 | 2665 | 2551 | 114 | 95.7% | 90%+ |
| reovim-module-textobjects | 5 | 3358 | 3219 | 139 | 95.9% | 90%+ |
| reovim-module-window-ops | 3 | 1074 | 1036 | 38 | 96.5% | 90%+ |
| reovim-kernel | 71 | 17355 | 16771 | 584 | 96.6% | 90%+ |
| reovim-driver-input | 16 | 4393 | 4247 | 146 | 96.7% | 90%+ |
| reovim-client-model | 20 | 2382 | 2310 | 72 | 97.0% | 90%+ |
| reovim-module-scratch-buffer | 1 | 168 | 163 | 5 | 97.0% | 90%+ |
| reovim-driver-syntax-treesitter | 3 | 1523 | 1482 | 41 | 97.3% | 90%+ |
| reovim-arch | 10 | 2783 | 2709 | 74 | 97.3% | 90%+ |
| reovim-module-commands | 7 | 1111 | 1086 | 25 | 97.7% | 90%+ |
| reovim-module-defaults | 1 | 177 | 174 | 3 | 98.3% | 90%+ |
| reovim-module-motions | 5 | 3272 | 3220 | 52 | 98.4% | 90%+ |
| reovim-module-buffer-simple | 1 | 195 | 192 | 3 | 98.5% | 90%+ |
| reovim-module-undo | 2 | 1238 | 1220 | 18 | 98.5% | 90%+ |
| reovim-module-search | 2 | 511 | 506 | 5 | 99.0% | 90%+ |
| reovim-driver-trace | 1 | 273 | 271 | 2 | 99.3% | 90%+ |
| reovim-module-treesitter-rust | 1 | 725 | 720 | 5 | 99.3% | 90%+ |
| reovim-driver-lsp | 5 | 573 | 570 | 3 | 99.5% | 90%+ |
| reovim-driver-search | 3 | 196 | 195 | 1 | 99.5% | 90%+ |
| reovim-protocol | 18 | 4456 | 4438 | 18 | 99.6% | 90%+ |
| reovim-driver-syntax | 9 | 1428 | 1424 | 4 | 99.7% | 90%+ |
| reovim-driver-vfs | 12 | 3000 | 2997 | 3 | 99.9% | 90%+ |
| reovim-module-treesitter-markdown | 1 | 422 | 422 | 0 | 100.0% | PASS |
| reovim-driver-buffer | 2 | 109 | 109 | 0 | 100.0% | PASS |
| reovim-module-options | 1 | 536 | 536 | 0 | 100.0% | PASS |
| reovim-module-mode-manager | 1 | 230 | 230 | 0 | 100.0% | PASS |
| reovim-module-keymap | 2 | 593 | 593 | 0 | 100.0% | PASS |
| reovim-driver-command-types | 4 | 748 | 748 | 0 | 100.0% | PASS |
| reovim-module-buffer-ops | 1 | 199 | 199 | 0 | 100.0% | PASS |
| **Total** | **360** | **107329** | **101498** | **5831** | **94.6%** | |

## Gaps (files below 100%)

### reovim-client-cli (34.5%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `clients/cli/src/commands.rs` | 336 | 5 | 331 | 1.5% |
| `clients/cli/src/lib.rs` | 95 | 58 | 37 | 61.1% |
| `clients/cli/src/client.rs` | 242 | 169 | 73 | 69.8% |

### reovim-testing (45.8%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/testing/src/lib.rs` | 35 | 0 | 35 | 0.0% |
| `shared/testing/src/step_test.rs` | 310 | 0 | 310 | 0.0% |
| `shared/testing/src/multi_client.rs` | 115 | 0 | 115 | 0.0% |
| `shared/testing/src/integration.rs` | 184 | 125 | 59 | 67.9% |
| `shared/testing/src/presence.rs` | 201 | 155 | 46 | 77.1% |
| `shared/testing/src/harness.rs` | 169 | 143 | 26 | 84.6% |
| `shared/testing/src/frame.rs` | 93 | 84 | 9 | 90.3% |

### reovim-app (55.3%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `apps/bin/src/main.rs` | 265 | 61 | 204 | 23.0% |
| `apps/bin/src/bootstrap.rs` | 241 | 219 | 22 | 90.9% |

### reovim-driver-clipboard (81.4%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/clipboard/src/provider.rs` | 282 | 219 | 63 | 77.7% |

### reovim-driver-undo (87.2%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/undo/src/provider.rs` | 598 | 515 | 83 | 86.1% |

### reovim-driver-log (87.5%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/log/src/subscriber.rs` | 521 | 412 | 109 | 79.1% |

### reovim-driver-command (88.0%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/command/src/traits.rs` | 264 | 158 | 106 | 59.8% |
| `server/lib/drivers/command/src/provider.rs` | 61 | 52 | 9 | 85.2% |
| `server/lib/drivers/command/src/ex_handler.rs` | 568 | 491 | 77 | 86.4% |
| `server/lib/drivers/command/src/lib.rs` | 166 | 145 | 21 | 87.3% |
| `server/lib/drivers/command/src/ex_registry.rs` | 152 | 142 | 10 | 93.4% |
| `server/lib/drivers/command/src/registry.rs` | 159 | 149 | 10 | 93.7% |
| `server/lib/drivers/command/src/query.rs` | 108 | 106 | 2 | 98.1% |
| `server/lib/drivers/command/src/ex_dispatch.rs` | 527 | 522 | 5 | 99.1% |

### reovim-client-tui (88.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `clients/tui/src/output/terminal.rs` | 56 | 0 | 56 | 0.0% |
| `clients/tui/src/grpc_client.rs` | 366 | 184 | 182 | 50.3% |
| `clients/tui/src/app.rs` | 593 | 352 | 241 | 59.4% |
| `clients/tui/src/adapter/mod.rs` | 151 | 107 | 44 | 70.9% |
| `clients/tui/src/output/headless.rs` | 23 | 17 | 6 | 73.9% |
| `clients/tui/src/input.rs` | 124 | 94 | 30 | 75.8% |
| `clients/tui/src/adapter/focus.rs` | 219 | 174 | 45 | 79.5% |
| `clients/tui/src/adapter/layout.rs` | 291 | 241 | 50 | 82.8% |
| `clients/tui/src/handle.rs` | 109 | 92 | 17 | 84.4% |
| `clients/tui/src/core_helpers.rs` | 312 | 283 | 29 | 90.7% |
| `clients/tui/src/adapter/overlay.rs` | 495 | 457 | 38 | 92.3% |
| `clients/tui/src/cli_render.rs` | 144 | 133 | 11 | 92.4% |
| `clients/tui/src/notification_handler.rs` | 813 | 770 | 43 | 94.7% |
| `clients/tui/src/adapter/anchor.rs` | 165 | 157 | 8 | 95.2% |
| `clients/tui/src/log_render.rs` | 186 | 177 | 9 | 95.2% |
| `clients/tui/src/render_backend.rs` | 323 | 310 | 13 | 96.0% |
| `clients/tui/src/render_core.rs` | 300 | 295 | 5 | 98.3% |
| `clients/tui/src/render_engine.rs` | 830 | 824 | 6 | 99.3% |
| `clients/tui/src/cli_panel.rs` | 575 | 571 | 4 | 99.3% |
| `clients/tui/src/lib.rs` | 162 | 161 | 1 | 99.4% |
| `clients/tui/src/log_buffer.rs` | 330 | 329 | 1 | 99.7% |

### reovim-driver-net (92.2%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/net/src/traits.rs` | 321 | 286 | 35 | 89.1% |
| `shared/net/src/handler.rs` | 197 | 180 | 17 | 91.4% |
| `shared/net/src/transport.rs` | 112 | 103 | 9 | 92.0% |
| `shared/net/src/local.rs` | 84 | 79 | 5 | 94.0% |
| `shared/net/src/error.rs` | 161 | 159 | 2 | 98.8% |

### reovim-module-clipboard (93.1%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/clipboard/src/service.rs` | 240 | 212 | 28 | 88.3% |
| `server/modules/clipboard/src/lib.rs` | 68 | 65 | 3 | 95.6% |

### reovim-driver-session (94.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/session/src/empty_handler.rs` | 405 | 287 | 118 | 70.9% |
| `server/lib/drivers/session/src/api/command.rs` | 27 | 23 | 4 | 85.2% |
| `server/lib/drivers/session/src/handler_registry.rs` | 66 | 57 | 9 | 86.4% |
| `server/lib/drivers/session/src/runtime.rs` | 2927 | 2756 | 171 | 94.2% |
| `server/lib/drivers/session/src/transition.rs` | 142 | 138 | 4 | 97.2% |
| `server/lib/drivers/session/src/testing.rs` | 430 | 419 | 11 | 97.4% |
| `server/lib/drivers/session/src/mode.rs` | 119 | 116 | 3 | 97.5% |

### reovim-module-editor (94.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/editor/src/command/operators.rs` | 174 | 151 | 23 | 86.8% |
| `server/modules/editor/src/command/undo.rs` | 374 | 333 | 41 | 89.0% |
| `server/modules/editor/src/command/yank.rs` | 373 | 341 | 32 | 91.4% |
| `server/modules/editor/src/command/file.rs` | 397 | 364 | 33 | 91.7% |
| `server/modules/editor/src/command/replace.rs` | 557 | 525 | 32 | 94.3% |
| `server/modules/editor/src/command/insert_edit.rs` | 592 | 560 | 32 | 94.6% |
| `server/modules/editor/src/command/cursor.rs` | 746 | 710 | 36 | 95.2% |
| `server/modules/editor/src/command/display_line.rs` | 752 | 718 | 34 | 95.5% |
| `server/modules/editor/src/command/paste.rs` | 842 | 806 | 36 | 95.7% |
| `server/modules/editor/src/command/delete.rs` | 836 | 803 | 33 | 96.1% |
| `server/modules/editor/src/command/mod.rs` | 838 | 806 | 32 | 96.2% |
| `server/modules/editor/src/lib.rs` | 81 | 78 | 3 | 96.3% |

### reovim-module-vim (95.3%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/vim/src/visual/exit.rs` | 224 | 189 | 35 | 84.4% |
| `server/modules/vim/src/resolvers/yank.rs` | 1002 | 889 | 113 | 88.7% |
| `server/modules/vim/src/resolvers/change.rs` | 1094 | 979 | 115 | 89.5% |
| `server/modules/vim/src/operators/change.rs` | 713 | 639 | 74 | 89.6% |
| `server/modules/vim/src/operators/commands.rs` | 665 | 602 | 63 | 90.5% |
| `server/modules/vim/src/operators/delete.rs` | 852 | 776 | 76 | 91.1% |
| `server/modules/vim/src/visual/entry.rs` | 413 | 378 | 35 | 91.5% |
| `server/modules/vim/src/commands/find_char.rs` | 387 | 355 | 32 | 91.7% |
| `server/modules/vim/src/commands/mode_entry.rs` | 870 | 809 | 61 | 93.0% |
| `server/modules/vim/src/commands/repeat.rs` | 475 | 442 | 33 | 93.1% |
| `server/modules/vim/src/operators/yank.rs` | 598 | 558 | 40 | 93.3% |
| `server/modules/vim/src/visual/manipulation.rs` | 576 | 541 | 35 | 93.9% |
| `server/modules/vim/src/commands/change.rs` | 592 | 557 | 35 | 94.1% |
| `server/modules/vim/src/commands/mode.rs` | 1385 | 1304 | 81 | 94.2% |
| `server/modules/vim/src/visual/mod.rs` | 625 | 591 | 34 | 94.6% |
| `server/modules/vim/src/visual/operators.rs` | 792 | 755 | 37 | 95.3% |
| `server/modules/vim/src/operators/registers.rs` | 201 | 192 | 9 | 95.5% |
| `server/modules/vim/src/fallback.rs` | 438 | 426 | 12 | 97.3% |
| `server/modules/vim/src/resolvers/delete.rs` | 1106 | 1076 | 30 | 97.3% |
| `server/modules/vim/src/bindings/mod.rs` | 120 | 117 | 3 | 97.5% |
| `server/modules/vim/src/resolvers/visual.rs` | 902 | 881 | 21 | 97.7% |
| `server/modules/vim/src/lib.rs` | 393 | 384 | 9 | 97.7% |
| `server/modules/vim/src/resolvers/window.rs` | 264 | 258 | 6 | 97.7% |
| `server/modules/vim/src/resolvers/normal.rs` | 1427 | 1396 | 31 | 97.8% |
| `server/modules/vim/src/operators/types.rs` | 302 | 296 | 6 | 98.0% |
| `server/modules/vim/src/annotation/line_number.rs` | 415 | 407 | 8 | 98.1% |
| `server/modules/vim/src/resolvers/operator_common.rs` | 514 | 506 | 8 | 98.4% |
| `server/modules/vim/src/resolvers/insert.rs` | 417 | 413 | 4 | 99.0% |
| `server/modules/vim/src/resolvers/commandline.rs` | 332 | 329 | 3 | 99.1% |
| `server/modules/vim/src/operators/mod.rs` | 115 | 114 | 1 | 99.1% |
| `server/modules/vim/src/modes.rs` | 283 | 281 | 2 | 99.3% |
| `server/modules/vim/src/session_state.rs` | 710 | 707 | 3 | 99.6% |
| `server/modules/vim/src/macros.rs` | 297 | 296 | 1 | 99.7% |
| `server/modules/vim/src/bindings/operator_modes.rs` | 1053 | 1050 | 3 | 99.7% |

### reovim-driver-ffi (95.5%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/ffi/src/timer.rs` | 305 | 282 | 23 | 92.5% |

### reovim-module-vfs-local (95.6%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/vfs-local/src/lib.rs` | 160 | 153 | 7 | 95.6% |

### reovim-driver-tui (95.7%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `clients/tui/lib/drivers/tui/src/terminal.rs` | 54 | 7 | 47 | 13.0% |
| `clients/tui/lib/drivers/tui/src/input.rs` | 484 | 437 | 47 | 90.3% |
| `clients/tui/lib/drivers/tui/src/screen.rs` | 389 | 383 | 6 | 98.5% |
| `clients/tui/lib/drivers/tui/src/cursor.rs` | 375 | 370 | 5 | 98.7% |
| `clients/tui/lib/drivers/tui/src/style.rs` | 603 | 596 | 7 | 98.8% |
| `clients/tui/lib/drivers/tui/src/frame/renderer.rs` | 314 | 312 | 2 | 99.4% |

### reovim-module-textobjects (95.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/textobjects/src/paragraph.rs` | 501 | 468 | 33 | 93.4% |
| `server/modules/textobjects/src/word.rs` | 737 | 702 | 35 | 95.3% |
| `server/modules/textobjects/src/quote.rs` | 887 | 853 | 34 | 96.2% |
| `server/modules/textobjects/src/lib.rs` | 82 | 79 | 3 | 96.3% |
| `server/modules/textobjects/src/bracket.rs` | 1151 | 1117 | 34 | 97.0% |

### reovim-module-window-ops (96.5%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/window-ops/src/command.rs` | 666 | 631 | 35 | 94.7% |
| `server/modules/window-ops/src/lib.rs` | 201 | 199 | 2 | 99.0% |
| `server/modules/window-ops/src/ids.rs` | 207 | 206 | 1 | 99.5% |

### reovim-kernel (96.6%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/kernel/src/ipc/event_bus/sender.rs` | 14 | 11 | 3 | 78.6% |
| `server/lib/kernel/src/core/config.rs` | 1130 | 915 | 215 | 81.0% |
| `server/lib/kernel/src/api/module/mod.rs` | 652 | 534 | 118 | 81.9% |
| `server/lib/kernel/src/sched/work_queue.rs` | 274 | 254 | 20 | 92.7% |
| `server/lib/kernel/src/debug/profiler.rs` | 274 | 259 | 15 | 94.5% |
| `server/lib/kernel/src/api/module/state.rs` | 111 | 105 | 6 | 94.6% |
| `server/lib/kernel/src/sched/timer.rs` | 556 | 527 | 29 | 94.8% |
| `server/lib/kernel/src/printk/macros.rs` | 85 | 81 | 4 | 95.3% |
| `server/lib/kernel/src/printk/logger.rs` | 98 | 94 | 4 | 95.9% |
| `server/lib/kernel/src/panic/recovery.rs` | 133 | 128 | 5 | 96.2% |
| `server/lib/kernel/src/api/module/error.rs` | 55 | 53 | 2 | 96.4% |
| `server/lib/kernel/src/sched/task.rs` | 303 | 292 | 11 | 96.4% |
| `server/lib/kernel/src/panic/report.rs` | 197 | 190 | 7 | 96.4% |
| `server/lib/kernel/src/ipc/events/kernel.rs` | 150 | 145 | 5 | 96.7% |
| `server/lib/kernel/src/sched/priority.rs` | 199 | 193 | 6 | 97.0% |
| `server/lib/kernel/src/api/debug.rs` | 148 | 144 | 4 | 97.3% |
| `server/lib/kernel/src/block/undo.rs` | 318 | 310 | 8 | 97.5% |
| `server/lib/kernel/src/ipc/event_bus/mod.rs` | 637 | 622 | 15 | 97.6% |
| `server/lib/kernel/src/core/textobj.rs` | 817 | 798 | 19 | 97.7% |
| `server/lib/kernel/src/mm/selection.rs` | 225 | 220 | 5 | 97.8% |
| `server/lib/kernel/src/core/mode.rs` | 381 | 373 | 8 | 97.9% |
| `server/lib/kernel/src/mm/delimiter.rs` | 336 | 329 | 7 | 97.9% |
| `server/lib/kernel/src/sched/runtime.rs` | 546 | 535 | 11 | 98.0% |
| `server/lib/kernel/src/ipc/scope.rs` | 231 | 227 | 4 | 98.3% |
| `server/lib/kernel/src/panic/handler.rs` | 64 | 63 | 1 | 98.4% |
| `server/lib/kernel/src/core/motion/engine.rs` | 1645 | 1620 | 25 | 98.5% |
| `server/lib/kernel/src/mm/snapshot.rs` | 219 | 216 | 3 | 98.6% |
| `server/lib/kernel/src/api/module/probe.rs` | 225 | 222 | 3 | 98.7% |
| `server/lib/kernel/src/mm/cache.rs` | 229 | 226 | 3 | 98.7% |
| `server/lib/kernel/src/ipc/event.rs` | 246 | 243 | 3 | 98.8% |
| `server/lib/kernel/src/core/option/value.rs` | 83 | 82 | 1 | 98.8% |
| `server/lib/kernel/src/mm/saturator.rs` | 287 | 284 | 3 | 99.0% |
| `server/lib/kernel/src/sched/executor.rs` | 215 | 213 | 2 | 99.1% |
| `server/lib/kernel/src/ipc/channel.rs` | 353 | 350 | 3 | 99.2% |
| `server/lib/kernel/src/debug/trace.rs` | 118 | 117 | 1 | 99.2% |
| `server/lib/kernel/src/core/option/mod.rs` | 640 | 637 | 3 | 99.5% |
| `server/lib/kernel/src/mm/buffer.rs` | 592 | 590 | 2 | 99.7% |

### reovim-driver-input (96.7%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/input/src/traits.rs` | 309 | 283 | 26 | 91.6% |
| `server/lib/drivers/input/src/resolver.rs` | 994 | 919 | 75 | 92.5% |
| `server/lib/drivers/input/src/fallback.rs` | 267 | 251 | 16 | 94.0% |
| `server/lib/drivers/input/src/mode_store.rs` | 255 | 243 | 12 | 95.3% |
| `server/lib/drivers/input/src/lifecycle.rs` | 56 | 54 | 2 | 96.4% |
| `server/lib/drivers/input/src/provider.rs` | 103 | 100 | 3 | 97.1% |
| `server/lib/drivers/input/src/key.rs` | 143 | 139 | 4 | 97.2% |
| `server/lib/drivers/input/src/lookup.rs` | 272 | 267 | 5 | 98.2% |
| `server/lib/drivers/input/src/resolver_registry.rs` | 583 | 580 | 3 | 99.5% |

### reovim-client-model (97.0%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/clients/model/src/traits/overlay.rs` | 201 | 171 | 30 | 85.1% |
| `shared/clients/model/src/wire/anchor.rs` | 56 | 53 | 3 | 94.6% |
| `shared/clients/model/src/wire/client.rs` | 133 | 126 | 7 | 94.7% |
| `shared/clients/model/src/interaction.rs` | 96 | 91 | 5 | 94.8% |
| `shared/clients/model/src/rendered/panel.rs` | 122 | 116 | 6 | 95.1% |
| `shared/clients/model/src/sync/presence.rs` | 125 | 119 | 6 | 95.2% |
| `shared/clients/model/src/rendered/overlay.rs` | 173 | 165 | 8 | 95.4% |
| `shared/clients/model/src/wire/layout.rs` | 190 | 186 | 4 | 97.9% |
| `shared/clients/model/src/rendered/window.rs` | 159 | 157 | 2 | 98.7% |
| `shared/clients/model/src/wire/presence.rs` | 86 | 85 | 1 | 98.8% |

### reovim-module-scratch-buffer (97.0%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/scratch-buffer/src/lib.rs` | 168 | 163 | 5 | 97.0% |

### reovim-driver-syntax-treesitter (97.3%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/syntax-treesitter/src/injection.rs` | 499 | 477 | 22 | 95.6% |
| `server/lib/drivers/syntax-treesitter/src/driver.rs` | 687 | 668 | 19 | 97.2% |

### reovim-arch (97.3%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/arch/src/unix/input.rs` | 30 | 23 | 7 | 76.7% |
| `shared/arch/src/unix/signal.rs` | 52 | 40 | 12 | 76.9% |
| `shared/arch/src/dirs.rs` | 135 | 115 | 20 | 85.2% |
| `shared/arch/src/unix/terminal.rs` | 153 | 143 | 10 | 93.5% |
| `shared/arch/src/palette.rs` | 119 | 112 | 7 | 94.1% |
| `shared/arch/src/unix/local.rs` | 169 | 166 | 3 | 98.2% |
| `shared/arch/src/error.rs` | 93 | 92 | 1 | 98.9% |
| `shared/arch/src/traits.rs` | 1375 | 1364 | 11 | 99.2% |
| `shared/arch/src/unix/convert.rs` | 594 | 591 | 3 | 99.5% |

### reovim-module-commands (97.7%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/commands/src/edit.rs` | 289 | 264 | 25 | 91.3% |

### reovim-module-defaults (98.3%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/defaults/src/lib.rs` | 177 | 174 | 3 | 98.3% |

### reovim-module-motions (98.4%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/motions/src/lib.rs` | 87 | 84 | 3 | 96.6% |
| `server/modules/motions/src/search.rs` | 1237 | 1195 | 42 | 96.6% |
| `server/modules/motions/src/find_char.rs` | 364 | 362 | 2 | 99.5% |
| `server/modules/motions/src/word.rs` | 748 | 745 | 3 | 99.6% |
| `server/modules/motions/src/line.rs` | 836 | 834 | 2 | 99.8% |

### reovim-module-buffer-simple (98.5%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/buffer-simple/src/lib.rs` | 195 | 192 | 3 | 98.5% |

### reovim-module-undo (98.5%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/undo/src/lib.rs` | 78 | 75 | 3 | 96.2% |
| `server/modules/undo/src/registry.rs` | 1160 | 1145 | 15 | 98.7% |

### reovim-module-search (99.0%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/search/src/lib.rs` | 68 | 65 | 3 | 95.6% |
| `server/modules/search/src/engine.rs` | 443 | 441 | 2 | 99.5% |

### reovim-driver-trace (99.3%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/trace/src/lib.rs` | 273 | 271 | 2 | 99.3% |

### reovim-module-treesitter-rust (99.3%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/treesitter-rust/src/lib.rs` | 725 | 720 | 5 | 99.3% |

### reovim-driver-lsp (99.5%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/lsp/src/config.rs` | 176 | 173 | 3 | 98.3% |

### reovim-driver-search (99.5%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/search/src/provider.rs` | 130 | 129 | 1 | 99.2% |

### reovim-protocol (99.6%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/protocol/src/instance/registry.rs` | 366 | 353 | 13 | 96.4% |
| `shared/protocol/src/v1/undo.rs` | 594 | 589 | 5 | 99.2% |

### reovim-driver-syntax (99.7%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/syntax/src/driver.rs` | 82 | 80 | 2 | 97.6% |
| `server/lib/drivers/syntax/src/factory.rs` | 117 | 115 | 2 | 98.3% |

### reovim-driver-vfs (99.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/vfs/src/path.rs` | 228 | 226 | 2 | 99.1% |
| `server/lib/drivers/vfs/src/standard.rs` | 436 | 435 | 1 | 99.8% |

## Uncovered Lines (top 20 files by miss count)

- `clients/cli/src/commands.rs` (**309** uncovered): `21-25,27,29-33,36,38-39,41,45-47,49,52,59-63,65-66,70,73-76,78,81,88-92,94-96,98-99,101-104,106,109,116-120,122,124-126,128-132,137,140-147,149,152,159-164,166,168-171,174-178,180,183,190-194,196-197,199-200,202,205,212-216,218-226,228-233,235,238,249-255,257,259-261,263-264,266-267,269,272-273,275,278-284,286,289,303-309,311,314,317-321,323,326,344-352,354,356-358,360-361,363-369,372,374,377-384,386,389,400-406,408-414,416-423,425,428,435-438,440,442,444-445,447,451-452,455,462-466,468,470-472,474-480,483-484,489,493-503,505,508,518-523,525,527,529-530,532,536-537,540,551-556,558-560,562-566,569,571-572,574,578-580,582,585`
- `shared/testing/src/step_test.rs` (**303** uncovered): `75-76,80,84-86,88,90,94,159-172,178-180,183-184,186-191,194,196-199,203,206-212,215-217,220,223-232,236-239,244-247,252-255,259-266,270-275,279-285,289-295,299-305,309-315,319-328,336,338-345,348,351-355,358-371,374,376-377,380,382-386,389,392-400,402-408,410-416,418-427,429-435,437-458,463,465,467-474,490-494,502-504,507-511,514-517,519,526,528-529,531,537-543,546,548-554,558-562,565-566,570-573,575-578,585-588,590-592,594-595,599-601,605-607`
- `clients/tui/src/app.rs` (**229** uncovered): `64-65,75-77,81-83,161,184,186-190,193-197,199,202-204,206,208,254-255,257-258,277-279,287-290,293-301,303-311,313-317,319,338-340,342-352,354-356,359-360,362-363,379-382,385,407,428-431,445-446,452-454,471,490-491,494-495,518,524-526,528-529,531-533,536-541,543-544,546-548,550-551,591-592,611-615,635,662,672,674-675,682-683,685,689-691,694-702,704-719,722-727,729,742-744,748-750,754-756,776-781,783-786,788,795-805,822-828,830,833,835-836,838-847,850-852`
- `apps/bin/src/main.rs` (**183** uncovered): `236-237,242-244,246,248-252,255,258-262,271,280-282,285-287,290-296,299,301-302,304-305,307-308,310-312,315-317,319,322-326,328,331-333,337-340,343-346,348-349,351,355,357,360-364,367-369,372-374,376-383,386-390,392-399,401-404,406-407,410,413,417,419-422,424,426,429-430,432-434,436,439,441-442,445,447-448,451-452,454-456,458-461,463-465,472-473,476,479,482-487,490-495,498-505,508,510-511,514-516,519-524,527-531,534,537,539-540`
- `server/lib/drivers/session/src/runtime.rs` (**166** uncovered): `829-831,835,863-865,872,938-940,943,974-976,982,1176,1556-1563,3247-3250,3252-3254,3263-3272,3274,3276-3278,3280-3282,3414-3416,3420-3428,3441-3443,3641-3643,3658-3660,3662,3664-3666,3681-3685,3687-3694,3696-3703,3970-3972,3974-3976,3978-3986,4025-4029,4031-4032,4035,4063,4067-4069,4090-4095,4097,4099-4101,4103-4105,4107-4112,4114-4119,4121,4123,4141-4143,4165-4167,4169-4179`
- `clients/tui/src/grpc_client.rs` (**162** uncovered): `54-55,63,68-69,74,79-80,85,89-90,123-125,225-229,258-268,310-311,313,338-339,341,362-363,365,400-404,476-480,487-491,502-506,541-545,556-563,570-574,585-589,606,618-628,639-649,666-683,698-704,716-724,735-739,784-791,839-843,850-854`
- `server/lib/drivers/session/src/empty_handler.rs` (**118** uncovered): `127-135,159-167,179-181,185-190,208-213,227,242-247,261,280-285,313-315,367,413-418,432-437,461-463,467-472,489,499-507,523-528,541,555-560,573,595-597,601-606,628-633,648-650,654-656,675`
- `server/lib/kernel/src/api/module/mod.rs` (**116** uncovered): `374-375,394-399,401,403-406,426-440,481-495,535-549,625-639,679-687,1103-1111,1202-1216,1225,1236-1244`
- `server/modules/vim/src/resolvers/change.rs` (**115** uncovered): `202-203,275-276,480-482,541,561,564,592,617,641,714,738,741,816-852,856-858,862-879,883-885,889-908,912-915,938,969,1011,1056,1183,1227,1271,1303,1337,1368,1403,1530,1568,1643`
- `server/modules/vim/src/resolvers/yank.rs` (**113** uncovered): `195-196,266-267,480,500,503,531,596,599,652,723-759,763-765,769-786,790-792,796-815,819-822,845,878,922,968,1095,1137,1176,1179,1219,1251,1282,1319,1385,1423,1498,1568,1571`
- `server/lib/drivers/command/src/traits.rs` (**106** uncovered): `143-144,150-151,162-164,171-177,192-197,211-216,230-235,258-263,290-296,317-322,337-342,361-366,393-398,427-433,450-452,455-461,477-479,482-488,508-510,563-569`
- `shared/testing/src/multi_client.rs` (**101** uncovered): `38-43,46-47,49-53,55,58,62-66,70-75,79-86,90-94,96-100,102-103,107-109,141,143-146,149-153,155,159-161,165-168,172-177,180-187,190-192,199-207,209-214,216,218`
- `shared/log/src/subscriber.rs` (**91** uncovered): `115-123,126-132,135-141,143-144,148,152,159,180-181,183-191,194-200,203-209,212,215-225,227-230,236-237,239,241-247,251-260,263,351`
- `server/lib/drivers/undo/src/provider.rs` (**82** uncovered): `529-530,548-587,610-649`
- `server/modules/vim/src/commands/mode.rs` (**78** uncovered): `173,549-554,563-571,573-575,577-579,585-592,1315-1336,1343-1351,1494,1752-1758,1760-1762,2001-2003,2061-2063`
- `server/lib/drivers/command/src/ex_handler.rs` (**77** uncovered): `362-363,404-416,431-443,581-586,609-614,643-648,675-680,723-735,916-921,953-958`
- `server/lib/drivers/input/src/resolver.rs` (**75** uncovered): `307-308,1131-1133,1150,1174,1200,1218,1244,1329,1352,1377,1380,1396,1636,1654,1668,1750-1756,1772-1778,1822-1824,1839-1841,1865-1867,1877-1879,1905-1907,1921-1923,1946-1948,1963-1965,1993-1995,2188-2190,2200-2202,2233-2235,2249-2251,2271,2288,2302,2319`
- `server/modules/vim/src/operators/delete.rs` (**73** uncovered): `109,125,160,197,213,283-290,327-332,341-349,351-353,355-357,921-929,941-961,1034,1084,1093-1098,1228`
- `server/modules/vim/src/operators/change.rs` (**71** uncovered): `96,103,116,134,209-216,253-258,267-275,277-279,281-283,711-719,731-751,799,897,910-915`
- `clients/cli/src/client.rs` (**66** uncovered): `76-78,170,187,190,198,203-204,212,217-218,223,227-228,340-344,373-377,384-388,399-406,420-424,426-432,561-576`

---
*Generated by `scripts/coverage-report.sh`*
