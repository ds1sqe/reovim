# Host Boundary and Capability Admission

[Host API index](../02-Host-API.md)

## Purpose

The **Host API** is a family of small Reovim Kernel Interface contracts through
which a specific portable consumer requests one semantic service from its
process environment. It exposes Reovim-owned capability meaning, values,
limits, and failures. It never exposes raw environment handles, global
platform tables, numeric platform constants, or target-specific branches.

Host is not a universal object, architecture layer, registry, service locator,
or generic environment abstraction. Kernel Core, Kernel Runtime, Resource
Adapters, and Clients receive only the independently negotiated capabilities
that their accepted public paths actually use.

A Host Capability supplies an environmental service. A **Resource Adapter**
connects one external or synthetic Resource and may consume individually
injected Host Capabilities. These responsibilities never merge.

## Ownership and dependency boundary

Each part has one narrow dependency and ownership rule:

- Kernel Interface owns Host Capability contracts. They may depend on
  Foundation and must not own Environment mechanisms, Resource semantics, or
  concrete configuration.
- Kernel Core or Kernel Runtime may consume only the exact Host Interface
  contracts justified by their public paths. They must not own concrete Host
  Adapter types or hidden discovery.
- Client Family or Client Runtime may consume public Client contracts and only
  their exact justified Host Interface contracts. They must not own a universal
  Host handle, concrete Host Adapter type, or Kernel-private state.
- A concrete Resource Adapter may consume Resource contracts and individual
  Host Capabilities. It must not own a universal Host handle.
- A Standard Host Adapter may consume its Host contract plus private standard
  library or admitted dependencies. It must not own Kernel policy or Resource
  interpretation.
- A Deterministic Test Adapter may consume the same Host contract plus a
  test-only controller. It must not weaken the production contract.
- Assembly may consume selected components and adapters. It must not redefine
  their semantics or own lifecycle state.

Kernel Interface must not depend on a Host implementation. Kernel Core and
Kernel Runtime must not import concrete adapter or environment types. A Host
Adapter must not call private Buffer or Core mechanisms. A Resource Adapter may
depend on an individual Host Capability contract; that edge never permits Host
to depend back on Resource.

Assembly selects and constructs inert Host Adapters and records their ownership
graph. The named Lifecycle Owner for the consuming subtree activates, grants,
supervises, and closes each injected capability instance and its admitted work.
Client Runtime is that owner for a Client subtree; Kernel Runtime is that owner
for a Kernel subtree. No consumer discovers an implementation through a global.

## Initial closed scope

The initial Host API contains exactly **MonotonicClock** and
**DiagnosticOutput**.

- The Assembly-selected, Runtime-owned private `ExecutionFacility` is not a
  Host Capability, public service, or extension API.
- An Extension Adapter, Extension Engine Adapter, Extension Host Supervisor,
  interpreter, VM, task runtime, worker pool, scheduler, or callback executor
  is not a Host Capability.
- Protocol and Presentation Adapters remain independently owned even when
  their implementations use environmental services.

Kernel or Client Runtime may use its private `ExecutionFacility` to realize its
own bounded work, but neither an extension nor a Host consumer receives that
facility as a handle. Extension execution and hosting follow
[Extension Contracts](../03-Extensions/README.md).

## Capability admission

A new capability enters the Host Interface family only when all of the
following exist together:

- one named accepted public path, owning boundary, concrete consumer, and exact
  operation;
- contract identity and compatible-version negotiation independent of the
  Reovim package version;
- contract-owned values, typed errors, and hard limits;
- authority validation before lookup, expansion, allocation, or external work;
- separate Effect Certainty and Outcome Cause for mutation, plus explicit
  unsupported, pressure, cancellation, closing, closed, timeout, and failure
  outcomes where applicable;
- ownership, lifecycle, and cancellation behavior for work that may wait,
  queue, or outlive a call;
- a retained operation-specific rationale and evidence for any justified
  immediate or no-waiting lifecycle/cancellation case;
- one Standard Host Adapter and one Deterministic Test Adapter;
- one shared behavioral conformance suite;
- bounded, redacted lifecycle and failure events, or a justified non-recursive
  bounded health record when event emission would require the failing
  capability itself; and
- measurements mapped to exact workload identifiers for latency, allocation,
  memory, queue, and pressure behavior.

Optional Host behavior is negotiated before first use. Unsupported service is
an ordinary typed outcome, never a panic, environment probe, or fallback to a
broader Host object.

The initial two-capability set is not a permanent ceiling. A later capability
requires another explicit owner decision plus this complete admission record;
implementation convenience or historical usage cannot add it implicitly.
