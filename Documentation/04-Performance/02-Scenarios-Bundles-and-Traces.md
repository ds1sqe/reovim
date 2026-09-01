# Scenarios, Experiment Bundles, and Traces

[Performance Contract index](README.md)

## Scenario and Experiment Bundle

A **Scenario** is an executable, versioned workflow with exactly one evidence
class:

- `RealProductScenario` runs the exact built or packaged artifact through the
  actual selected product, Client, SDK/extension/adapter, transport, rendering,
  and environment paths. It may support a scoped product or support claim.
- `QualificationProductScenario` runs the exact packaged candidate through its
  mapped public journey in the declared clean environment. Only this class may
  support exact-artifact qualification or a release-blocking performance gate.
- `SyntheticFaultScenario` uses deterministic injection or a synthetic subject
  to prove only its named state transition, fault, or probe behavior.
- `TraceReplayScenario` replays a retained input/interaction trace through its
  declared reconstruction and timing policy. It may prove deterministic
  reproduction or differential semantics, but cannot inherit the source run's
  latency, environment, representativeness, support, or qualification claim.
- `CalibrationScenario` estimates or measures one collector, clock, probe, or
  analysis method and supports only the named Measurement Capability Record.
- `MechanismBenchmark` isolates a bounded mechanism and may explain cost; it
  cannot replace a Real Product Scenario.

Before execution, every Scenario declares its evidence class, artifact and
dependency digests, target/environment, Dataset Manifest, product
configuration, Clients and presentation paths, SDK/extension/adapter rows,
transport/process topology, operation mix, concurrency and pressure, faults,
warm-up and sample policy, correctness oracle, metrics, trace profile, and
invalidators. The claim-to-workload matrix states which evidence and dataset
classes are admissible for each claim.

A Scenario claiming representative workflow behavior also declares the target
workflow population and the operation, arrival/inter-arrival, concurrency,
pressure, state/residency, and configuration distributions it represents. Each
distribution links retained captured observations or an empirically fitted
model, predeclared fit tests, residual bias, and claim limits. An artificial
mix without that evidence is a declared workload, not evidence of real-world
frequency or representativeness.

A **Dataset Manifest** classifies every input as `CapturedReal`,
`RepresentativeGenerated`, `AdversarialGenerated`, or `SyntheticModel` and
records collection source or generator/version/seed, canonical digest,
upstream acquisition identity and parent digests, population and sampling
method, inclusions/exclusions, size/shape/distribution, every transformation
with input/output digests plus implementation artifact/configuration identity,
licensing, consent, privacy, access policy, secret/personal-data screening and
disposition, retention/destruction policy, validation oracle, reconstruction
requirements, representativeness rationale, known bias, and claim limitations.
Generated input executed through
the real product does not turn the execution path into a model, but it proves
only the declared generated workload. A real-world representativeness claim
requires either `CapturedReal` data or a generator empirically fitted and
validated against retained `CapturedReal` parent distributions, with parent
digests, predeclared fit criteria, results, and residual bias. Purely generated
data cannot establish real-world prevalence or representativeness by owner
acceptance alone. A synthetic model never substitutes for a required real-
product path or real/representative data class.

Every run produces a content-addressed **Experiment Bundle** containing exact
Scenario, Dataset Manifest, and predeclared Experiment Protocol bytes/digests;
product, dependency, runner,
collector, and tool identities/checksums; exact effective Assembly, Runtime,
ExecutionFacility, Client, adapter, SDK, transport, rendering, instrumentation,
and limit configuration; invoked commands and tool arguments; all attempts and
raw samples; stdout/stderr, structured results, and applicable Event output;
environment setup, acceptance, cleanup, and residual-state evidence;
correctness/fidelity outcomes; every Measurement Capability Record; each
attempt's Interaction Trace Status and, for every `Captured` status, its
referenced Manifest, segment index/digests, and gaps; a privacy,
redaction, exclusion, and data-transformation ledger; analysis code and
configuration; checksums for every retained member; exact claim-to-workload
mapping; derived summaries; and one bundle content-index digest. Derived
summaries never replace raw observations. Observed, derived, estimated,
unavailable, and valid-observed-zero fields are distinguished.

