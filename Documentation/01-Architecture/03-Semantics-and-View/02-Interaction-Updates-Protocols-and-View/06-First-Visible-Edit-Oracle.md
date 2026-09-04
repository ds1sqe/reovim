# First Visible Edit Oracle

[Interaction, Updates, Protocols, and View](../02-Interaction-Updates-Protocols-and-View.md)

## Authority and inherited behavior

This chapter owns the first v0.16 visible-edit migration oracle. It defines a
target contract and does not claim that the target is implemented or
qualified.

The inherited user behavior is deliberately small: after a TUI becomes ready,
the input sequence `ihello<Esc>` produces a presented content row containing
`hello`. The sequence is donor stimulus, not a Kernel command language. Donor
mode ownership, raw-key protocol, full-buffer text refetch, polling, sleeps,
and unstamped change notification are not inherited contracts.

## Exact target composition

The deterministic fixture contains:

- one empty stable byte-addressed synthetic Resource;
- one Resource Access incarnation and immutable initial Snapshot;
- one Buffer at local version zero;
- one editable Text Representation and one Projection;
- one direct in-process Contract Session;
- independently owned Kernel and TUI Client Runtime subtrees;
- one TUI cell Rendering Capability over an 80-by-24 target; and
- one deterministic Presentation Adapter plus test controller.

The byte Resource and Text policy are concrete components outside the neutral
Kernel contracts. The fixed cell target is TUI-owned. No part of that concrete
shape becomes a universal Resource, Buffer, semantic, or presentation model.

## Readiness oracle

Assembly first constructs inert subtrees and injects exact public ports. Kernel
readiness requires its Resource Access, Buffer, Text policy, Representation,
Projection, and direct Session generations to be current.

TUI readiness requires compatible Session and Family contracts, the initial
stamped Text Representation snapshot, a live cell capability, and a completed
initial presentation handoff. The Client installs that snapshot atomically
before publishing Ready. The controller cannot inject input before Ready.

A typed Ready result or failure proves the transition. A timer, retry loop,
nonempty frame, port-listening message, or successful connection alone does
not.

## Input and semantic Intent

The controller expands `ihello<Esc>` into seven ordered TUI device-input
contributions. TUI-local input policy interprets `i` and `Esc` as local policy
state. Neither contribution creates a Kernel mode or crosses the Contract
Session.

For this bounded deterministic batch, the policy emits exactly one Text
insertion Intent. It carries the semantic value `hello` at the qualified Text
origin observed in the initial Representation. This fixture-specific
coalescing makes no claim about live input batching. A selected production
input policy needs its own behavior and latency evidence.

Before admission, the Intent names the exact:

- Contract Session incarnation and authority generation;
- Client role, request, root correlation, cancellation, and deadline;
- Resource Identity, Access Incarnation, Revision, and Snapshot;
- Buffer identity, incarnation, and observed version zero;
- Representation identity and Projection predecessor generation; and
- Text coordinate/value schemas and coordinate-space incarnation.

The generic Kernel validates the envelope, authority, stamps, and structural
bounds without decoding Text. Selected Text policy validates semantic fields
and completely lowers the Intent before Buffer State Lane authority.

## Acceptance, Outcome, and Update

The accepted path has one atomic Transaction from Buffer version zero to one.
It creates sufficient History and Projection invalidation state for coherent
reads. No Resource publication occurs.

Exactly one terminal Interaction Outcome names the request, Transaction, local
acceptance, and resulting Buffer version one. One Text Representation Update
names Buffer version one and exact predecessor and result Projection
generations. The Client installs it only against the named predecessor.

Outcome and Update are separate fan-out results. Either may arrive first. The
fixture waits for both and never treats an Update as proof of acceptance or an
Outcome as current Representation state.

## Presentation oracle

After installing the Update, the TUI advances its View, layout, render, and
frame generations. One later successful presentation handoff names those
generations, the cell capability, and the exact target incarnation.

The first content row has the ordered cell values `h`, `e`, `l`, `l`, and `o`
at columns zero through four. The test compares those semantic positions and
values. It does not use a content digest, serialized byte length, collection
cardinality, or a nonempty-frame proxy.

The Kernel never receives the target size, cells, View, layout, render, frame,
or presentation handle.

## Negative and recovery oracles

The contract fixture makes the following outcomes independently observable:

- incompatible negotiation or denied authority before revealing lookup;
- malformed or over-bound carrier before traversal or allocation;
- stale Access incarnation, Buffer version, or Projection generation;
- cancellation before admission and cancellation racing admitted work;
- Buffer conflict with a definitely-not-applied local result;
- Intent, Outcome, Update, Client, or Presentation pressure;
- duplicate or older Update as a typed no-op or stale result;
- missing Update ancestry as a Gap followed by full resynchronization;
- replacement generation and Presentation target loss;
- module failure without partial Buffer acceptance; and
- close before submission or racing admitted work.

No fault silently retargets work, refetches an unstamped current buffer,
guesses missing Representation state, or turns a diagnostic Event into state
authority.

## Close oracle

Closing first rejects new device input and Intents. It advances Client,
Session, Projection, capability, and adapter fences; resolves or exposes every
admitted terminal Outcome; drains or visibly discards only declared bounded
delivery; and closes dependants before their dependencies.

The TUI Presentation subtree and Client Runtime close before the Kernel
dependencies they consume. Repeated close returns the same terminal result.
Incomplete shutdown retains an explicit live owner. Sleeps and dropped task
handles do not prove termination.

## Causal and measurement oracle

One root correlation records actual edges through input observation, TUI
mapping, Intent send/receive, admission, Text lowering, Transaction acceptance,
Outcome delivery, Projection computation, Update delivery, Client state
installation, render completion, and presentation handoff.

Every node carries applicable before/result stamps. Reverse lookup from the
frame, Update, Outcome, Transaction, or input reaches known roots within
declared bounds or returns a typed gap. Outcome/Update delivery order and
timestamp adjacency never create causal ancestry.

Measurement points cover each named stage plus allocation, copy, queue,
lock/wait, and instrumentation observations where available. This oracle fixes
boundaries and correctness only. Numeric budgets require later real Scenario
baselines and retained raw data.
