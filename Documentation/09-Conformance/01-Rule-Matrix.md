# 9.1 — Conformance Rule Matrix

**Scope.** The cross-cutting promotion gate for v4: every locked
rule has a fixture, every normative MUST/MUST NOT/SHALL maps to a
rule, every ABI shape has a golden test.

**Heritage.** v4 README §16, §17, §18.

**Locked rules.** `CF1..CF5`.

---

## 1. The matrix

A row per locked rule:

| Field | Meaning |
|---|---|
| Rule ID | `<prefix><number>` |
| Source file | chapter that owns the rule |
| Enforcement class | compile / runtime / CI-depgraph / CI-spec / spec-asserted / convention |
| Positive fixture | what passing behaviour looks like |
| Negative fixture | what must fail |
| Expected error / status | exact error code, status, log shape |
| Observable event | DS12 / protocol / debug event, if applicable |

The matrix is **machine-readable**. Markdown rendering may
accompany, but CI must validate uniqueness, coverage, and chapter
references from the source-of-truth file.

## 2. Source-of-truth format (TODO)

Two candidates:

```toml
# Documentation/conformance/rules.toml
[[rule]]
id        = "CFG6"
chapter   = "06-ABI/04-Config-Slice.md"
class     = "abi-runtime"
positive  = "Round-trip: kernel → slice → participant; values match."
negative  = "Slice exceeds total_bytes cap → ConfigSliceTooLarge."
error     = "ConfigSliceTooLarge"
event     = "config.layer.rejected"
```

OR a single Markdown table file with strict format CI parses.

Decision pending; either form is acceptable as long as CI can
extract.

## 3. Promotion gates

> **CF1 — Every locked rule has a conformance row.** *Class*: CI/spec.

> **CF2 — Every normative MUST/MUST NOT/SHALL/REJECT/FAIL maps to a
> rule ID.** Free-form normative wording without an ID fails CI.
> *Class*: CI/spec.

> **CF3 — Drafts and unresolved alternatives are non-lockable.**
> A rule in `D` (draft) state is not enforceable; references to a
> draft from a locked rule fail CI unless the locked rule
> explicitly degrades on draft absence.
> *Class*: CI/spec.

> **CF4 — Golden tests required for ABI layouts, wire message
> fields, vtable offsets, ConfigSlice, DS12.** The golden artefacts are
> versioned, per-target-triple where layout depends on platform,
> and regenerated only by spec edit. Post-v1.0, golden artefacts
> for shipped stable surfaces are **permanent**: they may be added
> to, never edited or deleted (AB15 — the goldens are the
> userspace covenant's enforcement mechanism).
> *Class*: CI.

> **CF5 — Wire message structs and spec chapter stay aligned.** The
> hand-written message structs in `uapi/protocol` own the wire form;
> chapter 7.3 owns semantics. The **canonical statement** of the wire
> surface is the 7.3 §7 message-inventory table. CI verifies, against
> that table as the source of truth: every message type named in 7.3
> §7 exists as a `uapi/protocol` struct with the matching numeric
> tag, direction, and body-field set; a struct present without a §7
> row, or a §7 row without a struct, fails CI; tags are never reused
> or repurposed (AB15); the codec round-trip golden (CF4) for every
> struct is byte-identical to its fixture.
> *Class*: CI.
>
> > Heritage (non-normative): pre-sovereignty drafts split wire-form
> > ownership to generated `.proto` files; `DAG5` (1.2 §9) replaced
> > that with hand-written structs `(resolved #782)`.

## 4. Required golden tests

| Surface | Golden artefact |
|---|---|
| `VtableHeader` | offsets, size, alignment per triple |
| `AbiVersion` / `Version` | offsets, size |
| `ConfigSlice`, `ConfigKvp`, `ConfigValueKind`, `ConfigSource`, `ConfigValue` (union) | offsets, size, alignment per triple |
| `PositionHeader`, `CursorHeader` | byte order test (endian fixture) |
| `ServiceDescriptor` | offsets, size |
| `RawInput` and per-kind payloads | offsets per kind |
| Generated C header | round-trips through a C-side test that calls back into Rust |
| `uapi/protocol` wire message fields | wire-format golden bytes per message type; the 7.3 worked `Hello` frame bytes are a permanent fixture |
| `FrameHeader` (6.3) | wire-bytes fixture: the 16-byte little-endian prefix round-trips through encode/decode |
| DS12 event schema | JSON Schema golden file per event family |

Per-target triples: `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu`, `aarch64-apple-darwin`,
`x86_64-pc-windows-msvc`. Other triples are best-effort.

## 5. Uniqueness checks

- Rule IDs are globally unique. CI fails if two chapters define
  the same ID.
- Rule numbers within a namespace are dense or recorded gap.
  Deprecated rules (e.g. `AB5`, `AB6`) must carry a "DEPRECATED"
  marker pointing at the replacement.

## 6. Reshape notes

For every `R`-state rule (reshaped from v3), the owner chapter must
include a "Reshape note" naming what changed and why. CI checks the
presence; humans check the substance.

## Open items

1. Source-of-truth format pick (§2): TOML vs Markdown table.
2. Golden-test apparatus — language and harness.
3. Triples baseline — which fail closed vs warn.

## Conformance

This chapter governs other chapters' fixtures. CF1..CF4 enforced
by CI; the matrix structure described here is the implementation.
