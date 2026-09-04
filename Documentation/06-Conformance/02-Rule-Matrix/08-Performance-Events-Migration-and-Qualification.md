# Performance, events, migration, and qualification

[Rule Matrix index](../02-Rule-Matrix.md)


| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `PERF-001` | Input-to-present p99 is primary; command-to-present, CLI command-to-output, and stages are separately named and never substituted. | [Performance](../../04-Performance/01-Reaction-Metrics-Baselines-and-Workloads.md) | `PERF,TRACE` |
| `PERF-002` | Every product/support or exact-release claim maps respectively to its predeclared `RealProductScenario` or `QualificationProductScenario`, admissible Dataset provenance, Experiment Protocol, and content-addressed Bundle with all raw attempts. | [Performance](../../04-Performance/02-Scenarios-Bundles-and-Traces.md) | `SHAPE,PERF,REVIEW` |
| `PERF-003` | Every run has Interaction Trace Status; captured traces have a schema-valid Manifest and are loss-aware, generation-exact, clock-domain-correct, and mandatory for stage claims. | [Performance](../../04-Performance/02-Scenarios-Bundles-and-Traces.md) | `SHAPE,TRACE,PERF,EVENT` |
| `PERF-004` | IPC/transport, SDK, Client, rendering, adapter, and language rows are measured on real exact paths without substitution. | [Performance](../../04-Performance/03-Path-Overhead-and-Regression-Evidence.md) | `PERF,MEM,IO,QUAL` |
| `PERF-005` | Instrumentation has a prior cost/capacity model plus exact-artifact individual, combined, maximum-profile, and rate-matrix calibration against observations. | [Performance](../../04-Performance/03-Path-Overhead-and-Regression-Evidence.md) | `PERF,MEM,IO,REVIEW` |
| `PERF-006` | Lock/wait/hold/contention and freeze/deadlock/liveness behavior are measured through deterministic fault evidence and `RealProductScenario` or `QualificationProductScenario` evidence as applicable. | [Performance](../../04-Performance/03-Path-Overhead-and-Regression-Evidence.md) | `FAULT,PERF,EVENT` |
| `PERF-007` | Every claim field has a Measurement Capability Record proving semantics, coverage, uncertainty, overhead, blind spots, and unavailable/observed-zero treatment. | [Performance](../../04-Performance/02-Scenarios-Bundles-and-Traces.md) | `SHAPE,PERF,REVIEW` |
| `PERF-008` | One Performance Authority owns each budget, baseline, workload mapping, rationale, and deterministic invalidation/re-baseline decision. | [Performance](../../04-Performance/01-Reaction-Metrics-Baselines-and-Workloads.md) | `TRACE,PERF,REVIEW` |
| `EVT-001` | One structured Event Stream owns operational diagnostics; the ring is exactly 1 MiB and all loss is visible. | [Event stream](../../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `MODEL,EVENT,MEM` |
| `EVT-002` | Canonical rendering is byte-identical across ring, file, stderr, replay, export, and `reovim log`. | [Event stream](../../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `GOLDEN,EVENT` |
| `EVT-003` | Event, ring, renderer, subscriber, and sink failures remain bounded, non-recursive, and non-authoritative. | [Event stream](../../05-Editor-Event-Stream/02-Scope-Failure-and-Conformance.md) | `FAULT,EVENT,PERF` |
| `EVT-004` | Event provenance is editor-stamped, redacted before fan-out, and paired with typed Actionability/remediation where required. | [Event stream](../../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `SHAPE,SEC,EVENT` |
| `EVT-005` | One follow operation returns each available sequence at most once; cursor reuse replays deterministically, gaps stay explicit, snapshot/follow handoff is atomic, and foreign-incarnation cursors return typed stale/resynchronize. | [Event stream](../../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `MODEL,CONTRACT,FAULT` |
| `EVT-006` | Producer admission is a bounded NonBlockingInvocation with non-waiting synchronization, explicit rejection/drop/closed outcomes, and no synchronous sink or subscriber work. | [Event stream](../../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `MODEL,FAULT,PERF` |
| `MIG-001` | Every donor slice is adopt/adapt/rewrite/retire with a public target, evidence, cutover, and removal record. | [Migration](../../05-Migration/01-Selective-Migration.md) | `TRACE,REVIEW,INTEG` |
| `MIG-002` | Duplicate/stale paths and invalid compatibility shims are removed after the conforming path is proved. | [Migration](../../05-Migration/01-Selective-Migration.md) | `STATIC,INTEG,REVIEW` |
| `MIG-003` | Each migration slice starts from one accepted journey, ratifies and compiles its full public interface chain, then implements only reachable inner mechanisms, adapters, and Assembly. | [Migration](../../05-Migration/01-Selective-Migration.md) | `TRACE,CONTRACT,INTEG,REVIEW` |
| `QUAL-001` | Features enter a candidate only with complete public, fault, bounds, lifecycle, dependency, performance, Event, and qualification mapping. | [Quality](../../07-Mission-Critical-Quality/03-Admission-Qualification-and-Release-Scope.md) | `TRACE,REVIEW,QUAL` |
| `QUAL-002` | Exact TUI/CLI/Web artifacts and required SDK rows qualify on `x86_64-unknown-linux-gnu` before v0.16 release. | [Quality](../../07-Mission-Critical-Quality/03-Admission-Qualification-and-Release-Scope.md) | `INTEG,PERF,QUAL` |
| `QUAL-003` | Mission-critical language is scoped evidence discipline and never implies unnamed formal certification. | [Quality](../../07-Mission-Critical-Quality/01-Integrity-Effects-and-Fencing.md) | `DOC,REVIEW` |

The exact `kernel.startup` schema, canonical bytes, operation classes, and
cause mapping used by `EVT-002`, `EVT-004`, and `EVT-006` are owned by the
[startup family][startup].

[startup]:
  ../../05-Editor-Event-Stream/03-Startup-Family-and-Canonical-Bytes.md
