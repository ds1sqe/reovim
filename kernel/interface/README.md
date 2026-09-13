# Kernel Interface

Public Resource access and provider contracts, independent of the
[Runtime implementation](../runtime/README.md).

- `Connection`, `ResourceId` and `Message`: granted Resource requests.
- `ResourceProvider` and `ModelProvider`: supplied implementations.
- `Module`, `Factory` and `DriverOps`: construction and driven lifecycle.
- `abi`: C-compatible caller tables used by language adapters.

```rust
// The module's API package defines parse_contract; the common SDK does not.
let input = Message::reference(parse_contract, source);
let parsed = kernel.write(parse_resource, &input).await?;
```

Module-specific schemas and helpers belong with their module. Adding a service
must not add fields to the common Interface. Names and concrete storage remain
private to the implementation.

This crate uses `no_std + alloc` and has no dependency on Runtime or Clients.

## Backed opening

`buffer::open` writes `kernel.buffer.open` v1 to the selected Model. Data uses
the Model's initialization encoding; one READ reference names the source. The
current profile requires exactly one READ initialization slot and no variable
reference policy. The Model initializes from the selected source state;
transformations belong to subsequent edits.

Prepare any source I/O through its backend before this call. Runtime acquires
an inert independent read view that declares `resource.base` v1 alongside the
Model's data surfaces. That view owns one coherent immutable data/base version;
the base's meaning stays with the backend. Runtime retains the actual source
separately and translates the view into the Model's receiving namespace.

The reply is a Buffer descriptor: revision:u64 little endian and Buffer,
History, source references. Decode it with `buffer::descriptor`. Buffer/History
receive normal owner access; the source slot conveys identity only. Describing
a backed Buffer likewise grants no extra source operations to observers.

Buffer/History and association install together after initialization succeeds.
Failed or cancelled initialization exposes no Buffer. Ordinary Model creation
remains unbound even when it reads a source. RejectModified accepts a backed
Buffer at its original source state, including Undo to that state; later edits
remain unpublished. Closing one Buffer releases its source/base leases, not
the shared source or independently retained working state.

This is the initial association mechanism under the
[Buffer contract](../../Documentation/Design/02-buffer.md). It does not implement
file saving, Save As, rebinding, or filesystem publication guarantees.

## Backend publication evidence

`publication::backend` supplies the first coordinator/backend protocol:
`resource.publish` consumes immutable selected-data/base READ slots under target
WRITE authority and returns an independently retained evidence Resource plus
an opaque release key. Evidence reads classify the effect; reconciliation
progresses only that original effect. Explicit release at the original target
rejects unresolved or captured evidence. Required ownership does not live in
optional Error.id diagnostics.

Applied evidence supplies an independent view of its exact committed base,
not a refreshed source. Retained Model snapshots now also supply independent
data-only read views, so a backend can read selected state after the original
snapshot and Buffer close. Native details and concrete Model data stay behind
their providers.

The public test witness publishes a non-byte logical value conditionally and
exercises old-base rejection, lost reply recovery, unresolved evidence and
independent committed-base lifetime. The Kernel save profile below consumes
that protocol. Actual file saving still requires a qualified writable backend.

## Conditional save and retained Work

`publication::save` writes `kernel.buffer.save` v1 to an exact BufferState.
Admission captures immutable Model state, the actual source and original base
before queueing. It separately validates source WRITE authority and the backend
publication/release contracts. Formatting parameters remain backend-defined.
Unbound, stale, denied and unsupported requests reject before backend effects.

The report carries the selected observation, target identity, effect status
and original Work/recovery locator. Buffer and target slots grant no incidental
operations; only Work supplies reconciliation/release access. Private readers
and target authority stay in the coordinator's scope across caller reconnect.

Only proved application for the same association and original base advances
that base and marks the selected History node published. Later edits remain
dirty. Known application stays Applied if base acquisition/adoption fails;
the Work remains pending and owns that unfinished adoption. Predictable reply
storage is reserved before handoff.

`publication::reconcile` awaits progress of that same save and returns its
refined report. Concurrent callers join it. Failed reconciliation preserves
the earlier evidence and reports the control failure separately. Earlier
Message copies never change. Ordinary non-publication Work does not gain a
reconciliation implementation from this profile.

Reconciliation cannot erase a known partial effect or atomicity violation.
Contradictory backend claims reject as Provider failure while prior evidence
remains retained. A non-atomic partial operation may complete its remaining
original work and then report Applied; a violated atomic guarantee cannot be
retroactively satisfied by repair.

Close Work after adopting its proof. Unresolved effects reject release;
settled effects may first return Busy while Runtime releases backend evidence.
Retained release requests prevent uncertain cleanup from becoming a new call.
Other captures can keep release Busy. A settled proof may outlive Buffer close;
the Buffer retains its proved base independently from backend evidence.

Cancellation before handoff prevents publication; cancellation after handoff
does not erase the actual result. The current logical backend exercises this
chain. File backing remains read-only, and attempted save preserves dirty state
and file contents. Save As, rebinding, rename, actual partial effects/transfer
and filesystem concurrency/durability qualification remain unfinished.
