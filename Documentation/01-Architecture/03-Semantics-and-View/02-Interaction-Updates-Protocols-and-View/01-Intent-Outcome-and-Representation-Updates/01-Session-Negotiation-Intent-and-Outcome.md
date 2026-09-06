# Session Negotiation, Intent, and Outcome

[Interaction Intent, Outcome, and Representation Update](../01-Intent-Outcome-and-Representation-Updates.md)

## Scope and authority

Kernel Interface owns the transport-neutral Interaction Session contract.
Kernel Runtime provides a direct endpoint. Client Runtime, a public service
projection, or an External Protocol Binding consumes it without access to
private Kernel state.

The Rust projection remains `no_std + alloc` and Experimental in v0.16. Rust
trait layout, scalar identity representation, and in-process ownership are not
the language-neutral or wire contract.

## Session identities and scope

Client, Contract Session, Interaction request, cancellation, root correlation,
deadline, and reconciliation identities are distinct types. A Session stamp
binds its identity, selected contract and compatible range, lifecycle
incarnation/generation, exact Client identity and lifecycle, selected Client
role, and authority generation.

One Session Resource Scope binds the Resource Identity, Resource Access
incarnation, and Buffer incarnation it may address. It grants neither concrete
adapter access nor mutation authority. A later multi-Resource contract requires
its own atomicity and bound rules rather than widening this scope implicitly.

An Interaction request maps explicitly to the separately scoped semantic
request, Semantic Intent, Buffer cancellation, Buffer correlation, deadline,
Transaction, and History-transition identities. The relation is a typed
binding. Equal scalar values, timestamps, or field position never create it.

## Session descriptor and negotiation

The immutable Session descriptor declares:

- exact Session, Client/role, authority, and Resource Scope stamps;
- mandatory Intent admission, terminal Outcome delivery, and close support;
- selected cancellation, reconciliation, Update, delta, and resynchronization
  capabilities;
- invocation, completion, delivery, effect, callback, and reentrancy classes;
- pending/retained work, payload, stream, credit, and shutdown bounds;
- terminal and Update retention, fairness, and slow-Client isolation; and
- the exact Representation Update carrier contract.

The first direct Session requires all capabilities used by the visible-edit
oracle. An unsupported optional capability returns a typed result; a method's
presence does not silently enable it.

Negotiation validates authority before Resource or attachment lookup. It checks
contract compatibility, Client and role, Resource Scope, capabilities, bounds,
and the complete Session generation. Explicit selection occurs in Assembly;
this port never probes or chooses among competing providers.

The Session remains provisional until the required Representation attachment
and initial state satisfy the
[Update contract](02-Update-Delivery-Gaps-and-Resynchronization.md).
Construction or successful transport connection alone does not publish Ready.

## Bounds and reservation

One Session Bound Descriptor uses one explicit charge owner and independently
bounds:

- pending Intent count and aggregate retained Intent bytes;
- retained terminal Outcome count and bytes;
- per-Update item count, nesting depth, and payload bytes;
- retained Update count and aggregate bytes;
- live data credit and reserved control delivery;
- outstanding resynchronization count, abstract work, and result bytes; and
- bounded close work.

The ten stateful calls on one direct Rust Session endpoint require exclusive
mutable `InteractionSessionPort` ownership and are therefore serialized. The
Rust projection expresses this as `&mut self`; immutable descriptor inspection
remains borrow-only. Serialization is not a concurrent-call capacity and adds
no per-Intent call charge. A Protocol Binding may accept concurrent ingress
only under its own bounds and must serialize dispatch before entering this
port. Deferred Intent, policy, and resynchronization work retains its
separately declared concurrency after the admitting call returns.

Checked arithmetic and unit/zero validation occur before allocation or
ownership transfer. Intent admission reserves both pending-work capacity and
one terminal Outcome slot. Terminal pressure therefore rejects admission; it
never accepts a request whose terminal disposition can only be dropped.
The terminal byte reservation equals the selected per-Outcome maximum.
Revalidation rejects a smaller same-owner reservation prepared under another
descriptor; merely fitting below the selected maximum is insufficient.

