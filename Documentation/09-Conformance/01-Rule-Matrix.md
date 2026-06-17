# 9.1 — Conformance Rule Matrix

**Scope.** The cross-cutting promotion gate: every locked
rule has a fixture, every normative MUST/MUST NOT/SHALL maps to a
rule, every ABI shape has a golden test.

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

New spec-first rows — the probes and suites land with their enforcement
flights, exactly as `DAG5` was spec-first before the workspace scaffold
built it:

| Rule | Class | Fixture |
|---|---|---|
| `DAG6` (1.2 §10) | CI-depgraph (spec-first) | Positive: every product crate root declares `#![no_std]`; product sources contain no `std`/`alloc` use; workspace profile is `panic = "abort"`. Negative (three fixtures): a product crate missing `#![no_std]`; a `use std::` in product code; a `use alloc::` in product code — each expects **fail**. Bootstrap-state crates are enumerated in the probe, not pattern-matched. |
| `SP15` (7.3 §1a) | CI-depgraph (spec-first) | Purity probe: `uapi/protocol` imports nothing but `core` and `uapi` siblings. Negative: an `arch/` (or any IO-capable) dependency or import in `uapi/protocol` expects **fail**. |
| `SP16` (7.3 §1a) | spec-asserted + structural | The §3 frame format and §6 codec contain no carrier-specific reference; the carrier contract is satisfiable by a file-IO fixture carrier (frames written to and re-read from a file decode identically — exercised once the codec lands). |
| `AB16` (6.5 §5.1) | spec-asserted | A platform provider is valid iff it passes the `platform-conformance` behavioral suite; the zero-arch `platform-linux-mock` passes the **same** suite (no reference provider). Append-only: a new fixture tightens the contract, never invalidates a conforming provider. Suite + fixtures land in the 05d sub-plans (Phase 5); normative now, implementation deferred. |
| `DAG7` (1.2 §12) | CI-depgraph (spec-first) | Tier-1 firewall: no `uapi/*` or `kabi/*` crate names an `arch-*`/`platform-*` crate. Negative: a `kabi/*` crate adds an `arch-*` path dep → **fail**. Extends `firewall_probe.rs` from the Tier-2 residual (`lib/ds ⊄ arch`) to Tier-1; probe extension lands in the 05d sub-plans (Phase 9), spec-first until then. |
| `DAG5` amend (1.2 §9.1) | CI-depgraph | Include-tool clause: the sovereignty probe also bans external crates in host/build tooling — `[build-dependencies]` and `tools/*`/`lib/depgraph` deps included; `linkme`/`inventory` named forbidden. Widens the already-enforced `DAG5` probe scope; distinct from `DAG6`'s std-in-tooling bootstrap rows. |

The last three rows (`AB16`, `DAG7`, `DAG5` amend) ratify the
POSIX-personality model (`01-Architecture/06-OS-Modes.md`,
`06-ABI/05-Platform-Contract.md`, `01-Architecture/02-Project-Layout-and-DAG.md`
§9.1/§12). `AB16` and `DAG7` are spec-first — their suite and probe
implementations land in the 05d sub-plans; `DAG5`'s include-tool clause
widens an already-enforced probe.

Phase 4 substrate rows added for #798 remain in the owner chapters
until §2 chooses the machine-readable source format. The rows are
mandatory inputs to that eventual matrix and cover: DT10/DT17
dispatch ordering, DT11..DT16 attachment/focus semantics, undo group
boundaries and external markers, stream S1..S10 registration and
backpressure, CR1..CR19 carrier validation/fallback, view-slot
scope/diagnostics, service leases/flags, register scope lookup, and
projection composition/full-resend boundaries.

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

For every `R`-state rule (reshaped), the owner chapter must
include a "Reshape note" naming what changed and why. CI checks the
presence; humans check the substance.

## Open items

1. Source-of-truth format pick (§2): TOML vs Markdown table.
2. Golden-test apparatus — language and harness.
3. Triples baseline — which fail closed vs warn.

## Conformance

This chapter governs other chapters' fixtures. CF1..CF4 enforced
by CI; the matrix structure described here is the implementation.
