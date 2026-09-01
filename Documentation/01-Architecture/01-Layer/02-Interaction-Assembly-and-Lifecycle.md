# Interaction, Assembly, and Lifecycle

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Client interaction seam

Kernel Interface owns three transport-neutral contract families:

- **Interaction Intent** carries a bounded semantic request against one Contract
  Session and the exact Resource, Buffer, Representation, Projection, and
  semantic-coordinate state observed.
- **Interaction Outcome** is the one bounded correlated terminal result and
  records acceptance, Outcome Cause, Effect Certainty where applicable, and the
  resulting Buffer version.
- **Representation Update** carries a bounded semantic snapshot, delta,
  invalidation, Representation Gap, or resynchronization result stamped with
  exact source and Projection state.

Contract identity and compatibility are negotiated independently of product or
package version. The Contract Session descriptor fixes version, role,
capabilities, Resource scope, bounds, authority generation/lifetime, request
identity, and reconciliation behavior. A stale Intent is rejected or explicitly
reconciled, never silently retargeted.

Representation Update is not Interaction Outcome. An accepted Outcome names the
resulting Buffer version; Update delivery may follow independently and may
report a gap. A delta applies only to its named predecessor.

Interaction Intent contains neither raw device events nor presentation
coordinates. Representation Update may contain schema-qualified intrinsic
source values, including media pixels, but no target-screen pixels, terminal
cells, viewport transforms, drawing commands, or frame state.

## Selection, readiness, rollback, and shutdown

Assembly resolves configuration, selects exact role-specific components, and
constructs an inert dependency/ownership graph. Each component family exposes
side-effect-free applicability, contract/version, capabilities, bounds, and
authority requirements. Explicit configuration wins; multiple applicable
components without explicit selection are an error. Authority is checked before
probing that could reveal information or cause effects.

Selection returns typed missing and ambiguous outcomes before activation. It
does not panic, silently pick an equal-priority match, or probe the environment
from inside Kernel Runtime.

One Lifecycle Owner per subtree owns its readiness transaction and lifecycle
state. Kernel Runtime owns Kernel-side attachments. Client Runtime owns its
Family/Module/View/Presentation subtree. Product-process Extension Adapter or
Engine mechanics are subordinate to the applicable Server or Client Module
Lifecycle Owner. For a supervised child, the outside-Kernel Extension Host
Supervisor owns process, engine, code-load, invocation, and reap mechanics while
Kernel or Client Runtime retains semantic Module attachment, dispatch,
installation, and state. Protocol Adapter retains carrier ownership.

Construction is not readiness. Before external Ready publication, each owner
negotiates contract versions, capabilities, bounds, and authority; validates
components; attaches work under exact generations; and completes required
initial synchronization. Inspectable readiness predicates replace unverified
self-reports.

Ready publishes atomically only after every required child predicate passes. A
failure exposes no partial contribution, fences the generation, cancels owned
work, and reverses public child operations with dependants before dependencies
in reverse activation order. No detached task survives rollback.

Shutdown is the reverse ownership transaction:

1. stop external admission and publish Closing;
2. advance generation fences;
3. cancel and boundedly drain or reconcile owned work;
4. remove contributions and memberships;
5. close dependants before dependencies; and
6. release code/resources only after execution is terminal or its containing
   process is reaped, then publish one idempotent terminal Outcome.

Nonterminal same-process extension work remains failed-owned as
`IncompleteShutdown`; a deadline or generation fence never proves execution
stopped.

## Runtime relationship

~~~text
device input
    |
    v
Client View/input map
    |
    v
versioned Interaction Intent
    |
    v
Kernel Runtime -> Domain/Representation lowering -> typed Transaction
    |                                              |
    |                                              v
    |                                      Kernel Core / Buffer
    |                                              |
    +---------> correlated Interaction Outcome
    +<--------- bounded Representation Update <----+
                                                   |
                                      optional Resource publication
                                                   |
                                      injected Resource Adapter

Kernel or Client consumer -> exact Host Capability -> injected Host Adapter
external protocol -> External Protocol Binding -> public contract
extension code -> Extension Adapter -> public Module contract
~~~

One Buffer is the Transaction and History authority for Representations in one
working context. Client Views observe it through stamped Representation Updates;
they do not retain a second content authority.

## Deliberately unselected mechanisms

The architecture does not select package count, Rust trait signatures, scheduler
or thread model, cache shapes, eviction or Projection algorithms, universal
Intent opcodes, wire encoding, transport, renderer, global registry, permanent
native ABI, extension engine, VM, interpreter, JIT, worker realization, or
process/package topology. These choices require concrete consumers,
measurements, dependency review, and conformance evidence.

A general Editing IR or pass manager additionally requires two independently
implemented consumers with the same documented Transaction gap, measured
lowering/boundary costs, and explicit Transaction Contract Authority approval.
Until then, typed Transactions remain the common lowered form.

## Conformance

Static conformance checks registered package edges, owner-projected acyclicity,
transitive closure, source imports, and public types. Runtime conformance checks
authority-before-probe, negotiation, atomic readiness, rollback, fencing,
bounded pressure/cancellation, ordered shutdown, and absence of private
shortcuts. Passing one class never substitutes for another.

See [Resource, Snapshot, Buffer, and Transaction](../02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md)
and [Domain, Representation, Projection, and Client-Owned
View](../03-Semantics-and-View/01-Domains-Representations-and-Projections.md).
