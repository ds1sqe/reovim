# Buffer, Transactions, History, and Residency

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## One Resource per Buffer

A **Buffer** is Reovim-owned transactional working state for exactly one
Resource. A Resource may be opened into several explicitly identified Buffers
for independent contexts, branches, or base observations. Each is a separate
working-state and History authority. Representations and Views intended to share
edits identify the same Buffer through public contracts. Cross-Resource
operations coordinate several Buffers above this boundary.

A Buffer owns:

- Resource Identity, current Resource Access Incarnation, and base Snapshot;
- a local version distinct from Resource Revision;
- accepted edit intent and coherent working-state reads;
- Undo/Redo History and conflict state;
- bounded partial-residency state;
- pending publication and reconciliation state; and
- the distinction between local acceptance and external effects.

Its descriptor bounds Transaction payload and owned bytes, Operation and
Precondition counts, pending overlay state, History entries/bytes, resident
source state, and concurrent or unresolved publication/creation state. Shape
limits are checked before traversing or allocating an untrusted request.

A Buffer is not a string, View, Resource Adapter, or full source copy.
Presentation cursor, selection, focus, viewport, and layout are Client-owned.
Encoding and newline policy belong to a Domain or Representation.

Promoting Client-local state to shared Domain or Kernel authority requires a
separately accepted contract for identity, authority, persistence,
synchronization, fencing, lifecycle, bounds, privacy, failure, and conformance.

## Transactions, Buffer State Lane, and History

A **Transaction** is an ordered atomic lowered edit against a known Buffer
incarnation and observed version. It carries Buffer-valid Operations and
Preconditions plus origin/correlation data. Domain, Client, extension, and
external callback policy is fully evaluated before the atomic acceptance
section.

The **Buffer State Lane** is the one logical linearization authority for
Transaction, History, compatibility, publication/reconcile, and lifecycle
installation. It is not necessarily a thread. Work outside the lane observes
immutable stamped state. Before entry, all fallible capacity and rollback state
is reserved. Inside the lane, the Kernel revalidates every identity, version,
and Precondition and performs only bounded synchronous non-suspending commit.
The lane is released before external work, callbacks, waits, Protocol/Resource
I/O, Event consumers, or Client work.

Local acceptance:

1. validates the complete Transaction and Preconditions against one coherent
   Buffer version;
2. rejects without state change or accepts atomically;
3. advances the Buffer version; and
4. records sufficient intent for coherent reads and History.

Undo and Redo are new ordered Buffer transitions, not mutation of old state.
The History descriptor declares ordering and current position, retained entry
and owned-byte maxima, eligibility, branch/truncation and Redo invalidation,
checkpoint identity, and reconstruction inputs that cannot be evicted.

After any allowed eviction, compaction, or checkpoint, reconstruction at the
same Buffer identity/incarnation/version produces equivalent Units, accepted
intent, Preconditions, History position, and pending Intents. If this proof
cannot be retained within bounds, the operation rejects before state change.

## Incarnation and stale-work fencing

Every Buffer has an incarnation distinct from Resource Identity and local
version. Reopen, replacement, or reset creates a new incarnation. Asynchronous
observation, lowering, Projection work, publication, creation, and extension
requests carry the exact Buffer and Resource Access generations observed.

Old work cannot install derived state, accept a Transaction, or begin a new
external operation after cancellation, replacement, close, or an incompatible
transition. Caller-visible work resolves once or retains bounded Reconcile
Status. No-waiter speculation may be discarded only with declared counters and
Event accounting.

Fencing cannot undo handed-off effects. An applied or unknown result for a
closed Buffer cannot mutate it but remains visible to the named reconcile owner.
Close reconciles within bounds, transfers the unresolved Intent to an explicitly
live owner, or returns typed incomplete/abandon-required status.

Retiring a Resource Adapter retires its Access Incarnation before new work.
Late reads, Snapshots, change deliveries, publication/creation completions, and
reconcile responses cannot install through the replacement even if Resource
Identity and Revision happen to match.

## Bounded partial residency

Partial residency is normal:

- opening does not load the full Resource;
- bounded operations access only needed native regions;
- nonresident data, absent Units, stale observations, invalid ranges,
  cancellation, and failed access remain distinct;
- eviction of clean source material does not discard accepted edits, History,
  or unresolved external effects; and
- resident payload and metadata obey declared bounds with visible pressure.

The residency descriptor declares native range/Unit accounting, payload and
metadata bytes, entry/region/pin/in-flight/waiter counts, per-Buffer and
aggregate maxima, accounting ownership for sharing, checked Extent/cost
arithmetic, zero limits, admission order, and typed pressure.

The contract requires stable provenance so state from the wrong Resource,
Revision, Access Incarnation, or Buffer version cannot become current. It does
not select pages, span keys, cache tokens, coalescing, pin types, replacement
policy, ownership carriers, or synchronous versus asynchronous hydration.
