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

### Public Transaction plan boundary

Kernel Interface owns the portable **Transaction Plan** values crossing from
Domain or Representation lowering into Kernel Runtime. Kernel Core owns the
private unstable Buffer Transition port and its state mechanism. A Transaction
Plan is immutable data for that later port; constructing or validating it does
not prove currentness, accept state, advance Buffer or History, or publish an
Outcome.

One **Buffer Plan Descriptor** selects:

- its own contract identity and compatible version;
- the exact lifecycle generation authorized to receive plans;
- independent versioned carrier contracts for origin, ordered Operations, and
  Preconditions; and
- Count bounds for Operations and Preconditions plus Byte bounds for each
  carrier and total owned plan state. All six bounds name one explicit charge
  owner; construction rejects the first mismatched owner as a typed error.

The three carrier type identities are distinct. Their portable Rust projection
is immutable, move-only, transferable with shared borrowing, and value-lifetime
owned. Debug exposes role, type/version stamp, item count, and charge but never
carried bytes. Carrier mismatch rejects without parsing and returns the
unchanged owned allocation.

The origin carrier is one self-contained schema-qualified provenance record.
Its selected semantic schema includes the exact applicable Domain,
Representation, Projection, coordinate-space, request, and authority stamps.
Kernel Interface validates and retains its carrier contract but does not decode
Domain meaning. Operations and Preconditions likewise remain Buffer-family
selected opaque values; there is no universal opcode or general Editing IR.

Preparation validates carrier role, type identity, selected schema version,
item count, byte count, zero rules, checked arithmetic, and per-plan hard bounds
before allocation, traversal, parsing, lookup, or policy/Core work. Binding
repeats those checks against the current descriptor and exact allocation
length. A plan has exactly one nonempty origin record and at least one
Operation. Preconditions are optional: zero items require zero bytes, while a
nonzero count requires nonzero bytes.

Binding returns one move-only **Buffer Plan Binding Outcome** so acceptance and
rejection remain explicit ownership transfers. Its Accepted variant owns the
complete Transaction Plan. Its Rejected variant owns the exact rejection
cause, stamp, target, and every unchanged carrier. Both variants are
transferable, immutable, and payload-redacted under Debug.

One **Transaction Plan Stamp** names the descriptor-owner generation,
Transaction identity, History-transition identity, cancellation authority,
root correlation, and optional separately governed deadline. The target state
names one exact Resource Snapshot, Buffer identity/incarnation and local
version, and History lifecycle and position. Resource Revision and Buffer
version remain distinct.

Cancellation or deadline observation before Buffer State Lane entry can reject
the plan. The atomic transition itself is local, inline, bounded,
non-suspending, non-reentrant, and callback-free. Once its linearization section
begins, it completes with acceptance or typed non-acceptance rather than
polling an outside cancellation source.

Structural validation cannot determine whether a previously valid target is
still current or whether an encoded Precondition holds. Kernel Runtime and the
private Core port revalidate the exact target, carrier selections, Operations,
and Preconditions at the declared cuts. Failure returns or retains the complete
plan under its ownership contract and never silently retargets it.

The Transaction Plan is not a public Buffer mutation service. Clients and
extensions submit semantic Intents; selected Domain/Representation policy
produces plans. A general Editing IR or pass manager remains unavailable until
the separately governed multi-consumer and measurement gate admits one.

### Buffer State Lane and acceptance

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