## Interaction Intent ownership

One Interaction Intent move-owns the exact existing Semantic Intent. Its outer
stamp adds only Session, attachment, Interaction request, cancellation,
correlation, deadline, and explicit cross-scope mapping. It does not duplicate
or reinterpret the semantic payload.

Construction validates in this order:

1. Session, Client, role, authority, and attachment generations;
2. Resource, Access, Buffer, Representation, Projection, and coordinate scope;
3. explicit mapping to semantic and Buffer-plan identities;
4. semantic carrier and structural bounds already proved by its descriptor;
5. Session pending count/bytes and terminal reservation; and
6. cancellation and deadline state.

No lookup, traversal, policy call, Buffer work, or allocation occurs before its
applicable prior check. A malformed outer binding returns the unchanged
Semantic Intent owner.

Submission is a bounded non-waiting ownership cut. Definite non-admission
returns the unchanged complete Interaction Intent and one immediate terminal
non-acceptance. Admission transfers the owner to Kernel Runtime until policy,
Buffer disposition, and terminal Outcome ownership resolve. A caught policy
panic, caller timeout, fence, or failed delivery cannot consume or retarget it.

## Interaction Outcome

One admitted Intent produces exactly one bounded terminal Interaction Outcome.
The Outcome repeats the exact Session, attachment, request, Intent, and root
correlation identities. Its state is exactly one of:

- **Accepted:** `Applied`, accepted Transaction and History transition, and the
  resulting exact `BufferStateStamp`;
- **Not accepted:** `DefinitelyNotApplied` and one typed cause; or
- **Reconciliation required:** `OutcomeUnknown`, one typed cause, and one
  bounded reconciliation identity.

Construction rejects an accepted result without matching Transaction,
History-transition, Buffer incarnation, source Snapshot, and monotonic result
state. It also rejects a non-acceptance with accepted state or an unknown result
without reconciliation. No optional-field combination can express an
incoherent certainty.

Expected causes distinguish denial, unsupported/incompatible contract,
invalid Intent, stale state, read-only policy, no/ambiguous correspondence,
conflict, pressure, cancellation, deadline, caller timeout, replacement,
Closing, Closed, policy failure, provider violation, and invariant failure.
The exact mechanism cut determines whether cancellation, timeout, or failure is
definitely not applied or requires reconciliation.

Outcome and Update delivery may occur in either order. An accepted Outcome
does not advance Client Representation state; an Update does not prove local
acceptance.

## Retrieval, cancellation, and reconciliation

Non-waiting Outcome retrieval returns the one terminal value or typed pending,
unknown request, already delivered, retention Gap, replaced, Closing, or Closed
state. A retention Gap carries an exact reconciliation identity and never
converts absence into success.

Cancellation requests identify one exact admitted request and report already
terminal, newly requested, already requested, denied, unknown, stale, Closing,
or Closed. Cancellation cause and execution terminality remain independent.

Reconciliation consumes the exact Session/request/reconcile binding. It may
recover a retained Outcome or report pending, unresolved, expired, denied,
unknown, stale, Closing, or Closed. Recovering the retained value is its first
delivery, not a second Outcome. Resolving an `OutcomeUnknown` returns separate
reconcile status and never rewrites the original terminal record.

Timestamps, retry, transport acknowledgement, and Event presence do not infer
Outcome state or causality.

## Operation shape and public proof

Negotiation, cancellation request, non-waiting retrieval, and reconciliation
are bounded local calls with no user callback. Intent submission is a
non-blocking admission for deferred local work whose later policy phase may
invoke selected code. Every operation is non-reentrant.

All stateful Session operations share one serialized mutable owner. Assembly
does not publish multiple mutable endpoint owners for one Session generation.

Public conformance covers authority-before-lookup, incompatible negotiation,
wrong scope/mapping, duplicate identity, serialized mutable-owner dispatch,
every count/byte overflow and pressure cut, cancellation/deadline races, all
Outcome certainty shapes, retained-owner behavior across panic/fence,
reconciliation, replacement, Closing, and Closed. Payload-bearing Debug output
remains redacted.
