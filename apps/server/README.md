# Local editing service

This executable composes production Core/Runtime and selected providers. Its
clients submit public requests and consume actual results; they cannot choose
accepted Buffer versions, History nodes or native working state.

```sh
cargo run --manifest-path product/Cargo.toml -p reovim-server --features std
```

An optional argument selects `heading`, `keyed`, or `both` (default).
Each scenario starts the Runtime, attaches two clients, independently opens
one Resource, explicitly joins its Buffer, negotiates initial snapshots,
executes repeated edits, creates a sibling after undo, follows preferred and
explicit redo children, consumes both update streams and closes dependencies.
Stdout reports outcomes and interpreted state; stderr receives the canonical
diagnostic records after editing has stopped. Output failure is an error, not
evidence that the completed editing transactions were rolled back.

Append `--trace` to inspect the actual request-55 ancestry and captured
semantic state. The authorized reader reports missing/evicted evidence; it
never substitutes today's state. Receipt/install events identify the observer
separately from the originating request. These are not presentation events.

## Selected profile

| Owner | Selected behavior |
| --- | --- |
| Resource | Immutable in-memory source; byte and sparse keyed native models. |
| Domain | Replace one ASCII heading label; toggle one existing keyed value. |
| Representation | Native lowering and separately produced semantic snapshots. |
| Core | Immutable state leases, atomic installation, tree History and restore. |
| Runtime | Open, admission, bounded progress, exact replay and update delivery. |
| Client | Independent install, follow/keep policy and explicit translation. |
| Host | Explicit clock grant and non-waiting diagnostic ownership queue. |

The heading source is `# a\n` or `## a\n` followed by uninterpreted bytes.
Only the label changes; invalid source units outside that label survive.
The keyed family uses record keys, not byte-offset addresses. It supports
missing keys and retains unrelated values; no text types enter the Kernel.

The local assembly reserves 256 History nodes, 128 exact request records per
Session, two clients and four retained open requests. It rejects pressure;
there is no implicit request/History eviction. The diagnostic ring is bounded
by the common 1 MiB ceiling and reports gaps/loss independently of editing.
These are reference capacity limits, not ratified performance budgets.

Undo/redo selects existing opaque nodes with fresh Buffer versions. The native
model reconstructs actual state; Core installs it without replacing canonical
retained nodes. Returned prior materializations are released outside authority.
History needs the negotiated Session and observed Buffer, not an attached
Representation or a synthetic semantic intent. Edit and History share one
request-ID ledger, including cancellation, exact replay and terminal draining.

This stage uses full snapshots only. Slow/lost delivery produces an explicit
gap; exact receipt-bound recovery transfers a snapshot or typed failure.
Independent control credit keeps recovery possible when data credit is full.
Failed open retains cancellation/drain/close obligations until resolved.
There is no transport,
file publication, renderer, general Markdown parser or extension runtime.
Single-threaded direct execution does not measure IPC or presentation latency.

## Measuring this reference

Use the same Cargo invocation with `--release -- --measure`. It runs generated
small/64-KiB heading and sparse-keyed workloads: five warm-up and 100 measured
counterbalanced off/on pairs each. Every batch executes actual edit/undo/redo
and one intentional invalid request, with independent semantic oracles.
`--measure-smoke` runs one pair per workload to validate the experiment path.

Stdout is raw CSV, including failed/warm-up attempts. Stderr retains canonical
events, separate query/export timings, clock calibration and loss counters.
All endpoints use one Client clock; update timings include installation/ACK
by each client. Ring/trace/native memory fields are charges, not process RSS.
Detailed observations are disabled for off runs; endpoint probes and allocated
limits stay identical. No overhead is subtracted. Retain environment, exact
source/build and raw output before making a performance claim.

## Extraction and verification

The native-state and semantic behavior in the
[public contract witnesses](../../tests/kernel-interface-public-consumer/src/first_edit.rs)
was adapted into separately owned providers. State wrappers no longer require
Clone. Their capacity, malformed input and native preconditions have direct
provider tests. Core and Runtime were rewritten; no fixture completion hook
or scripted coordinator participates in the executing service.
The duplicate C.3 coordinator was retired after its valid fault cases moved
to [production-path checks](tests/retirement.rs); native witnesses remain.
Parent/child and preferred-redo behavior was adapted from the stable undo tree.
Text inverse edits, cursor restoration, clocks, ordinal IDs and implicit
pruning were excluded; selected native Restore supplies reconstruction.

[Executing tests](tests/executing.rs) exercise repeated edits, stale requests
and independent clients. [Core admission tests](tests/core_admission.rs) check
native preservation, immutable leases, fences and capacity.
[History tests](tests/history.rs) exercise retained siblings, exact edge
notifications and view-independent undo through both models.
Fault injection
and diagnostic tests exercise the same composed path and public contracts.
`scripts/ci.sh` runs these with the pinned stable/nightly window; hosted std
assembly is feature-gated while the default library path compiles no_std.
