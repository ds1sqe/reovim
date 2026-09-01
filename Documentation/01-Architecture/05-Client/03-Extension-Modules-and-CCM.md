# Extension Modules and CCM

[Client contract index](README.md)

## 8. Extension Packages and module roles

An **Extension Package** may contain a Server Module Definition, zero or more
family-specific Client Module Definitions, shared schemas/assets, or any valid
combination. Its declarative manifest records:

- package and module identity/version;
- Server and Client Family roles;
- compatible public contracts and schema identities;
- required or optional pair/CCM shape;
- capabilities, grants, trust/process class, and bounds;
- lifecycle and dependencies; and
- applicable conformance, performance, and qualification workloads.

Discovery and manifest validation execute no module code.

Shipping TUI, CLI, and Web does not require every Extension Package to contain
all three Client definitions. A package declares its supported-family subset.
An absent package/family binding is registry scope, not failure of the Family's
own mandatory Module surface.

### 8.1 Server Module

A Server Module Instance attaches to an explicit server-side scope and uses
only registry-listed Domain, Representation, command, synchronization, or other
public extension points. It owns no Buffer/History authority, Resource Adapter,
View, or presentation policy and never mutates storage directly.

### 8.2 Client Module

A family Client Module contributes local input, mode, View, layout, decoration,
output, render, or presentation policy only through its Family Contract. It may
consume stamped semantic or module-sync state and adapt it locally. It does not
duplicate authoritative Server Module state or import another Family's module.

An editing effect returns through Interaction Intent, Domain/Representation
lowering, and a Buffer Transaction. Neither CCM nor a private module message is
an editing side door.

### 8.3 Process supervision

An outside-Kernel Extension Host Supervisor owns or records process launch,
attach, sandbox/environment, crash/restart, and reaping. It is not a semantic
module owner, Protocol Adapter, or ExecutionFacility realization.

Process-per-module, process-per-package, shared language host, and externally
managed topologies require separate selection and evidence. Every topology
preserves logical Module/Session identities, grants, bounds, isolation, and
lifecycle ownership.

## 9. CCM state classification

Every synchronization family classifies state before admission.

1. **Authoritative editing state.** Resource, Buffer, History, and accepted
   transitions remain Kernel-owned. Clients submit Intents and receive stamped
   Updates. CCM never owns or applies peer Buffer deltas.
2. **Client-local CLM state.** Viewport, layout, focus, local mode, frame,
   cursor, and selection remain local by default and are not synchronized.
3. **Explicit shared semantic or collaboration state.** Presence, shared
   cursor/selection, session mode, or another value enters CCM only under a
   versioned schema declaring authority, privacy, retention/expiry, conflict,
   bounds, lifecycle, and failure semantics.
4. **Shared module state.** A module-owned versioned schema may synchronize a
   Server Module and compatible Client Module Instances under one named
   Synchronization Authority.
5. **Derived Client cache.** Local replicas are generation-fenced,
   discardable, and never truth.
6. **Transport and Session state.** Carrier queues, credits, cursors, retries,
   and reconnect state belong to Contract Session and Protocol Adapter owners,
   not CCM semantics.

Promotion from Client-local to shared state is explicit. Matching names do not
merge authority.

CCM is cross-family only for common semantic or module schemas. A Family Module
maps accepted CCM state into local View/output state; that rendered/local result
is not synchronized as another Family's authority. Family-specific values need
an explicit common translation contract before crossing Family boundaries.

## 10. Synchronization Scope

A versioned **Synchronization Scope** names:

- scope identity and incarnation;
- one Synchronization Authority;
- membership generation;
- Client and module identities/incarnations;
- schema identity/version, grants, and privacy;
- state revision/generation;
- stream incarnation, ordering, and predecessor/result relations;
- payload, record, byte, queue, and concurrency bounds;
- retention and expiry; and
- typed terminal outcomes.

### 10.1 Topology and authority

The v0.16 reference composition is logically server-mediated. An Assembly-
selected server-side Synchronization Authority or Server Module validates
submissions and publishes accepted state to joined Clients through public
Contract Sessions.

Server mediation does not require one process or machine. CCM remains
physically topology-neutral. Protocol Adapters may realize local, remote,
relayed, multiplexed, brokered, or peer-assisted carriers only after their
separate admission. A carrier never grants synchronization or editing authority
and never lets a peer bypass the named authority. Kernel Core knows no carrier.

### 10.2 Snapshot, delivery, gaps, and resynchronization

Every synchronized stream declares:

- Snapshot-plus-checkpoint readiness;
- sequence/cursor and predecessor rules;
- duplicate and coalescing behavior;
- bounded per-Client queues and aggregate charges;
- fairness and slow-Client isolation;
- cancellation and terminal delivery;
- retention and loss visibility; and
- gap detection and resynchronization.

Coalescing is legal only when the schema declares intermediate states
unobservable and the replacement names the exact covered range.

Pressure, retention loss, restart, or missing ancestry produces a typed
**Synchronization Gap**. On a gap, the Client stops applying deltas and requests
a bounded authoritative Snapshot/resynchronization. It never guesses missing
state or treats a diagnostic Event as synchronized state.

A slow or failed Client must not block unrelated Clients or Kernel editing.
Reconnect creates a new Client, Contract Session, and membership incarnation.
Resume requires a retained compatible cursor/checkpoint; otherwise complete
resynchronization is required. Work from an earlier incarnation remains fenced.

## 11. Module Synchronization Contract

A paired Extension Package declares a versioned **Module Synchronization
Contract**, not merely matching Server and Client module names. It defines:

- compatible Server and Family Module contract versions;
- shared schema and Synchronization Authority;
- capabilities and grants;
- submission and publication operations;
- fan-out, coalescing, gaps, and resynchronization; and
- behavior when either side is absent.

Server-derived semantic UI data should ordinarily be a Representation delivered
as an Update. Auxiliary module-sync state is explicit collaboration or module
state, not arbitrary bytes or a private editing channel.

The normal editing path is:

```text
Family Client Module action
  -> Family Contract / common Interaction Intent
  -> optional Protocol Adapter / IPC
  -> Kernel Runtime plus selected Domain or Server Module policy
  -> Transaction accepted or typed rejection
  -> stamped Representation or CCM Update
  -> bounded per-Client delivery
  -> Family Client Module
  -> Client-local View or output
```

Each Server Module, Client Module, normal Client interaction, and CCM membership
uses a distinct Contract Session, identity/incarnation, grant, queue/byte/work/
concurrency bounds, cancellation, correlation, reconnect, readiness, and
shutdown, even when a carrier is multiplexed.

Protocol Adapter owns framing, codec, and carrier. An External Protocol Binding
appears only for a genuinely external protocol, not for Reovim's own module or
CCM contract.
