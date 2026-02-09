# Workspace MC/DC Coverage Report

Generated: 2026-02-09 05:11 | Mode: mcdc | Target: 100%

**Overall**: 99.3% lines (93049/93697) | 353 files

## Crate Summary

| Crate | Files | Lines | Hit | Miss | Line % | Status |
|-------|------:|------:|----:|-----:|-------:|--------|
| reovim-client-cli | 3 | 83 | 75 | 8 | 90.4% | 90%+ |
| reovim-kernel | 71 | 16015 | 15552 | 463 | 97.1% | 90%+ |
| reovim-testing | 2 | 79 | 77 | 2 | 97.5% | 90%+ |
| reovim-driver-log | 3 | 724 | 714 | 10 | 98.6% | 90%+ |
| reovim-module-undo | 2 | 1307 | 1298 | 9 | 99.3% | 90%+ |
| reovim-client-tui | 24 | 6063 | 6026 | 37 | 99.4% | 90%+ |
| reovim-driver-syntax-treesitter | 3 | 1072 | 1066 | 6 | 99.4% | 90%+ |
| reovim-arch | 10 | 2177 | 2168 | 9 | 99.6% | 90%+ |
| reovim-module-vim | 43 | 18589 | 18533 | 56 | 99.7% | 90%+ |
| reovim-driver-undo | 3 | 387 | 386 | 1 | 99.7% | 90%+ |
| reovim-driver-tui | 8 | 2539 | 2533 | 6 | 99.8% | 90%+ |
| reovim-driver-ffi | 4 | 432 | 431 | 1 | 99.8% | 90%+ |
| reovim-client-model | 20 | 2351 | 2346 | 5 | 99.8% | 90%+ |
| reovim-driver-session | 19 | 5378 | 5367 | 11 | 99.8% | 90%+ |
| reovim-module-window-ops | 3 | 1020 | 1018 | 2 | 99.8% | 90%+ |
| reovim-module-motions | 5 | 2964 | 2960 | 4 | 99.9% | 90%+ |
| reovim-driver-input | 16 | 3561 | 3557 | 4 | 99.9% | 90%+ |
| reovim-module-editor | 14 | 6470 | 6463 | 7 | 99.9% | 90%+ |
| reovim-module-textobjects | 5 | 4403 | 4399 | 4 | 99.9% | 90%+ |
| reovim-protocol | 18 | 4371 | 4368 | 3 | 99.9% | 90%+ |
| reovim-module-clipboard | 3 | 398 | 398 | 0 | 100.0% | PASS |
| reovim-driver-command | 8 | 1309 | 1309 | 0 | 100.0% | PASS |
| reovim-driver-net | 5 | 507 | 507 | 0 | 100.0% | PASS |
| reovim-driver-search | 3 | 202 | 202 | 0 | 100.0% | PASS |
| reovim-driver-clipboard | 3 | 196 | 196 | 0 | 100.0% | PASS |
| reovim-module-buffer-simple | 1 | 198 | 198 | 0 | 100.0% | PASS |
| reovim-driver-lsp | 5 | 578 | 578 | 0 | 100.0% | PASS |
| reovim-module-scratch-buffer | 1 | 147 | 147 | 0 | 100.0% | PASS |
| reovim-driver-trace | 1 | 279 | 279 | 0 | 100.0% | PASS |
| reovim-module-search | 2 | 520 | 520 | 0 | 100.0% | PASS |
| reovim-driver-vfs | 12 | 2996 | 2996 | 0 | 100.0% | PASS |
| reovim-driver-syntax | 9 | 1420 | 1420 | 0 | 100.0% | PASS |
| reovim-module-vfs-local | 1 | 157 | 157 | 0 | 100.0% | PASS |
| reovim-module-keymap | 2 | 590 | 590 | 0 | 100.0% | PASS |
| reovim-module-buffer-ops | 1 | 191 | 191 | 0 | 100.0% | PASS |
| reovim-module-commands | 7 | 1064 | 1064 | 0 | 100.0% | PASS |
| reovim-module-treesitter-rust | 1 | 618 | 618 | 0 | 100.0% | PASS |
| reovim-module-treesitter-markdown | 1 | 422 | 422 | 0 | 100.0% | PASS |
| reovim-driver-buffer | 2 | 109 | 109 | 0 | 100.0% | PASS |
| reovim-module-defaults | 1 | 176 | 176 | 0 | 100.0% | PASS |
| reovim-module-options | 1 | 536 | 536 | 0 | 100.0% | PASS |
| reovim-module-mode-manager | 1 | 230 | 230 | 0 | 100.0% | PASS |
| reovim-driver-command-types | 4 | 748 | 748 | 0 | 100.0% | PASS |
| reovim-app | 2 | 121 | 121 | 0 | 100.0% | PASS |
| **Total** | **353** | **93697** | **93049** | **648** | **99.3%** | |

