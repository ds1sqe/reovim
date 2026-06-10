# 1.4 — Provenance

**Scope.** Which artefacts are spec-owned, which are generated,
which are heritage. Ownership of `.proto` files, generated bindings,
ABI golden files.

**Heritage.** v3 `01-Foundation/03-Provenance.md`; v4 README §0.4.

**Locked rules.** None at this revision; resolution items below.

---

## 1. Spec-owned vs generated

| Artefact | Source of truth | Generated form | Notes |
|---|---|---|---|
| Type Catalog | `06-ABI/03-Type-Catalog.md` | `uapi/<crate>/src/*.rs`, generated C header | Catalog is normative; code mirrors it. |
| `.proto` files | wire form (field numbers, message shapes, service signatures): the `.proto` files themselves; semantics (capability rules, error mapping, audit events): `07-Surfaces/03-Server-Client-Protocol.md` | `uapi/protocol/src/gen/*.rs` | Split ownership. CI cross-checks chapter ↔ proto alignment (CF5). |
| Generated proto bindings | `*.proto` | `uapi/protocol/src/gen/*.rs` | Implementation-generated; not spec-owned. |
| Vtable layouts | `06-ABI/02-Versioning-and-Vtables.md` | `uapi/{module,driver}-macros/` expansions | Macros encode the spec-defined header + slot rules. |
| ConfigSlice layout | `06-ABI/04-Config-Slice.md` | implementation `#[repr(C)]` | Layout golden tests bind both. |
| Carrier headers | `04-Domain-Substrate/05-Coordination.md` | `uapi/coordination/src/*.rs` (TODO) | Spec defines wire bytes; impl mirrors. |
| DS12 event schema | `09-Conformance/04-Observability.md` | implementation event types | Schema is normative. |

## 2. Heritage and archive

- `archive/*` — the v0.15.0 implementation and its docs. Non-normative;
  excluded from depgraph (DAG1).
- `Documentation/heritage/*` — historical narrative (proposals, phase
  records, memorials). Non-normative.
- `archive/docs/architecture/client/` (v6.3 CLM) — heritage. Banner pending.

## 3. v3 → v4 carry status

| v3 chapter | v4 disposition |
|---|---|
| `01-Foundation/01-Model.md` | folded into 1.1 + 4.1 |
| `01-Foundation/02-Linux-Mappings.md` | retained reference; not re-folded |
| `01-Foundation/03-Provenance.md` | this chapter |
| `01-Foundation/04-Apps-Layout.md` | folded into 1.3 + 1.2 |
| `01-Foundation/05-Editor-Invocation.md` | folded into 1.3 |
| `01-Foundation/06-Configuration.md` | superseded by 1.5 + 6.4 + 7.4 |
| `02-Process/01-Types.md` | folded into 2.1 |
| `02-Process/02-Lifecycle.md` | folded into 2.2 |
| `02-Process/03-Concurrency.md` | folded into 2.3 |
| `02-Process/04-Persistence.md` | folded into 2.4 |
| `03-State/01-State-Split.md` | folded into 3.1 |
| `03-State/02-Module-State.md` | folded into 3.2 |
| `03-State/03-Services.md` | superseded by 3.3 |
| `03-State/04-Registers.md` | folded into 3.4 |
| `04-Substrate/01-Domain.md` | folded into 4.1 |
| `04-Substrate/02-Domain-Tree.md` | folded into 4.2 |
| `04-Substrate/03-Undo.md` | folded into 4.3 |
| `04-Substrate/04-Stream.md` | folded into 4.4 |
| `05-View/01-Windows.md` | folded into 5.1 |
| `05-View/02-Input-Codec.md` | superseded by 5.2 |
| `06-ABI/01-Surface.md` | folded into 6.1 |
| `06-ABI/02-Versioning.md` | folded into 6.2 |
| `06-ABI/03-Type-Catalog.md` | folded into 6.3 |
| `07-Surfaces/01-Package-Manager.md` | folded into 7.1 |
| `07-Surfaces/02-Debug-Surface.md` | folded into 7.2 |
| `07-Surfaces/03-Server-Client-Protocol.md` | folded into 7.3 |

## Open items

1. ~~`.proto` provenance~~ — resolved (CF5): proto files
   own the wire form and are hand-edited as source; the spec
   chapter owns semantics; CI cross-checks the two (every message
   and RPC named in 7.3 exists in a `.proto`; field numbers in
   chapter examples match; removals flag as breaks). Generated
   bindings (`uapi/protocol/src/gen/*.rs`) are in-repo; CI
   verifies regeneration is byte-identical.
2. Golden-file ownership: who regenerates ABI offsets when a
   `#[repr(C)]` struct grows a field? Spec edit must precede code
   edit; conformance row in 9.1 must pass.
3. ~~v3 Open Question files~~ — resolved (option D): v3
   `Open/` stays as heritage with one-line v4-disposition banners;
   v4 `Open/` is the operational lock-blocker tracker.

## Conformance

This chapter has no rules. Provenance is enforced by
`09-Conformance/01-Rule-Matrix.md` golden-test requirements.