Every attempt in a Bundle has exactly one **Interaction Trace Status**:
`Captured` with its Manifest digest, `Disabled`, or `NotApplicable`.
Disabled/NotApplicable records the predeclared reason, applicable claim
limitation, endpoint/root and terminal timing identities still retained, and
the Measurement Capability Record proving that narrower timing. It never
supplies causal-stage attribution. Calibration, Mechanism, and detailed-trace-
off control attempts therefore remain honest without fabricating an empty trace
or omitting trace policy.

An **Experiment Protocol** is declared before observing results. It names
hypotheses and supported claim/baseline/budget role; variant, treatment,
control, and calibration matrix; invoked commands/tools and output schemas;
warm-up, attempt, and sample counts; attempt ordering, randomization or
counterbalancing, and concurrency; fault and calibration schedule; stop rules;
statistical method; rerun limits; environment tolerances; exclusion/error
rules; and how every attempt contributes to the reported result. Its exact
resolved bytes and digest are retained in the Bundle. Post-hoc filtering or
selecting a preferred run invalidates a qualification claim.

## Measurement Capability Record

Every collector or probe contributing a field to a claim emits a versioned
**Measurement Capability Record**. It declares:

- metric identity/version, semantics, unit, start/end or count boundary, and
  valid aggregation;
- observer, data source and collection point, charge owner, collector/tool
  artifact and configuration, required privileges, and operating scope;
- clock domain, accuracy, resolution, calibration, drift, uncertainty, and
  comparability limits where time participates;
- admission and sampling policy, sample/root propagation, coverage,
  completeness rules, loss/overflow/error counters, and retention bounds;
- estimated and measured overhead-profile identity plus perturbation risks;
- blind spots, unsupported topologies or fields, failure modes, known bias,
  privacy/redaction, and transformations; and
- the exact claim classes it may support and the conditions for `unavailable`,
  `estimated`, and valid `observed_zero`.

Absence of an observation is `unavailable` unless the record proves that the
probe covered the complete relevant interval and that zero is a valid measured
value. A nearby collector, higher-resolution clock, or more privileged run does
not retroactively upgrade another record.

## Causal Interaction Trace

Experiment evidence uses a bounded, loss-aware causal DAG from admitted input
or command to terminal outcome and presentation/output. It is separate from the
Editor Event Stream and from the Causal Debug Graph's retained reverse index,
though all may share correlation identities.

Every trace is governed by an immutable **Interaction Trace Manifest** retained
in the Experiment Bundle. It names trace/schema version; exact source Scenario
digest and attempt identity; sample and root identities/namespaces;
root-admission sampling algorithm, configuration, seed and decision
propagation; mandatory and optional node, edge, and terminal
sets for each participating path; record/node/edge/byte/queue/work/retention
bounds and overflow policy; process/collector segments and their artifact
identities; clock domains and calibration mappings; capture/merge/order rules;
authority, privacy, redaction, and field-classification policy; every
normalization, filtering, enrichment, or other transformation; segment and
manifest digests; access and retention policy; replay/reconstruction procedure
and limitations; and gap/completeness rules. A derived or merged trace retains
the source-segment digests and transformation lineage.

Each applicable trace root records typed edges through input mapping, command
or mode state, Contract Session/transport, Domain/Representation lowering,
Transaction/Buffer/History transition, Projection and Representation Update,
Client/View/layout/render, adapter queues or GPU work where observable, and
Presentation handoff. Server/Client Module, CCM, Extension Adapter/Engine,
Host, Resource, publication, lock, and process boundaries add their exact nodes
when they participate.

Every node carries run/attempt/sample/root/node identity, owning process/task/
component, local sequence, operation/outcome, clock domain and raw interval,
work/payload/queue/allocation/copy observations, and exact applicable Session,
authority, Resource Access, Revision, Buffer incarnation/version,
Representation, Projection, Client/View/render/frame, CCM, adapter/engine/
module, and synchronization generations. Cross-process causality uses explicit
send/receive and correlation edges, never timestamp adjacency.

Sampling is selected at root admission and propagated. A sampled root is
complete only when mandatory nodes and terminal rules close without a relevant
gap. Overflow records missing counts and known identity frontiers rather than
inventing ancestry. A gap invalidates causal-path or stage-attribution claims
that require the missing relation; the valid end-to-end functional/timing
sample remains visible as trace-incomplete.

Clock-domain durations are not added unless a declared calibration proves
mapping, drift, uncertainty, and validity. Client-clock input-to-present and
origin-clock round trips remain authoritative; otherwise cross-clock one-way
values are unavailable.