## Gaps (files below 100%)

### reovim-client-cli (90.4%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `clients/cli/src/lib.rs` | 66 | 58 | 8 | 87.9% |

### reovim-kernel (97.1%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/kernel/src/panic/handler.rs` | 70 | 48 | 22 | 68.6% |
| `server/lib/kernel/src/printk/logger.rs` | 120 | 94 | 26 | 78.3% |
| `server/lib/kernel/src/core/config.rs` | 1063 | 860 | 203 | 80.9% |
| `server/lib/kernel/src/debug/profiler.rs` | 206 | 168 | 38 | 81.6% |
| `server/lib/kernel/src/sched/work_queue.rs` | 255 | 238 | 17 | 93.3% |
| `server/lib/kernel/src/printk/macros.rs` | 85 | 81 | 4 | 95.3% |
| `server/lib/kernel/src/sched/timer.rs` | 482 | 460 | 22 | 95.4% |
| `server/lib/kernel/src/api/module/error.rs` | 24 | 23 | 1 | 95.8% |
| `server/lib/kernel/src/panic/recovery.rs` | 133 | 128 | 5 | 96.2% |
| `server/lib/kernel/src/sched/task.rs` | 270 | 260 | 10 | 96.3% |
| `server/lib/kernel/src/panic/report.rs` | 197 | 190 | 7 | 96.4% |
| `server/lib/kernel/src/ipc/events/kernel.rs` | 144 | 140 | 4 | 97.2% |
| `server/lib/kernel/src/ipc/event_bus/mod.rs` | 597 | 582 | 15 | 97.5% |
| `server/lib/kernel/src/mm/selection.rs` | 225 | 220 | 5 | 97.8% |
| `server/lib/kernel/src/sched/runtime.rs` | 510 | 499 | 11 | 97.8% |
| `server/lib/kernel/src/core/mode.rs` | 371 | 363 | 8 | 97.8% |
| `server/lib/kernel/src/block/undo.rs` | 318 | 312 | 6 | 98.1% |
| `server/lib/kernel/src/sched/priority.rs` | 204 | 201 | 3 | 98.5% |
| `server/lib/kernel/src/core/textobj.rs` | 838 | 826 | 12 | 98.6% |
| `server/lib/kernel/src/mm/snapshot.rs` | 219 | 216 | 3 | 98.6% |
| `server/lib/kernel/src/api/module/probe.rs` | 222 | 219 | 3 | 98.6% |
| `server/lib/kernel/src/ipc/event.rs` | 229 | 226 | 3 | 98.7% |
| `server/lib/kernel/src/core/option/value.rs` | 83 | 82 | 1 | 98.8% |
| `server/lib/kernel/src/mm/saturator.rs` | 253 | 250 | 3 | 98.8% |
| `server/lib/kernel/src/mm/delimiter.rs` | 344 | 340 | 4 | 98.8% |
| `server/lib/kernel/src/core/motion/engine.rs` | 1618 | 1602 | 16 | 99.0% |
| `server/lib/kernel/src/sched/executor.rs` | 215 | 213 | 2 | 99.1% |
| `server/lib/kernel/src/ipc/channel.rs` | 353 | 350 | 3 | 99.2% |
| `server/lib/kernel/src/core/option/mod.rs` | 640 | 637 | 3 | 99.5% |
| `server/lib/kernel/src/ipc/scope.rs` | 235 | 234 | 1 | 99.6% |
| `server/lib/kernel/src/mm/buffer.rs` | 587 | 585 | 2 | 99.7% |

### reovim-testing (97.5%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/testing/src/harness.rs` | 31 | 29 | 2 | 93.5% |

### reovim-driver-log (98.6%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/log/src/subscriber.rs` | 416 | 406 | 10 | 97.6% |

### reovim-module-undo (99.3%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/undo/src/registry.rs` | 1226 | 1217 | 9 | 99.3% |

