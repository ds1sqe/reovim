# Local editing service

This executable composes production Core/Runtime and selected providers. Its
clients submit public requests and consume actual results; they cannot choose
accepted Buffer versions, History nodes or native working state.

```sh
cargo run --manifest-path product/Cargo.toml -p reovim-server --features std
```

An optional final argument selects `heading`, `keyed`, or `both` (default).
Each scenario starts the Runtime, attaches two clients, independently opens
one Resource, explicitly joins its Buffer, negotiates initial snapshots,
executes four edits, consumes both update streams and closes dependencies.
Stdout reports outcomes and interpreted state; stderr receives the canonical
diagnostic records after editing has stopped. Output failure is an error, not
evidence that the completed editing transactions were rolled back.

## Selected profile

| Owner | Selected behavior |
| --- | --- |
| Resource | Immutable in-memory source; byte and sparse keyed native models. |
| Domain | Replace one ASCII heading label; toggle one existing keyed value. |
| Representation | Native lowering and separately produced semantic snapshots. |
| Core | Immutable state leases, atomic installation, retained parent relations. |
| Runtime | Open, admission, bounded progress, exact replay and update delivery. |
| Client | Current observations and public requests, independent update install. |
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

This stage uses full snapshots only. Stream exhaustion produces explicit
invalidation, not invented recovery evidence. History traversal and snapshot
recovery are not implemented by this reference yet. There is no transport,
file publication, renderer, general Markdown parser or extension runtime.
Single-threaded direct execution does not measure IPC or presentation latency.

## Extraction and verification

The native-state and semantic behavior in the
[public contract witnesses](../../tests/kernel-interface-public-consumer/src/first_edit.rs)
was adapted into separately owned providers. State wrappers no longer require
Clone. Their capacity, malformed input and native preconditions have direct
provider tests. Core and Runtime were rewritten; the scripted journey
coordinator is not a product dependency or an alternate execution path.
Older History traversal remains only a behavior donor for the next slice.

[Executing tests](tests/executing.rs) exercise repeated edits, stale requests
and independent clients. [Core admission tests](tests/core_admission.rs) check
native preservation, immutable leases, fences and capacity. Fault injection
and diagnostic tests exercise the same composed path and public contracts.
`scripts/ci.sh` runs these with the pinned stable/nightly window; hosted std
assembly is feature-gated while the default library path compiles no_std.
