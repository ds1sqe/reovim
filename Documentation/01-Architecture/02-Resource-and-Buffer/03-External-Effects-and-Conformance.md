# External Effects and Conformance

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Local branches and create-another

A Buffer over Read-Only Access may be explicitly locally read-only or may accept
local Transactions as a derived working branch. Same-Resource publication
remains unsupported.

Creating a destination uses one immutable **Resource Creation Intent**:

- it binds the complete source stamp: Resource Identity, Access Incarnation,
  base Revision or exact Snapshot, and Buffer identity/incarnation/version when
  Buffer state is selected;
- it names an independently authorized destination family/namespace, requested
  identity policy, fidelity, bounds, cancellation, conflict, and recovery
  witness;
- it streams bounded Resource-native Units when needed and never requires a
  whole-state in-memory clone;
- it does not mutate or rebind the source Resource or Buffer;
- asynchronous creation retains a charged selected-state lease or complete
  reconstruction witness so later Buffer edits cannot change the payload; and
- it returns Effect Certainty, Outcome Cause, validated creation proof when
  applied, or bounded Reconcile Status when outcome is unknown.

Each destination family declares creation atomicity and visibility. Under an
atomic capability, the new Resource becomes visible only with the complete
selected initial state and validated initial Revision/Snapshot. A family
exposing reserved identity, staging, or partial creation uses a separately
named non-atomic contract defining every visible state, cleanup, certainty,
proof, and recovery outcome.

Opening a new Buffer over the created Resource is a later explicit operation,
never a creation side effect. Cross-family transformation belongs to an
accepted Domain/Representation or orchestration contract; create-another does
not invent byte-copy semantics.

## Add-Only and Undo

**Add-Only** is Resource-native and broader than byte append. It adds a record,
key, graph fact, sample, or another family-defined value under a declared
monotonicity relation and cannot remove or replace already accepted Resource
state.

The descriptor defines accepted additions, ordering, identity, duplicate and
concurrent-add behavior, atomicity/handoff, proof, compensation, and
reconciliation.

- Before proved external application, Undo is a local Buffer transition and may
  remove an addition from a later Publication Intent.
- After proved application, local Undo may change Buffer state but cannot erase
  the Resource effect or publish a removal.
- Outcome-unknown work retains reconciliation ownership.
- A compensating addition is a new external effect, never rollback.

## Local acceptance and publication

Accepting a Transaction is not Resource publication. Local acceptance performs
no hidden external I/O and does not imply publication support.

Publication submits one immutable **Publication Intent** binding Resource
Identity, Resource Access Incarnation, base Revision, adapter contract, Buffer
identity/incarnation, exact selected Buffer version/transitions, and a family
recovery identity or witness. Later local edits remain pending and are never
silently included.

Every publishable family owns versioned **Publication Proof** and **Reconcile
Status** schemas with public validators and Bound Descriptors. The proof
enumerates applied variants, required resulting Identity/Revision/Snapshot or
witness fields, compatibility, and size limits. Reconcile Status binds exact
Resource/Access and request identity, selected Buffer state, Effect Certainty,
Outcome Cause, proof, retention/expiry, duplicate behavior, conflicting later
publication rules, and permitted next operation.

A family advertising atomic publication applies its complete declared set or
none under the claimed failure model. A known partial effect violates that
capability and cannot be hidden as `OutcomeUnknown`. A non-atomic family uses a
separate contract.

Every terminal result preserves two dimensions:

- **Effect Certainty:** `Applied`, `DefinitelyNotApplied`, or `OutcomeUnknown`;
- **Outcome Cause:** success, conflict, stale, unsupported, denial,
  cancellation, pressure, closure, timeout, or failure as applicable.

Each family publishes the allowed matrix at every admission/handoff cut.
Pre-admission stale, conflict, unsupported, denied, pressure, and closed results
are definitely not applied. Confirmed success is applied. Outcome unknown is
permitted only after effect handoff. Cancellation, timeout, and failure derive
certainty from the cut and available proof.

An applied result includes a validated proof. `DefinitelyNotApplied` is returned
only when unchanged state is proved. `OutcomeUnknown` returns the complete
Reconcile Status. Conflict preserves local intent for explicit reject, rebase,
or replacement.

Cancellation before admission is definitely not applied. After admission but
before handoff, the family declares whether abort can still prove that result.
After handoff, cancellation stops waiting and fences local installation but
cannot cancel the external effect; the result remains applied or unknown and
owned by reconciliation.

Recovery prevents accidental duplicate application. The family defines a
bounded reconcile operation over the Access-incarnation recovery witness,
outcome retention/expiry, duplicate identity, and blocking rules for unresolved
unknown work. Abandon, when supported, records that certainty remains unknown.
Retention expiry never fabricates certainty.

## Fidelity and proof obligations

The Buffer retains source Units faithfully. A high-level interpretation cannot
erase Units it does not express by round-tripping. Preservation and rejection
belong to the semantic contracts in
[Domain, Representation, Projection, and Client-Owned
View](../03-Semantics-and-View/01-Domains-Representations-and-Projections.md).

The v0.16 production vertical proof opens one partially resident byte-addressed
Resource, attaches faithful byte/hex and Text Representations to one Buffer,
edits from both directions, preserves invalid byte sequences, exercises Undo,
Redo, stale conflict, fencing, cancellation, publication certainty and
reconciliation, reopens, and measures I/O, latency, and memory. No View or
presentation type enters Buffer or Kernel state.

Deterministic public models separately prove Read-Only plus create-another,
volatile Snapshot freshness/change gaps, and Add-Only external irreversibility.
These model obligations do not silently add production adapters or support
claims.

Every publishable production adapter runs the shared family conformance suite
and target-specific atomicity, crash, durability, and I/O cases it claims.
Release evidence maps the exact artifact to the Supported and Qualified
Environment Matrix. The initial qualification target is
`x86_64-unknown-linux-gnu`; target selection does not prove qualification.

See [Layer and Dependency Model](../01-Layer/01-Roles-Descriptors-and-Dependencies.md).
