# Domain Validation and Representation Lowering

[Domains, Representations, and Projections](../01-Domains-Representations-and-Projections.md)

## Pipeline and ownership

Kernel Runtime owns the semantic policy operation. It retains the complete
[Semantic Intent](01-Semantic-Descriptors-Stamps-and-Intent.md) from admission
until a terminal semantic result and gives concrete policy only immutable
invocation-scoped borrows. Policy cannot retain the borrow, consume the Intent,
or obtain a private Runtime, Buffer, Projection, or execution handle.

The edit-policy pipeline has two explicit candidate cuts:

~~~text
retained Semantic Intent
  -> Domain validation candidate
  -> Runtime-validated receipt
  -> Representation lowering candidate
  -> Runtime-validated Buffer Transaction Plan
~~~

Provider output is never acceptance. Only Kernel Runtime validates a candidate
against retained authority; only the later private Buffer Transition port may
attempt state acceptance.

## Execution contract

The containing Runtime operation has non-blocking admission, deferred terminal
completion, one local-only result, non-reentrant execution, and a phase that may
invoke selected user/Module policy. Kernel Runtime admits it only through its
private `ExecutionFacility`; the policy port neither receives nor implements
that facility.

Each selected policy call declares work-unit, temporary-byte, result-byte, and
concurrent-call bounds, plus one charge owner, bounded-to-completion or
cooperative progress, callback context/affinity, permitted outward calls,
cancellation/deadline observation, and contained-unwind behavior. The first
visible-edit callback is bounded-to-completion and retains no input after its
return.

No policy callback runs under Buffer State Lane, Client State Lane, adapter,
Protocol, Event-consumer, or presentation authority. Policy performs no I/O and
emits no Event synchronously through the retained request.

## Domain validation cut

The Domain port exposes descriptor inspection and one semantic validation
callback. Its borrowed request contains the exact retained Intent, observed
cancellation/deadline state, and no mutable authority.

The provider returns either:

- a validation candidate echoing the Domain generation, unique Intent and
  request identities, carrier selection, and checked shape; or
- a typed semantic non-result.

Kernel Runtime compares the provider's current descriptor and candidate stamp
with its retained complete stamp. Only an exact match mints a **Domain
Validation Receipt**. The compact echo avoids copying the complete Resource,
Buffer, History, Projection, and plan stamp through the callback. Policy cannot
construct authority merely by returning “valid.” A wrong descriptor, request,
policy generation, or carrier stamp is a provider contract violation, not
invalid user input.

## Representation lowering cut

The Representation port exposes descriptor inspection and one complete
lowering callback. Its borrowed request contains:

- the same retained Semantic Intent;
- the Runtime-minted Domain Validation Receipt;
- the selected Representation and coordinate/correspondence contract; and
- the exact `BufferPlanDescriptor` governing output construction.

Concrete Representation policy validates coordinate/value meaning and reverse
correspondence, then returns either one complete `BufferTransactionPlan`
candidate or a typed no-plan result. Partial Operations, caller-visible builder
state, and mutation closures cannot cross this boundary.

Before forwarding the candidate, Kernel Runtime verifies exact equality of:

- Domain, Representation, Projection predecessor, and coordinate stamps;
- semantic request, caller authority, cancellation, correlation, deadline,
  Transaction, and History-transition identities;
- Resource Snapshot, Buffer incarnation/version, and History target state; and
- Buffer-plan contract plus origin, Operation, and Precondition carrier
  selections.

The selected `BufferPlanDescriptor` also revalidates the returned complete
plan's owner, carriers, shapes, and complete-byte bound through its borrowed
Buffer-owned operation. Runtime also rejects a plan whose actual owned bytes
exceed the result bytes reserved before callback entry. Mismatch is a provider
contract violation. It does not silently rewrite the plan, re-run policy
against current state, or enter Buffer State Lane. A rejected plan candidate
remains move-owned and payload-redacted until Runtime deliberately releases it.

## Origin and opaque meaning

The Representation provider writes the self-contained origin carrier under its
selected schema. That schema includes the applicable Domain, Representation,
Projection, coordinate-space, caller-authority, and request stamps. Kernel
Interface validates the selected carrier and outer plan equality but does not
decode or fabricate origin meaning.

The provider likewise owns concrete Operation and Precondition encoding. The
Buffer contract validates their selected families and bounds; Buffer Core later
evaluates them at its linearization cut. There is no intermediate general IR.

## Failure, cancellation, and lifecycle

Expected semantic results distinguish at least unsupported capability/schema,
invalid intent, no correspondence, ambiguous correspondence, stale state,
read-only policy, pressure, cancellation, deadline, caller timeout,
replacement, closing, closed, and bounded policy failure. These are distinct
from structural request errors and provider contract violations.

Runtime checks cancellation/deadline before each policy call and checks every
generation again when finishing its candidate. Both finish cuts consume an
explicit current-or-fenced installation outcome. Cancellation, caller timeout,
deadline, replacement, Closing, or Closed maps to a distinct semantic failure,
fences installation, and does not prove execution stopped. A late plan remains
owned by the Runtime outcome under declared terminal bounds and cannot install
into a newer generation.

Finish precedence is deterministic. Provider descriptor and candidate-shape
violations remain contract violations. A provider semantic non-result remains
that terminal non-result. Only an otherwise valid receipt or plan candidate is
converted to the observed installation-fence failure.

A selected catch boundary observes every policy unwind. Caught panic produces a
typed failed/fenced terminal result, preserves Runtime's Intent owner, and
cannot enter Buffer State Lane. No unwind crosses a Rust SDK, native/FFI, or
other non-unwind-safe boundary. Process-isolated providers require their later
Extension Host and service contracts; this port claims no process containment.

Closing stops new policy admission, advances policy and Projection fences,
requests cancellation, resolves or exposes all admitted work, then releases
policy code/state only after execution is stopped or its process is proved
reaped. Unresolved work yields `IncompleteShutdown`, never silent detach.

## Projection computation boundary

Projection computation remains a sibling policy operation over immutable
stamped Buffer inputs. It returns bounded semantic snapshot/delta,
correspondence, and source-dependency candidates that Kernel Core validates and
installs under Projection generation fencing.

This edit-lowering contract does not select or claim that sibling mechanism.
It preserves the identities, bounds, and ownership needed by the later
Representation Update slice rather than inventing a placeholder snapshot API.

## Public conformance

Before Runtime or Buffer mechanism source, an external provider and consumer
must compile using only public contracts and prove:

- valid Domain receipt followed by complete plan lowering;
- non-Text opaque carrier compatibility through the same port shape;
- wrong policy, Projection, coordinate, receipt, target, and plan stamps;
- invalid, read-only, no/ambiguous mapping, pressure, cancellation, deadline,
  replacement, closing, and provider-failure results;
- exact retained request ownership across normal failure and caught panic; and
- payload-redacted Debug for every public owning value.

That fixture proves only the public contract. Product latency, policy quality,
and release support require the later composed real Scenario, trace, Event,
fault, and exact-artifact evidence.
