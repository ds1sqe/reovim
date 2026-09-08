# Performance

Correctness comes first. A faster path is invalid if it loses fidelity,
installs stale state, hides publication uncertainty or silently drops work.
Numeric budgets follow reproducible experiments, not guesses.

## Measure the user journey

The primary metric is input-to-present p99: Client admission of device input,
before key mapping, to the Presentation Adapter's frame/update handoff
acknowledgment. Include command handling, transaction, projection, layout,
rendering and handoff. Software acknowledgment is not physical display time.

Use one Client monotonic clock for both endpoints. Already-semantic commands
have a separately named command-to-present metric. A no-view CLI measures
command-to-output with an explicit terminal boundary, not a fabricated frame.

Retain stage timings for input/resolver, transport, Kernel admission/commit,
representation update, Client layout/render and presentation. Kernel time
ends at semantic update, not at drawing. Do not sum cross-process clocks
without measured comparability, drift and uncertainty; otherwise one-way
duration is unavailable. Origin-clock round trips remain valid.

Separate successful latency distributions from rejection, conflict,
cancellation, timeout, pressure, unknown effects and missing presentation.
Keep all attempt/failure counts; never make failed samples disappear.

## Describe an experiment before running it

Record the user operation, expected result and start/end points; fixture or
dataset provenance/seed; resource size, sparseness and residency; clients,
modules, adapters, transports and limits; source/toolchain/build; machine and
presentation environment; warm-up, repetitions and concurrency; faults;
instrumentation; stop/rerun/exclusion rules; and statistical method.

Use real editor/service/client paths for product claims. Deterministic models
prove fault semantics, isolated benchmarks explain mechanism cost, replay
proves its stated reconstruction, and calibration measures a probe.
None alone proves production performance or an untested release artifact.

Generated workloads are useful but must be labelled. A claim about real-world
frequency needs captured observations or a validated model of them, with
known bias, privacy/license review and reconstruction instructions.

Retain commands/configuration, every attempt, raw samples, stdout/stderr,
functional oracle results, environment observations and analysis alongside
the result. Report p50/p95/p99/maximum, sample policy, variation/uncertainty,
failures and exclusions. A mean or best run is insufficient.

Each reported field distinguishes observed, derived, estimated, unavailable
and observed zero. Missing collection is not zero. Preserve source identity
and changes between attempts; a content digest may identify an artifact but
cannot serve as the behavior test or a manually maintained checkout manifest.

## Required workloads

- Open/read/edit/save/reopen large and sparse resources without default
  whole-resource materialization; include dirty/resident/evicted state.
- Measure initial, peak and retained memory per owner and for the process tree;
  charge shared storage once and include allocator/process overhead separately.
- Trace a small edit through both clients, including stale update rejection,
  resynchronization and presentation capability loss.
- Exercise standard and deterministic Host paths, extensions inactive/ready/
  active, normal/burst/slow-consumer pressure, and failure/shutdown.
- Measure semantic projection costs without hiding a full-state copy beneath
  a localized edit or charging Client allocations to Kernel.

## IPC and transports

Use the same semantic oracle over the direct reference, deterministic in-memory
envelopes, production codec/framing and each claimed real transport/runtime.
An in-memory pass does not establish IPC cost or another language's behavior.

Measure admission, translation, encode/decode, framing, queue wait, carrier
handoff/receipt, dispatch, terminal response/update and SDK delivery. Record
payload/carrier bytes, amplification, copies/allocations, retained memory,
throughput, CPU/I/O, wakeups/syscalls when observable, queue pressure and loss.
Include reconnect/resync, cancellation at handoff cuts, process failure and
shutdown. Name security, batching and process topology in the workload.

## Instrumentation and lock cost

Before measuring, estimate per-observation and aggregate cost: rates, bytes,
copies, allocations, atomics/locks, clock reads, queues, retention and export.
State assumptions and uncertainty; an estimate is not measured evidence.

Run paired detailed-instrumentation off/on profiles with minimum endpoint
timing retained and calibrated. Keep required safety enforcement enabled.
Compare individual probes, the claimed combination and maximum admitted
capture under idle, representative and burst/pressure workloads. Interleave
or counterbalance runs to expose warm-up and time-order drift.

Cover structured logging and every sink, causal graph/snapshots, lock/watchdog,
IPC, extension and rendering/GPU probes where used. Report effects on p99,
memory, queues, CPU, scheduling, loss and capture failure. Keep raw results
side by side; do not silently subtract overhead. Explain estimate/data gaps.

Measure lock wait/hold, contention/order, owner/waiter, queue delay, freeze
duration and cancellation. Use the classifications in [debugging](debugging.md);
elapsed time alone never proves deadlock. Deterministic schedules and real
hosted workloads provide different evidence and both are needed.

Capture/accounting must be bounded, non-recursive and independent of the
path it observes. Declare coverage, clock resolution, sampling/loss, retention,
privileges and blind spots. A failed collector invalidates only claims needing
its missing evidence; it cannot silently report a faster editor.

## Regression and release decisions

Publish a performance claim only with its workload and raw artifact link.
Compare equivalent functional outcomes, endpoints, instrumentation, build and
environment classes. Report absolute and relative differences and variation.
Changed assumptions require a new baseline, not reuse of an old threshold.

A release claim additionally runs the exact packaged candidate on every
claimed environment. Unsupported probes/topologies stay explicitly unavailable.
Budget changes require review of retained evidence; improving a median does
not excuse a p99, memory, correctness or failure-rate regression.
