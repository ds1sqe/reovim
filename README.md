# reovim

[![Crates.io](https://img.shields.io/crates/v/reovim.svg)](https://crates.io/crates/reovim)

A Rust-powered editing system.

## Project Goals

- **Fastest-reaction editor**: Minimal latency, instant response
- **Scalability**: Handles large files and complex operations
- **Zero-warning policy**: All code must compile warning-free

## Status

v0.16 is a **specification-first rebuild**. The v0.15.0 implementation
is preserved in-tree under [`archive/`](./archive/) as reference; the
next implementation is built against the normative specification in
[`Documentation/`](./Documentation/README.md), which is the single
source of truth for architecture, ABI, and conformance.

## Architecture

Reovim follows a **Linux kernel-inspired architecture**: the kernel
owns mechanism; Domains, modules, drivers, and client modules own
policy. Modules and drivers are runtime-loaded cdylibs crossing a
`#[repr(C)]` ABI — no Rust trait objects cross the boundary, and the
released ABI is treated as eternal (do not break userspace).

The full layer model, dependency DAG, type catalog, and locked rules
live in the specification:

- [Spec README](./Documentation/README.md) — posture, chapter index, rule namespaces
- [Vocabulary](./Documentation/00-Vocabulary.md) — cross-chapter terms
- [Layer Model](./Documentation/01-Architecture/01-Layer-Model.md) — foundation/contracts/kernel/runtime/ext/apps tiers
- [ABI Surface](./Documentation/06-ABI/01-Surface.md) and [Type Catalog](./Documentation/06-ABI/03-Type-Catalog.md) — the `#[repr(C)]` boundary
- [Conformance](./Documentation/09-Conformance/01-Rule-Matrix.md) — per-rule fixtures and golden tests
- [Heritage](./Documentation/heritage/README.md) — non-normative project history (proposals, phase records)

## Performance

Versioned benchmark reports for released implementations are kept in
[`perf/`](./perf/).

## Archive

- [`archive/`](./archive/) — the complete v0.15.0 implementation
  (workspace, scripts, configs). Reference only; non-normative.
- [`archive/docs/`](./archive/docs/) — pre-0.16 documentation for that
  implementation (architecture, user guide, contributing guides).
  Superseded by [`Documentation/`](./Documentation/README.md).
- [Legacy Documentation](https://github.com/ds1sqe/reovim/tree/81806439/archive/pre_kernel/docs) — pre-v0.9.0 documentation (removed from tree, preserved in git history)

## License

AGPL-3.0 - See [LICENSE](./LICENSE) for details.

For commercial licensing options, contact: ds1sqe@mensakorea.org
