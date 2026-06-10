# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [0.16.0-dev] - Unreleased

### Added
- v0.16 workspace scaffold: sovereign depgraph probe engine (`lib/depgraph`,
  zero third-party dependencies in all three dependency tables) enforcing
  DAG1..DAG5 including the three-dep-table sovereignty walk and the
  `Cargo.lock` resolved-graph gate (`sovereignty_gate.rs`); `scripts/check.sh`
  and `scripts/coverage.sh`; per-PR CI with a dedicated `sovereignty` job
  (`cargo test -p reovim-depgraph --test sovereignty_gate`). (#783)

### Changed
- Spec: Zero-Std Sovereignty is law (`DAG6`, 1.2 §10) — every product
  crate is `#![no_std]` and `alloc`-free including `arch/`, which owns
  the platform floor (syscall FFI, `_start`, panic handler, allocator,
  sync, all heap data structures); workspace `panic = "abort"`; the
  North Star gains the Mission-to-Mars reliability doctrine. Protocol
  purity + carrier seam (`SP15`/`SP16`, 7.3 §1a): the wire protocol is
  sans-IO pure over caller-provided buffers (`SP13` re-signed,
  `encoded_size` sizing contract, `ErrorCode::BufferTooSmall`); the
  carrier (UDS/TCP default; HTTP/WebSocket/gRPC/file possible) is a
  replaceable byte-mover. AB12 panic isolation re-specified for
  no-unwinder reality: panic disposition (`recover`/`halt`), the
  panic-time final ring flush (one kernel log buffer, no second
  log), first-panic quarantine via persisted state.
  `std::` realization claims scrubbed to `arch/` contracts. (#784)
- Spec: Dependency Sovereignty is law (`DAG5`, 1.2 §9) — the workspace
  dependency graph is closed to std + in-repo crates across all three
  dependency tables; OS access via `arch/`-owned FFI; North Star
  (fastest-reaction, 50-year survivability) stated in the spec README. (#782)
- Spec: the server-client wire protocol is redesigned from gRPC to an
  in-house framed protocol (7.3 full rewrite: SP9..SP14, message
  inventory with tags, deterministic byte codec aligned with the 6.3
  catalog, Hello/HelloAck handshake, reject/ErrorCode model, worked
  byte-level golden); 6.3 registers `FrameHeader` and fixes the
  fieldless-enum repr convention; ~20 chapters scrubbed of
  third-party mechanism assumptions. (#782)
- `Documentation/` is now the normative spec SSOT (v4 draft): architecture,
  process, state, domain substrate, view, ABI, surfaces, client,
  conformance, and development process chapters. (#777)
- Pre-0.16 docs and CI workflows moved to `archive/` (reference only). (#777)
