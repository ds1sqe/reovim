# Lifecycle, Isolation, and Authority

[Mission-Critical Quality index](README.md)

## Transactional lifecycle

A long-lived component has explicit states and one lifecycle owner. Assembly
resolves configuration, selects concrete components, and constructs an inert
dependency/ownership graph. It may trigger the root lifecycle operation but
does not share readiness state. One named Lifecycle Owner per subtree:

1. receives the inert dependencies without advertising readiness;
2. validates contracts, versions, capabilities, limits, and authority;
3. activates components in declared dependency order;
4. publishes readiness only after the complete required set is usable;
5. rolls back partial activation on failure;
6. supervises admitted work and applies only bounded, declared restart policy;
7. stops new admission during shutdown; and
8. cancels, drains or discards by contract, and releases components in reverse
   dependency order within declared bounds.

The selected-component descriptor enumerates required and optional components
and gives each one a testable readiness predicate. Failure of a required
component prevents readiness. Failure of an optional component permits a
declared degraded-ready state only when the descriptor names the removed
capabilities, typed user-visible status, and remaining safe operations. The
readiness result lists every active, degraded, and unavailable component; an
implicit or unreported degraded mode is forbidden.

Kernel Runtime is Lifecycle Owner for the Kernel-side subtree. Client Runtime
is Lifecycle Owner for a Client/View/Presentation and client-side Protocol
Adapter subtree, including independently attaching Clients. Their lifecycle
state remains separate across the Contract Session.

Selection fails with typed missing-component or ambiguous-component outcomes
before mutation. It does not panic, silently choose an equal-priority match, or
perform environment probing from portable Kernel code.

Shutdown and rollback are idempotent. A task, callback, subscription, process,
or output worker that can outlive its owner is nonconforming unless an explicit
external owner accepts it through a public contract.

## Development defects and production behavior

The standard production composition is `ObserveOnly`: liveness diagnostics may
observe, fence unsafe new work, poison a failed-owned boundary, and fail a
qualification Scenario, but never panic, abort, kill, unlock, retry, restart,
or rewrite editing state solely because a watchdog classified a condition.

The standard development/test composition is `DevelopmentFailFast`. A proved
lock-order violation, proved deadlock, lost mandatory causal spine, unobserved
task panic, or independently confirmed disallowed freeze captures one bounded
diagnostic snapshot and reverse causal subgraph, seals or records failure to
seal through an independently readable path, attempts one Event summary, and
then requests process/test-fatal disposition. The first trigger wins; capture
failure cannot postpone termination past the declared outer bound.

A development crash is diagnosis, not rollback or proof of correction. Fatal
capture preserves only the Effect Certainty evidenced at the cut. External
mutations already handed off retain a crash-survivable reconciliation owner/
witness or the composition is nonconforming. No panic unwinds across an SDK,
user callback, native/FFI, or another non-unwind-safe boundary.

## Bounds and isolation

Every queue, payload, retained history, resident cache, retry loop,
concurrency pool, subscription, extension request, diagnostic path, drain, and
shutdown wait has a declared bound or a justified immediate/no-retention
contract. Both per-consumer and subsystem-wide consumer count, record count,
charged bytes, and concurrency are bounded where many individually bounded
objects can accumulate. Shared storage has one charge owner. Pressure produces
an explicit outcome or Event Gap for unavailable admitted sequences;
pre-admission loss uses counters, never a fabricated Event Gap.

A slow or failed Client, extension, subscriber, Resource Adapter, Host Adapter,
Protocol Adapter, or diagnostic output is isolated from unrelated consumers.
The editing path blocks on an external component only when a public contract
explicitly admits that behavior and includes cancellation and deadline rules.

Each subsystem descriptor defines its isolation groups and the maximum
synchronous calls/work units, payload and temporary bytes, lock-wait attempts or
duration, and deadline for an admitted cross-component call. Consumers in
different groups do not share an unbounded queue, lock, or worker. Any shared
arbiter has explicit fairness and pressure outcomes within those bounds;
“unrelated” and “non-blocking” are not left to implementation judgment.

Every lifecycle phase has an outer deadline or another hard completion bound;
a component-local timeout does not justify an unbounded Runtime join or wait.

## Dependency and boundary enforcement

Repository paths assign every in-scope source responsibility one Source Owner,
Source Provenance, and Source Use under separate committed policy. Product
ArchitecturalRoles and the closed non-product `RepositoryTooling` owner remain
distinct. Provenance or use never grants a role or dependency edge, and source
classification never replaces a Documentation Contract Authority.

Locked build metadata is dependency-edge authority. Separate schema-valid
committed inputs enumerate exact workspace topology, source classification,
entrypoint reviews, and third-party Cargo-package admissions. Other dependency
mechanisms retain their own exact pins, admission decisions, and review
records.
Metadata is declared-graph evidence, not compiler-active source, helper,
package-membership, or public-consumer evidence. A Product claim additionally
requires the version-matched Product-closure and execution-tool admissions and
correlated compiler, package, and external-consumer evidence defined by the
Conformance Product Proof Profile.
Automated checks reject unregistered packages/targets, build mechanisms,
target/feature profiles, unmatched or ambiguous source, forbidden edges, and
cycles in both the concrete graph and its ArchitecturalRole-only product
projection. Repository Tooling is never projected into the product DAG,
selected by Assembly, or admitted into a shipping closure. Its concrete graph
remains dependency-reviewed and cycle-checked.

A product package cannot reach Repository Tooling through normal, dev/test,
build, or proc-macro dependency; import, include/macro expansion, re-export, or
public type; build/package invocation; or bundled file/asset. Repository Tooling
does not import product crates or private product types. It may inspect exact
product source or artifacts only as bounded evidence data. Generated production
source retains its product owner and is not mislabeled as build output.

Graph shape alone does not prove portability or semantic isolation. Source-
import/include, transitive-closure, invocation/packaging, and public-type checks
also reject direct OS,
filesystem, environment, socket, protocol, codec, input-device, terminal,
renderer, and platform mechanisms inside Foundation and Kernel Interface,
Core, and Runtime. Public Clients and SDK
implementations physically remain outside Kernel Runtime; an outer re-export
facade does not cure an inward concrete implementation.

Layer conformance also does not prove Resource atomicity or durability. Each
publishable Resource family supplies its own crash, partial-write,
reconciliation, and outcome-certainty evidence for the guarantees it claims.

## Authority and exposure

Identity is not authority. Before Resource lookup, bounded expansion,
attachment, subscription, or mutation, the boundary verifies the negotiated
contract, capability grant, limits, and applicable trust policy. Authorization
precedes lookup when lookup itself could reveal protected existence or
metadata.

The owning threat/authority contract classifies every lookup operation and
observable field as public or protected before implementation. The
classification covers existence, typed error distinction, counts and omitted-
item signals, metadata, and material timing distinctions where they can reveal
protected state. Unclassified lookup is treated as protected and denied before
resource/provider access until the contract is revised.

An operation captures an immutable authority context for its lifetime. An
extension, Client, or adapter receives no private Kernel Runtime, Host Adapter,
or concrete storage handle. Secrets and raw handles do not enter events or
cross a public contract when a bounded reference is sufficient. Redaction
occurs before event fan-out.

Network or nonlocal exposure fails closed until a concrete transport threat
model, authentication, authorization, resource limits, and shutdown behavior
are ratified and tested. Locality alone is not proof of user identity or trust.
