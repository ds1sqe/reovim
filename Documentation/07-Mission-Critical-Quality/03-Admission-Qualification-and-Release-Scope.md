# Admission, Qualification, and Release Scope

[Mission-Critical Quality index](README.md)

## Release-candidate feature-admission gate

A versioned feature-admission record names the feature's public journeys and,
for each journey, the consumer role and exact capability/authority grant. It
maps each operational failure to a required Event family or a typed not-
applicable rationale. For Rust, service, Python, and TypeScript surfaces it
records one of `unchanged`, `compatible-additive` with supported version range
and goldens, `breaking` with the new negotiation/version path, or `not-
applicable` with contract-owned rationale. Every performance claim maps to
workload identifiers in the Performance Authority's claim-to-workload matrix.

A feature enters a release candidate only when all applicable items exist:

- one owning contract and requirement identifier;
- allowed and forbidden dependency/type boundaries;
- public-only behavior journeys with no privileged first-party bypass;
- typed Effect Certainty and Outcome Cause, including conflict, stale state,
  cancellation, timeout, pressure, closure, and failure behavior;
- hard bounds and visible exhaustion for retained or concurrent work;
- activation, readiness, rollback, recovery, and shutdown behavior;
- deterministic tests for faults, timing, pressure, stale work, and closure;
- standard/test adapter shared conformance where Host capabilities are used;
- deterministic Resource fault/model conformance and deterministic Client
  presentation sink/goldens where those boundaries are used;
- security, trust, authority, redaction, and dependency review where exposed;
- workload-specific p99, memory, I/O, and instrumentation evidence;
- actionable structured events and correlation for operational failures;
- compatible public Rust, service, Python, and TypeScript impact where
  applicable; and
- a requirement-to-exact-artifact qualification plan for each environment the
  eventual release intends to claim.

A requirement may be explicitly not applicable with contract-owned rationale.
It is not silently omitted. A happy-path demo, passing unit test, issue closure,
or feature count cannot substitute for missing applicable evidence. Candidate
admission is not a release or qualification claim.

## Public reduction rule

First-party code has no privileged semantic path. A Client or extension uses
the same public Interaction Intent, Interaction Outcome, Representation Update,
Transaction, Resource/Host, SDK/service, and Event Stream contracts available
to another consumer with the same granted role.

If selected behavior cannot be expressed through those primitives, the design
review chooses one of three outcomes: improve the public contract, rebuild the
behavior above it, or remove the behavior from the release. It does not add a
private Buffer mutation, concrete-adapter lookup, or one-off Kernel callback.

## Qualification

Normative Documentation defines what evidence is required; it does not claim
the evidence already exists. Final release inclusion requires the exact
packaged candidate to pass its mapped qualification plan in every environment
the release claims. One clean-environment journey may satisfy several
requirements when traceability names that coverage; qualification need not
rerun the same artifact separately per feature. A qualification record names:

- exact artifact digest and source/build provenance;
- supported target, operating environment, architecture, and prerequisites;
- contract and schema identities/versions;
- public journeys, conformance rules, fault cases, and workloads executed;
- raw results, errors, exclusions, and known limitations;
- dependency, unsafe, license, and supply-chain review applicable to that
  artifact; and
- the reviewer and deterministic evidence invalidators.

A versioned **clean-environment contract** records an immutable base host/image
identity, fresh product installation and absence of prior product state, allowed
prerequisites installed explicitly by the qualification procedure, forbidden
ambient services/configuration/caches/credentials, network policy, and the
checks that prove setup and cleanup. A separate claimed-environment matrix maps
each claimed target, architecture, and environment class to a clean-environment
contract version, allowed prerequisites, and qualification suite. This contract
defines evidence shape. The first qualification attempt is
`x86_64-unknown-linux-gnu`; selection does not imply that any artifact has
qualified. The exact candidate includes and independently qualifies TUI, CLI,
and Web Family artifacts from one compatible release set, with Web's exact
browser/runtime/assets and TUI/CLI prerequisites recorded.

The exact packaged artifact runs in a clean environment before release
qualification. Testing a nearby checkout or a different locally linked SDK is
not artifact qualification. The record claims only its named scope; supported
targets without current evidence remain unqualified.

If the packaged candidate fails an applicable gate, release qualification is
blocked until the defect is corrected or the feature and its claim are removed
from the candidate. Planned scope is never reported as shipped scope.

A change to the artifact digest, contract/schema or qualification-relevant
configuration, dependency lock, target/runtime, workload, or qualification
procedure invalidates affected evidence. A newly disclosed vulnerability,
dependency-maintenance or license change, unsafe-surface discovery, or threat-
model finding also invalidates the affected security/dependency qualification
even when the artifact and lock digest are unchanged. Renewal is driven by
those events, not an unexplained calendar promise.

Regulated safety, security, medical, aviation, industrial-control, or other
formal certification is outside v0.16 unless the owner separately selects a
named standard, threat/safety model, assurance process, independent review,
and evidence program. Product prose must not imply such certification from the
mission-critical direction alone.

## v0.16 and v0.17

A versioned v0.16 release-scope matrix enumerates every admitted behavior, its
owning requirement identifiers, and the public journey that proves why it is
needed for the foundation. v0.16 proves the Kernel, Client boundary,
Resource/Buffer model, synchronized low/high Representations, Host and extension
seams, bounded diagnostics, and their qualification machinery only through that
matrix. TUI, CLI, and Web are non-removable v0.16 Client Families; a missing or
failing Family blocks release and cannot be marked not-applicable.

The official Module matrix selects Server/TUI/CLI `{Rust, Python}` and Web
`{TypeScript}`. Rust Server/TUI/CLI, Python Server, and TypeScript Web are
`RequiredV0.16`; Python TUI/CLI are `TargetNonBlocking`. On selection, each of
those seven rows enters as `PlannedMissing` and gains no Supported/Qualified
claim without independent public-path, exact-package, lifecycle/fault,
usability, performance, and clean-environment evidence. Excluded cells carry
no AdmissionStatus. Live row status belongs to the registry and retained
evidence, not this contract.

v0.16 also defines the language-neutral Extension Adapter/Engine Adapter
contracts, Adapter Kit, and TestOnly model. It selects no production engine,
embedded Lua/Python, Haskell binding, native/FFI profile, configuration/
automation role, or Neovim compatibility facade.

An unresolved defect that contradicts an applicable requirement in the matrix
is release-disqualifying until fixed or the affected feature and claim are
removed. Feature count, subjective “needed” status, or a known workaround does
not waive the gate.

v0.17 pursues Neovim feature parity through the v0.16 public contracts. Parity
does not weaken the feature-admission gate, reintroduce text or viewport types
into the Kernel, justify carrying a known-buggy path for feature count, or
automatically select a Neovim plugin/Lua/RPC/UI compatibility profile.

See [Performance](../04-Performance/README.md),
[Editor Event Stream](../05-Editor-Event-Stream/README.md), and
[Conformance Requirements](../06-Conformance/01-Requirements-and-Evidence.md).
