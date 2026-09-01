# State, Semantics, Clients, and Extensions

> **Normative concept chapter.** Part of the
> [Editing-System Concept](../00-Concept.md).

## Resource and state authority

Resource capabilities are independent, composable dimensions. Source access,
source stability, Snapshot consistency and freshness, mutation grammar,
destination-creation authority, and change-discovery strength are negotiated
separately. Read-Only Access does not mean the external subject is stable, and
Snapshot immutability does not mean all future observations are identical.

One Buffer is the transactional working-state and History authority for exactly
one Resource. A Resource may have several explicitly identified Buffers, but a
Buffer never silently rebinds to another Resource. Cross-Resource operations
coordinate several Buffers above this boundary.

Local acceptance and external publication are separate outcomes. A Buffer over
Read-Only Access may accept local Transactions as a derived working branch when
its contract permits it. Publishing back to that Resource remains unsupported.
Creating a different Resource requires an independently authorized, immutable,
fully stamped Resource Creation Intent. Creation never mutates or silently
rebinds the source Resource or Buffer.

An Add-Only Resource defines a Resource-native monotonic addition relation.
Undo before proved external application remains a local Buffer transition. Undo
after application cannot erase the external addition; compensation is a new
external effect.

## Semantic and presentation boundaries

The Kernel knows a coordinate or value only through a negotiated contract,
qualified carrier, and exact state stamps. Resource Addresses and Units belong
to a Resource family. Semantic coordinates and values belong to a Domain or
Representation. Presentation coordinates and values belong to a Client and
View.

An intrinsic image pixel may be Resource data or a schema-qualified semantic
value. A target-screen pixel, terminal cell, glyph, viewport transform, draw
list, or frame command is presentation state and never becomes Kernel semantic
authority.

A faithful low-level Representation accounts for every source Unit in its
scope. A high-level Representation may expose only what its Domain understands,
but an editable high-level round trip preserves all unexpressed data or rejects
the edit. A form that cannot satisfy this rule is derived and read-only.

Edits from every editable View enter as typed Interaction Intents. Domain and
Representation policy validate the stamped semantic correspondence and fully
lower the intent into a Transaction. Neither a Client nor an extension mutates
concrete storage or a second private content buffer.

## Client-owned View boundary

A **Client** is a public consumer of the Kernel interaction and
Representation-update contracts. A **View** is Client-owned presentation and
interaction state over a Representation; it is not a Kernel layer.

The Kernel emits bounded Representation snapshots or updates stamped with the
Resource Revision, Buffer identity/incarnation/version, Representation
identity, Projection generation, and semantic coordinate-space identity they
describe. A Client maps device and presentation coordinates into a stamped
semantic location before submitting an Interaction Intent. Every submitted
intent receives one correlated terminal Interaction Outcome. Representation
Updates report state and do not substitute for that outcome.

Cursor, focus, selection, scroll, viewport, layout, hit testing, drawing, and
frame state remain Client-local by default. Moving state inward requires a
separately accepted shared, persistent, collaborative, or command-semantic
contract with explicit authority, identity, bounds, lifecycle, fencing,
privacy, failure behavior, and conformance evidence.

TUI, CLI, and Web are mandatory v0.16 Client Families. Each has its own Family
Core, public Family Contract, extension point and Module surface, and production
presentation or output path. Android and Native are future families and inherit
no support or qualification claim.

## Extensions

Easy extension is a product requirement. The language-neutral Extension
Adapter contract projects public SDK, Module, and Family contracts into a
language toolchain or execution environment. An Extension Engine Adapter is a
conditional specialization for one interpreter, VM, managed engine, or
equivalent engine instance. Neither owns editing semantics or grants authority.

Kernel, Client, Family, Module, and CCM contracts plus independently versioned
service schemas remain semantic authority. SDKs are language projections. Rust
implementation and admission proceeds first; Python and TypeScript use their
selected out-of-process Service Contract rows after the Rust-first and service-
reference gates. No SDK owns wire meaning, retry, Effect Certainty, or editing
semantics.

The v0.16 target includes the Extension Adapter and Engine Adapter contracts,
the Extension Adapter Kit, and a TestOnly deterministic synthetic model. It
selects no production extension engine or runtime. Local Quick Extension
manifests may be synthesized before source execution from explicit author
inputs; installed and published packages require explicit manifests. No
separate configuration/automation role or Neovim compatibility profile is
selected.

Official v0.16 Module bindings are:

| Module binding | Delivery gate | Admission state on selection |
| --- | --- | --- |
| Rust Server, TUI, CLI | `RequiredV0.16` | `PlannedMissing` |
| Python Server | `RequiredV0.16` | `PlannedMissing` |
| Python TUI, CLI | `TargetNonBlocking` | `PlannedMissing` |
| TypeScript Web | `RequiredV0.16` | `PlannedMissing` |

TypeScript Server/TUI/CLI and Rust/Python Web are excluded. Another language,
role, placement, or engine requires an explicit BindingScope decision and its
own DeliveryGate, AdmissionStatus, dependency review, and evidence. Neovim
compatibility may be evaluated for v0.17 only through another owner-scoped,
version-pinned profile; parity does not admit it implicitly.
