# Integrity, Effects, and Fencing

[Mission-Critical Quality index](README.md)

## Purpose

Reovim pursues mission-critical editing quality. That direction means a
smaller surface with demonstrated integrity is preferable to a feature-rich
surface with unresolved failure modes. “Mission-critical” is an evidence and
engineering discipline; it is not a blanket claim that every build is safe for
every environment or certified under an unnamed standard.

This contract governs admission of Kernel, Client, adapter, SDK, extension,
and migration features. It complements the functional contracts rather than
creating a second implementation path.

## Priority order

When requirements compete, review follows this order:

1. source fidelity and prevention of silent corruption or data loss;
2. atomic state authority and deterministic conflict behavior;
3. explicit Effect Certainty, recovery, cancellation, and shutdown;
4. bounded resources, isolation, backpressure, and pressure visibility;
5. actionable diagnostics, provenance, and reproducible evidence;
6. measured reaction latency, memory, I/O, and extension cost; and
7. feature breadth.

Performance remains required. It cannot be purchased by weakening fidelity,
installing stale results, bypassing Transactions, hiding failed samples,
silently dropping evidence, or moving unbounded work into a Client or adapter.

## No silent state ambiguity

An editing operation has one authoritative state transition path. State is not
silently copied into a Client, Projection cache, extension, Protocol Adapter,
or Resource Adapter as a second authority.

Local Buffer acceptance and external mutation are separate transitions. A
local Transaction is accepted atomically at one resulting Buffer version or is
not accepted with a typed cause.

Every boundary that can lose knowledge of a mutation reports two orthogonal
axes:

- **Effect Certainty:** applied, definitely not applied, or outcome unknown;
- **Outcome Cause:** success, conflict, stale state, unsupported behavior,
  denial, cancellation, pressure, closure, timeout, or failure as applicable.

Applied means the declared target effect is confirmed. Definitely not applied
means the target is proved unchanged. Outcome unknown means an effect may have
occurred and requires reconciliation or evidenced idempotency before retry. A
cause such as cancellation, timeout, or failure never erases certainty.

Each mutation contract publishes its allowed Effect Certainty × Outcome Cause
matrix by admission/handoff cut. Pre-admission conflict, stale, unsupported,
denied, pressure, or closed causes are definitely not applied; confirmed
success is applied; outcome unknown is allowed only after effect handoff; and
cancellation, timeout, or failure derives certainty from the cut and proof.
Conformance rejects illegal pairs. SDKs preserve unknown future causes without
inventing certainty.

Timeout, disconnect, or cancellation does not prove that an already-handed-off
external mutation was not applied. Error simplification in an SDK cannot erase
this distinction.

## Stale-work fencing

Every asynchronous operation carries the exact identities and generations its
boundary requires; “as applicable” is not an implementation choice. Work cannot
install local or derived state after its authority was cancelled, closed,
replaced, reclaimed, or made stale by a newer incompatible generation. Purely
speculative derived work without a waiter may be discarded with visible
accounting; a caller-visible mutation resolves exactly once or retains bounded
public reconciliation status.

Each owning contract defines a deterministic generation-compatibility
predicate. The predicate names which identity, capability, authority, source,
mapping, or configuration changes invalidate admitted work and which changes,
if any, permit reuse. Numeric ordering or “newer” alone does not prove
compatibility. When compatibility cannot be proved, the work is stale and
follows the boundary's rejection or reconciliation path.

Cancellation before mutation admission is definitely not applied. After
admission but before external handoff, the owning contract states whether abort
can still prove definitely-not-applied. After handoff, cancellation stops local
waiting and installation authority but cannot undo a target effect; the result
is applied or outcome unknown and remains visible to the reconciliation owner.
Retry creates or reuses identity only by the owning contract.

### Required fence matrix

| Boundary | Mandatory authority and observed state | Terminal acceptance check |
| --- | --- | --- |
| Resource read | Resource identity, Resource Access Incarnation, adapter contract, Revision, request/cancellation identity | returned Snapshot belongs to that exact access incarnation/Revision or is typed stale/unavailable |
| Local Transaction | Buffer identity/incarnation, observed Buffer version, Preconditions | atomic commit only against the current compatible incarnation/version |
| Projection work | Buffer identity/incarnation/version, Representation identity, Projection generation | install only into the exact current generation |
| Interaction Intent | Contract Session/authority generation, Buffer identity/incarnation/version, Resource Revision, Representation and Projection generation | one correlated terminal Outcome; accepted version is explicit |
| Representation Update | Contract Session, Buffer identity/incarnation/version, Representation identity, predecessor/result Projection generation | Client applies only to the exact predecessor or requests resynchronization |
| Publication Intent | Resource identity/access incarnation/base Revision, adapter contract, Buffer identity/incarnation and selected version/transitions, access-bound recovery witness | applied proof or bounded definitely-not-applied/outcome-unknown reconcile state; retired access cannot install locally |
| Resource Creation Intent | exact source Resource/access/Snapshot or Buffer incarnation/version, destination family/namespace and authority generation, creation request, bounds, and cancellation | exact destination proof or bounded definitely-not-applied/outcome-unknown reconcile state; neither source nor stale destination authority is rebound |
| Client computation/render | Contract Session, Client/Runtime/Family/Module/View incarnations and generations, source Update predecessor/result, layout/render generation, Rendering Capability, Presentation Adapter/surface/device incarnation | install or hand off only into the exact live Client/Family/Module/render/presentation generations; otherwise discard visibly or resynchronize |
| CCM work | Synchronization Scope/schema, membership and authority generation, source state revision, Client/Module incarnations, request/cancellation | apply only to the exact predecessor and membership; gap, leave, revoke, or replacement requires typed resynchronization/terminal outcome |
| Protocol/IPC work | Contract and Protocol Session, Protocol Adapter/connection/carrier incarnations, schema/codec version, request/message/correlation and cancellation identities, applicable mutation witness | deliver or install only through the exact live Session/connection generations; replacement/reconnect gaps resynchronize, and handed-off mutation certainty remains reconciled |
| Extension request | Contract Session/request identity/authority generation plus every target stamp used | exactly one terminal result or retained public reconcile status |
| Extension Adapter/Engine work | Adapter/Engine/Extension Host Supervisor/Module identities and generations, binding/placement, grant, request/callback, code/state generation, cancellation | callback/result installs only through the exact live Session, Adapter, Engine, Extension Host, and Module generations; stop/revoke/replacement retires every older completion before unload/reap |
| Host work | capability instance/generation, request identity, deadline/cancellation authority | no local install after close; external Effect Certainty remains observable |

Conformance generates stale, duplicate, reorder, cancellation, close, and
replacement traces from this matrix rather than relying on happy-path examples.
