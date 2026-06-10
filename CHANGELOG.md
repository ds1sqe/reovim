# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [0.16.0-dev] - Unreleased

### Changed
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
