# LLVM #119558 Minimal Reproducer

SIGSEGV in `llvm::coverage::CoverageMapping::getInstantiationGroups()` when
processing branch coverage data from Rust code using `#[tonic::async_trait]`.

## Environment

- rustc 1.95.0-nightly (db3e99bba 2026-02-04)
- LLVM 22.1.0-rust-1.95.0-nightly
- cargo-llvm-cov 0.8.4
- tonic 0.12.3
- Linux x86_64

## Reproduce

```bash
# Prerequisites
rustup toolchain install nightly
cargo install cargo-llvm-cov

# Run (crashes)
cargo +nightly llvm-cov --branch --lcov --output-path lcov.info

# Expected: LCOV report generated
# Actual: signal 11 (SIGSEGV) in getInstantiationGroups
```

## Root Cause Analysis

The crash occurs in `llvm-cov export` (and `report`/`show --summary-only`) when
processing coverage data compiled with `-Z coverage-options=branch`. The specific
trigger is `#[tonic::async_trait]` service implementations with **non-trivial
method bodies** (closures, `if let`, early returns, match arms).

### What crashes

- `llvm-cov export -format=lcov` (used by cargo-llvm-cov for --lcov)
- `llvm-cov report` (summary table)
- `llvm-cov show --summary-only` (per-file summary)

### What works

- `llvm-cov show --show-instantiations=false` (bypasses getInstantiationGroups)
- All formats when compiled WITHOUT `-Z coverage-options=branch` (line-only)
- Trivial async trait method bodies (just `Ok(...)` or `Err(...)`)

### Stack trace

```
0  libLLVM.so  llvm::sys::PrintStackTrace + 39
1  libLLVM.so  (signal handler)
2  libc.so.6   (signal trampoline)
3  libLLVM.so  llvm::coverage::CoverageMapping::getInstantiationGroups(llvm::StringRef) const + 319
4  llvm-cov    (export/report codepath)
```

### Isolation methodology

From a 65-crate Rust workspace (reovim), we tested all 167 source files with
coverage data individually using `llvm-cov show --sources <file>`. Exactly 8
files crashed — all implementing tonic gRPC service traits with non-trivial
async method bodies. Two files with the same `#[tonic::async_trait]` pattern
but trivial one-liner method bodies (returning `Ok`/`Err` immediately) did
NOT crash. This minimal reproducer distills the triggering pattern.

## Files

- `proto/greeter.proto` — 2-method gRPC service definition
- `build.rs` — tonic-build proto compilation
- `src/main.rs` — service impl with closures + branching inside async trait methods
- `Cargo.toml` — tonic 0.12, tokio, prost

## Workarounds

1. Use line-only coverage (no `--branch` flag)
2. Exclude affected crates from branch coverage
3. Use `llvm-cov show --show-instantiations=false` (works for HTML, not LCOV)