### reovim-client-tui (99.4%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `clients/tui/src/app.rs` | 13 | 11 | 2 | 84.6% |
| `clients/tui/src/adapter/anchor.rs` | 165 | 157 | 8 | 95.2% |
| `clients/tui/src/render_core.rs` | 300 | 295 | 5 | 98.3% |
| `clients/tui/src/render_backend.rs` | 293 | 289 | 4 | 98.6% |
| `clients/tui/src/handle.rs` | 93 | 92 | 1 | 98.9% |
| `clients/tui/src/render_engine.rs` | 830 | 824 | 6 | 99.3% |
| `clients/tui/src/cli_panel.rs` | 575 | 571 | 4 | 99.3% |
| `clients/tui/src/lib.rs` | 162 | 161 | 1 | 99.4% |
| `clients/tui/src/adapter/focus.rs` | 164 | 163 | 1 | 99.4% |
| `clients/tui/src/input.rs` | 179 | 178 | 1 | 99.4% |
| `clients/tui/src/adapter/overlay.rs` | 366 | 364 | 2 | 99.5% |
| `clients/tui/src/core_helpers.rs` | 284 | 283 | 1 | 99.6% |
| `clients/tui/src/log_buffer.rs` | 324 | 323 | 1 | 99.7% |

### reovim-driver-syntax-treesitter (99.4%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/syntax-treesitter/src/injection.rs` | 418 | 414 | 4 | 99.0% |
| `server/lib/drivers/syntax-treesitter/src/driver.rs` | 317 | 315 | 2 | 99.4% |

### reovim-arch (99.6%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/arch/src/palette.rs` | 56 | 54 | 2 | 96.4% |
| `shared/arch/src/unix/local.rs` | 150 | 148 | 2 | 98.7% |
| `shared/arch/src/traits.rs` | 1214 | 1209 | 5 | 99.6% |

### reovim-module-vim (99.7%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/vim/src/bindings/mod.rs` | 120 | 117 | 3 | 97.5% |
| `server/modules/vim/src/operators/types.rs` | 302 | 296 | 6 | 98.0% |
| `server/modules/vim/src/operators/mod.rs` | 94 | 93 | 1 | 98.9% |
| `server/modules/vim/src/resolvers/yank.rs` | 508 | 504 | 4 | 99.2% |
| `server/modules/vim/src/operators/delete.rs` | 696 | 691 | 5 | 99.3% |
| `server/modules/vim/src/modes.rs` | 283 | 281 | 2 | 99.3% |
| `server/modules/vim/src/operators/change.rs` | 578 | 574 | 4 | 99.3% |
| `server/modules/vim/src/visual/operators.rs` | 744 | 739 | 5 | 99.3% |
| `server/modules/vim/src/resolvers/change.rs` | 602 | 598 | 4 | 99.3% |
| `server/modules/vim/src/resolvers/delete.rs` | 636 | 632 | 4 | 99.4% |
| `server/modules/vim/src/commands/change.rs` | 539 | 536 | 3 | 99.4% |
| `server/modules/vim/src/annotation/line_number.rs` | 409 | 407 | 2 | 99.5% |
| `server/modules/vim/src/operators/yank.rs` | 548 | 546 | 2 | 99.6% |
| `server/modules/vim/src/operators/commands.rs` | 581 | 579 | 2 | 99.7% |
| `server/modules/vim/src/visual/mod.rs` | 584 | 582 | 2 | 99.7% |
| `server/modules/vim/src/macros.rs` | 297 | 296 | 1 | 99.7% |
| `server/modules/vim/src/lib.rs` | 315 | 314 | 1 | 99.7% |
| `server/modules/vim/src/bindings/operator_modes.rs` | 1053 | 1050 | 3 | 99.7% |
| `server/modules/vim/src/resolvers/normal.rs` | 1037 | 1036 | 1 | 99.9% |
| `server/modules/vim/src/commands/mode.rs` | 1165 | 1164 | 1 | 99.9% |

### reovim-driver-undo (99.7%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/undo/src/provider.rs` | 337 | 336 | 1 | 99.7% |

### reovim-driver-tui (99.8%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `clients/tui/lib/drivers/tui/src/input.rs` | 440 | 437 | 3 | 99.3% |
| `clients/tui/lib/drivers/tui/src/frame/renderer.rs` | 314 | 312 | 2 | 99.4% |
| `clients/tui/lib/drivers/tui/src/screen.rs` | 384 | 383 | 1 | 99.7% |

### reovim-driver-ffi (99.8%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/ffi/src/timer.rs` | 234 | 233 | 1 | 99.6% |

### reovim-client-model (99.8%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/clients/model/src/interaction.rs` | 113 | 108 | 5 | 95.6% |

