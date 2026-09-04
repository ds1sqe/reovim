# SDKs, Service Contracts, and Compatibility

[Extension contract index](README.md)

## Rust-first and later-language gates

Rust is the first implementation and reference projection, not semantic or wire
authority. Before later-language implementation or admission begins, the
**Rust-first gate** requires:

1. accepted language-neutral contract/schema and compatibility identities;
2. a versioned public Rust SDK registry and source/API compatibility record;
3. public-only Rust journeys for the common SDK and every required Rust binding
   row or other admitted Rust reference row;
4. deterministic lifecycle, bounds, cancellation, stale/fencing, panic,
   reentrancy, failure, and shutdown conformance;
5. shared semantic fixtures, an in-memory service oracle, canonical wire
   goldens, and generated/checkable metadata; and
6. applicable real performance, memory, Event Stream, and causal-trace
   evidence.

A separate **service and production-transport reference gate** then proves
negotiation, authority, encoding, bounds, backpressure, cancellation,
reconnect, process failure, shutdown, and transport measurements. A Rust
reference endpoint may implement the harness but creates no supported Rust
remote binding and proves no other language.

Only after both gates may Python and TypeScript implementation/admission begin.
Their tracks may then proceed independently. Python Server is required;
Python TUI and CLI remain non-blocking targets. TypeScript Web is required.
The required Python Server and TypeScript Web rows provide the minimum
installable later-language starter journeys; no other row can substitute for
either one.
A defect discovered by a later projection invalidates affected earlier
evidence and returns to the owning contract; Rust-first never freezes an
incorrect schema.

## Rust SDK

The Rust SDK exposes only registry-listed public traits and value types for
admitted Resource Adapter, Domain/Representation computation and lowering,
semantic command, diagnostic, Server Module, and Client Family Module points.
Resource Adapters remain distinct from Server Modules, and Kernel Core retains
Projection coordination. Private Core and Runtime modules are never extension
API.

The public surface carries explicit identity, revision, resource-native
addressing, authority, bounds, cancellation, lifecycle, and typed outcomes.
Every callable descriptor declares exactly one value from each Execution-
contract dimension:

- **InvocationClass:** `NonBlockingInvocation` or `CallerMayWait`;
- **CompletionClass:** `InlineTerminal` or `DeferredTerminal`;
- **DeliveryClass:** `Single` or `Stream`;
- **EffectClass:** `LocalOnly`, `ExternalObservation`, or `ExternalMutation`;
- **CallbackClass:** `NoUserCallback` or `MayInvokeUserCallback`; and
- **ReentrancyClass:** `NonReentrant` or `Reentrant`.

It also names its workload and budget class. `NonBlockingInvocation` permits
only descriptor-bounded in-process work with no I/O or wait. `CallerMayWait`
declares its deadline, cancellation, queue behavior, and prohibition on State
Lanes. Completion, Delivery, and Effect classifications preserve terminal,
stream, and Effect Certainty semantics independently of invocation return.
Callback and reentrancy classes state whether extension code may run during the
call and whether nested entry in the same Contract Session is accepted. An
unclassified callable is not public API.

The Rust SDK and public Client implementations remain physically outside Kernel
Core and Runtime. A re-export facade cannot turn private Runtime transport or
policy into a public boundary. The SDK version describes source/API
compatibility only. v0.16 makes no stable native binary ABI claim; a future ABI
requires a separately ratified contract, compatibility policy, conformance
suite, and qualification evidence.

## Language-neutral service contract

The out-of-process service contract is defined independently of programming
language, transport, serialization, and SDK implementation. Contract identity
and version are independent of Reovim product, package, SDK, and Git versions.

Its versioned schema covers:

- extension identity, Module role/Family, contract version, and implementation
  version;
- handshake, capabilities, grants, denial, scope, authority generation, and
  negotiated limits;
- Resource identity and Access Incarnation/Revision, Buffer identity and
  incarnation/version, Representation identity, Projection generation, typed
  address-space encoding, and bounded ranges;
- strict request, extensible response, Event, callback-reference, and bounded
  stream envelopes;
