# Reaction Metrics, Baselines, and Workloads

[Performance Contract index](README.md)

## Primary reaction metric

The primary user-reaction metric is input-to-present p99 for a device
path. One Client-side measurement authority and one monotonic clock instance
measure from Client admission of raw device input before input/key mapping to
the selected Presentation Adapter's acknowledgment of frame or equivalent view-
update handoff. A report identifies the presentation target and does not equate
software acknowledgment with physical display latency.

An already semantic command or Interaction Intent uses a separately named
command-to-present workload starting at its admission. It does not include
device decoding/mapping and cannot be reported as input-to-present.

A CLI without a Presentation journey uses **command-to-output**, measured from
admission of an already semantic command to the declared terminal handoff of
its bounded text, byte, or structured output. The workload names whether that
terminal is buffer acceptance, consumer handoff, stream close, or completed
write/flush under the CLI contract. One CLI-side monotonic-clock authority owns
both endpoints. Successful terminal outputs have p50/p95/p99/maximum and byte/
record distributions; rejection, cancellation, pressure, timeout, partial or
failed output, and shutdown outcomes have separate counts and completion
distributions. The exact CLI qualification row maps this metric and endpoint.
It neither invents a View/frame nor reports command-to-output as command-to-
present or input-to-present.

The end-to-end interval includes command handling, transaction acceptance,
projection, layout, rendering, and presentation handoff. Stage timings may
explain the result but cannot replace it.

At minimum, correlated stage evidence separates:

- Client input admission/mapping and Protocol Adapter transport, if any;
- Kernel intent admission through Transaction, Projection, and bounded
  Representation Update handoff;
- Client update receipt through viewport/layout/render work; and
- presentation-adapter handoff.

The Kernel stage ends at a Representation Update, not at a fabricated frame;
the participating View-bearing Client owns viewport and drawing. Conversely, a
fast Kernel-only number is not an input-to-present claim.

Cross-process Kernel/adapter spans use correlation for attribution. Their
durations are not arithmetically summed into the end-to-end interval unless a
measurement contract proves the clock domains comparable; the Client-side
authority remains the end-to-end clock.

Successful presentations have their own completion-latency distribution.
Cancellation, rejection, conflict, pressure, timeout, outcome-unknown,
reconciliation, and missing-presentation rates and completion distributions are
reported separately. They are never mixed into success latency or silently
censored. Filtering failed samples out of an overall workload report is not a
valid performance claim.

Correctness constrains every measurement. A faster path is invalid if it loses
source fidelity, bypasses atomic state transition, installs stale work, hides
an outcome-unknown publication, weakens a bound, or omits diagnostic loss.

## Baseline before budgets

Numeric release budgets follow a reproducible baseline; they are not guessed
into the specification. A baseline is eligible only when its workload,
instrumentation, environment, build, sample policy, raw observations, and error
counts can be reproduced. A later budget record names the baseline and the
evidence supporting its threshold.

One named **Performance Authority** owns each versioned budget record. The
record names the claim and workload scope, approving reviewer, baseline,
threshold rationale, and evidence invalidators. Only that authority can approve
or change the budget. A change to the workload schema or endpoint, correctness
oracle, instrumentation, build/toolchain class, environment class, sampling or
statistical method, or another named invalidator requires re-baselining and
explicit review before the budget is reused.

The stable contract defines what must be measured and retained. Numeric
budgets, current results, regressions, branch revisions, and machine-specific
findings belong in versioned evidence outside normative architecture prose.

## Workload description

Every retained workload records at least:

- user intent and start/end event definitions;
- Resource kind, size, sparseness, residency state, and change shape;
- Domain, Representation, Projection, View, and presentation configuration;
- extension, Host, Event Stream, and diagnostic-output configuration;
- warm-up, iteration, sample, concurrency, and cancellation policy;
- source revision, toolchain, optimization and feature settings;
- machine, operating environment, display/presentation target, and relevant
  resource limits; and
- expected functional result and validation method.

Fixtures and generators are versioned with enough input identity to reproduce
the workload without relying on ambient user data.

The Performance Authority also owns a versioned **claim-to-workload matrix**.
Every reaction, resource, boundary-overhead, or release performance claim maps
to the exact workload identifiers, metric dimensions, required variants, and
environment classes that can support it. The matrix, rather than implementer
judgment, defines which editing paths are representative for a claim. Adding,
removing, or weakening a mapping is reviewed as a performance-contract change,
invalidates affected evidence, and cannot hide a failing workload without an
explicitly narrowed product claim.

## Evidence record

A result retains raw samples and a machine-readable summary containing sample
count, p50, p95, p99, maximum, errors, and exclusions. The result identifies
its versioned statistical method and configuration for confidence or run-to-run
variation, or records a typed not-applicable result with rationale when that
statistic has no valid meaning for the workload. It reports clock source and
resolution, instrumentation configuration, build identity, environment,
workload identity, and peak as well as retained memory.

The workload and result schemas jointly declare objective environment-
acceptance observations and tolerances, measurement-error codes and detection
rules, the maximum rerun count and selection/aggregation policy, and links to
every original and rerun attempt. They also declare a comparison-compatibility
key covering workload/fixture and endpoints, correctness oracle, metric and
statistical method, instrumentation, build class, and environment class, plus
the exact differences that remain comparable.

Memory evidence reports both per-owner and whole-product/process-tree initial,
peak, and steady retained memory. Shared allocations have one stated charge
owner and are also visible in whole-product totals. I/O evidence reports
operation and byte counts; queue evidence reports charged bytes/records,
pressure, and loss. Outcome-unknown publication evidence includes reconcile
latency, attempts, I/O, and retained-state cost.

Every published performance claim names its exact workload and links the raw
artifact. A single mean, best run, throughput number, or unqualified “fast”
claim is insufficient.

## Required measurement families

The quality contract covers every family selected by the
claim-to-workload matrix:

- input-to-present reaction for representative editing paths;
- opening, bounded access, editing, publication, and reopen for large and
  sparse Resources;
- initial, peak, and steady retained memory, including residency and eviction;
- projection and representation recomputation, including stale work;
- Host capability call and adapter overhead;
- in-process Rust and out-of-process service-extension overhead, including
  serialization, transport, backpressure, and failure cases;
- Client range request, Representation Update, resynchronization after a
  Representation Gap,
  viewport/layout/render, and update-to-present overhead without charging
  Client allocations to the Kernel or vice versa; and
- Event Stream producer, ring, rendering, subscriber, and diagnostic-output
  overhead under default, burst, slow-output, failed-output, and slow-follower
  workloads.

The required publication family includes admission/handoff/commit/receipt
fault cuts and outcome-unknown reconciliation. The Client family includes
duplicate, reordered, missing-predecessor, Representation Gap, and full-resynchronization
updates.

Paired configurations isolate the cost of a boundary or feature while the
end-to-end metric shows its user-visible effect. Disabled instrumentation is
measured as a control, not assumed to be free.