### reovim-driver-session (99.8%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/session/src/types.rs` | 668 | 665 | 3 | 99.6% |
| `server/lib/drivers/session/src/runtime.rs` | 2653 | 2646 | 7 | 99.7% |
| `server/lib/drivers/session/src/testing.rs` | 395 | 394 | 1 | 99.7% |

### reovim-module-window-ops (99.8%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/window-ops/src/lib.rs` | 201 | 199 | 2 | 99.0% |

### reovim-module-motions (99.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/motions/src/search.rs` | 1088 | 1085 | 3 | 99.7% |
| `server/modules/motions/src/word.rs` | 680 | 679 | 1 | 99.9% |

### reovim-driver-input (99.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/lib/drivers/input/src/key.rs` | 135 | 131 | 4 | 97.0% |

### reovim-module-editor (99.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/editor/src/command/cursor.rs` | 668 | 664 | 4 | 99.4% |
| `server/modules/editor/src/command/file.rs` | 311 | 310 | 1 | 99.7% |
| `server/modules/editor/src/command/display_line.rs` | 766 | 764 | 2 | 99.7% |

### reovim-module-textobjects (99.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `server/modules/textobjects/src/word.rs` | 940 | 938 | 2 | 99.8% |
| `server/modules/textobjects/src/quote.rs` | 1211 | 1210 | 1 | 99.9% |
| `server/modules/textobjects/src/bracket.rs` | 1580 | 1579 | 1 | 99.9% |

### reovim-protocol (99.9%)

| File | Lines | Hit | Miss | Coverage |
|------|------:|----:|-----:|---------:|
| `shared/protocol/src/instance/registry.rs` | 365 | 362 | 3 | 99.2% |

## Uncovered Lines (top 20 files by miss count)

- `server/lib/kernel/src/core/config.rs` (**136** uncovered): `71,79,83,88,92,97,101,106,110,115,119,124,128,133,137-143,145,150-152,156-158,162-164,168-170,174-176,180-182,214-218,220-223,225,270-272,276-278,285-286,291-292,302-305,309-311,315-317,321-323,327-329,333-335,339-341,345-347,357-359,362-364,369,374,384-386,390-391,397-398,404-405,410,413,420-421,427-428,435,440-442,447,452-455,478-482,493-497,508-512,522-523,533-534`
- `server/lib/kernel/src/debug/profiler.rs` (**38** uncovered): `368-376,378-380,382-387,391-393,398-403,437-442,448-452`
- `server/lib/kernel/src/panic/handler.rs` (**22** uncovered): `103,106-110,113-115,118,121-123,126-128,146-148,152-154`
- `server/lib/kernel/src/core/motion/engine.rs` (**14** uncovered): `199-201,261,288-290,405-406,451-452,540,700,777`
- `server/lib/kernel/src/ipc/event_bus/mod.rs` (**11** uncovered): `297,378,790-792,845-847,1236-1238`
- `server/lib/kernel/src/core/textobj.rs` (**10** uncovered): `333-334,429,528,549,552,554,586,602,613`
- `server/lib/kernel/src/printk/logger.rs` (**10** uncovered): `161-163,179,193,202,204,211,218,276`
- `clients/tui/src/adapter/anchor.rs` (**8** uncovered): `153,173,188,203,218,233,248,273`
- `clients/cli/src/lib.rs` (**8** uncovered): `281,283,303,305,316,327,339,341`
- `server/lib/kernel/src/core/mode.rs` (**8** uncovered): `412-414,420-422,424-425`
- `clients/tui/src/render_engine.rs` (**6** uncovered): `259,358,470,474,482,948`
- `server/lib/kernel/src/block/undo.rs` (**6** uncovered): `254-255,377-378,555,561`
- `server/modules/vim/src/operators/types.rs` (**6** uncovered): `152-154,159-161`
- `server/lib/kernel/src/mm/selection.rs` (**5** uncovered): `241,264,272,282,289`
- `server/modules/undo/src/registry.rs` (**5** uncovered): `331,397,402,549-550`
- `server/modules/vim/src/operators/delete.rs` (**5** uncovered): `109,125,160,197,213`
- `server/lib/kernel/src/panic/report.rs` (**5** uncovered): `261-262,297-298,330`
- `server/modules/vim/src/visual/operators.rs` (**5** uncovered): `107,169,222,227,348`
- `shared/clients/model/src/interaction.rs` (**5** uncovered): `161,171,196,208,218`
- `server/lib/kernel/src/panic/recovery.rs` (**4** uncovered): `112,161-162,204`

---
*Generated by `scripts/coverage-report.sh`*