- Interaction Intents, terminal Interaction Outcomes, and Representation
  Updates without viewport or drawing instructions;
- local Transaction acceptance and typed rejection;
- Effect Certainty separated from Outcome Cause, including applied,
  definitely-not-applied, and outcome-unknown results;
- deadlines, cancellation, backpressure, ordering, correlation, duplicate
  request behavior, and reconciliation; and
- readiness, drain, shutdown, disconnect, restart, resynchronization, and
  recovery.

Functions or language closures do not cross the service boundary. Remote
callbacks use schema-qualified callback identities with explicit lifetime,
authority, bounds, cancellation, and delivery behavior. Large or sparse
content uses bounded ranges or streams. A wire address declares its address
space and canonical encoding and is never silently narrowed to a host integer.

A **Protocol Adapter** alone owns encoding, framing, carrier, connection, and
transport I/O. The semantic contract does not select a codec or production
transport. One behavior suite runs through the in-memory harness and every
supported production transport.

## Contract Sessions and compatibility

Every Contract Session negotiates a descriptor before Resource lookup,
attachment, activation, subscription, or mutation. The descriptor fixes
contract identity and selected version/epoch, Session identity/incarnation,
role/Family, operation capabilities, Resource scope, limits, authority
generation and lifetime/revocation, deadlines, request identity, and
reconciliation behavior. The composition's Admission Authority mints the
grant, and every request binds to it.

Failed negotiation or denied authority creates no partial registration, lookup
side channel, or capability grant. Requests reject unknown fields unless their
contract explicitly defines an extension point. Response fields and variants
are safely ignorable only when the schema explicitly marks the applicable
version range. Existing released fields are not repurposed. Unknown required
meaning returns a typed unsupported-version or capability outcome; incompatible
required semantics use a new identity/version.

Before the SDK Stability Release, compatibility remains explicit and testable
rather than an indefinite stability promise.
Changing authority, Effect Certainty, idempotency, ordering, delivery, or a
durable side effect is compatibility-significant even if bytes remain
decodable. One deterministic published compatibility matrix covers contract
identity/version, role, required capabilities, and optional capabilities.

All v0.16 SDK and service rows are Experimental API even when their independent
DeliveryGate requires them or their AdmissionStatus becomes Supported or
Qualified. A pre-stability incompatible change is allowed only through an
explicit new identity/version, compatibility fixtures, migration guidance, and
affected public-journey reruns. Private Core, Runtime, Family, and adapter APIs
remain unversioned Intra-Tree Interfaces and may break without an SDK migration
promise because they are forbidden from public use.

Request identity is unique within its negotiated scope. Reuse with another
payload is a protocol violation; identical reuse follows the declared duplicate
rule. The Session bounds retained terminal outcomes and their expiry and offers
the declared reconcile/status or state-resynchronization operation. Response
loss, disconnect, replacement, or expiry never authorizes blind mutation retry.
Restart creates a new negotiated Session and generation.

## Python and TypeScript SDKs

Python Server/TUI/CLI and TypeScript Web bindings use out-of-process service
boundaries. Neither interpreter is a Kernel or Client dependency. Each SDK
provides idiomatic helpers while preserving the language-neutral meaning,
including:

- generated or mechanically checked schema types and compatibility metadata;
- capability negotiation and least-privilege grants;
- bounded asynchronous streaming, backpressure, cancellation, and deadlines;
- Transaction and Interaction Intent/Outcome helpers retaining every identity,
  revision, address space, and Effect Certainty;
- Representation Update and gap/resynchronization helpers without Viewport or
  drawing policy;
- structured Event Stream and typed error mappings;
- public examples for each required or Supported binding row; and
- an author-runnable deterministic conformance harness.

Cross-language goldens derive from one schema authority. They cover negotiation,
unknown values, bounds, authority denial/revocation, cancellation at every
admission/handoff cut, stale identities and generations, duplicate identity,
response loss, outcome-unknown reconciliation and expiry, disconnect, rebind,
and shutdown. Generated code owns data shapes and language ergonomics only; it
does not own transport, retry, dispatch, or editing semantics.
