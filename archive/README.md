# Archived Legacy Code

This directory contains legacy code from pre-kernel architecture (v0.8.x).
These crates are **NOT** part of the active workspace.

## Why Archived

Reovim transitioned to a Linux kernel-inspired architecture in v0.9.0:
- `lib/core/` → replaced by `lib/kernel/`
- `lib/sys/` → functionality moved to `lib/arch/`
- `lib/lsp/` → replaced by `lib/drivers/lsp/`
- `runner/` → replaced by new clean-architecture runner
- `plugins/` → to be reimplemented as policy modules

## Contents

| Directory | Description | Lines |
|-----------|-------------|-------|
| `lib/core/` | Legacy buffer, cursor, mode, events | ~54,000 |
| `lib/sys/` | Legacy system utilities | ~2,000 |
| `lib/lsp/` | Legacy LSP client | ~3,000 |
| `runner/` | Legacy main binary | ~5,000 |
| `tools/bench/` | Legacy benchmarks | ~3,000 |
| `plugins/` | 26 legacy plugins | ~20,000 |

## Reference Only

This code is preserved for reference but is NOT actively maintained.
For current development, see root-level directories.

## Workspace Manifest

See `Cargo.toml.legacy` for the old workspace configuration.
