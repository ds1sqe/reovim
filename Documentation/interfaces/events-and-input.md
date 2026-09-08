# Events and Client input

Target contract. These conceptual names and signatures define required
behavior; they are not a claim that an event bus or resolver is implemented.

## Three different observation paths

Semantic events report committed owner state or its invalidation. They allow
clients/modules to observe editing without private state access. Typed request
outcomes remain authoritative for acceptance and external-effect certainty;
a lossy subscription cannot replace terminal-result ownership.

The structured Editor Event Stream is for diagnostics, including `reovim log`.
It is not a state replication feed. Causal graph and snapshot evidence links
input, requests, transitions, updates, and presentation without turning debug
records into state authority. See [debugging](../debugging.md).

## Semantic catalog

These names are the base catalog. Each payload also has its owner's lifetime,
stream incarnation/cursor, schema version, and available cause/request links.
Names do not imply a global bus or globally ordered cross-owner delivery.

| Event | Owner and payload |
| --- | --- |
| `buffer.opened` | Kernel: Buffer, Resource, initial state, capabilities. |
| `buffer.changed` | Kernel: prior/new state, transaction, History transition. |
| `buffer.closed` | Kernel: Buffer lifetime and terminal close outcome. |
| `resource.invalidated` | Resource: access, observed revision, known cause. |
| `resource.effect-completed` | Resource: operation, certainty, cause, proof. |
| `representation.updated` | Projection: attachment, state, lineage, payload. |
| `session.lifecycle-changed` | Session: generation, prior/new state, cause. |
| `module.lifecycle-changed` | Kernel/Client Runtime: module, state. |
| `client.buffer-switched` | Client: local slot, old/new Buffer binding. |
| `client.context-changed` | Client: context generation and changed keys. |

An unknown effect carries reconciliation status instead of an applied proof.
Resource invalidation does not silently overwrite Buffer state. Adapter feeds
must state whether they offer invalidation only or complete ordered changes.
A rejected edit produces its request outcome, not `buffer.changed`. A local
mode change produces no fabricated Buffer transaction. Client events cannot
expose a concrete viewport to Kernel state mechanisms.
Accepted undo/redo uses `buffer.changed` with the actual History edge and
resulting state. Notification reports acceptance; it is not a command to
change any client's Position/Selection. The [client contract](clients.md)
owns those response policies.

Representation update lineage is exactly one of full snapshot, delta,
invalidation, gap, or resynchronization. A delta names its exact predecessor
and result; invalidation supplies no replacement state. Snapshot/delta payload
schemas belong to the Representation, not the Kernel. They carry no terminal
cells, drawing commands, screen layout, or presentation timing.

## Subscribe, recover, and release

```text
owner.subscribe(scope, event_kinds, limits) -> subscription, snapshot, cursor
subscription.next(cursor) -> event, next_cursor | Pending | Gap | Closed
subscription.resynchronize(gap) -> snapshot, next_cursor | failure
subscription.unsubscribe() -> Closed | IncompleteShutdown
```

Scope and capabilities are checked before observation. A snapshot/checkpoint
handoff closes the initial subscription race; if it cannot, return explicit
resynchronizing state instead of Ready. State and payload bounds apply both
per subscriber and in aggregate. Retrieval never invokes consumer callbacks
under owner state authority. Module unload revokes and releases subscriptions.

One stream orders its deliveries; versions order Buffer transitions, and
Projection generations order interpreted state. Timestamps do not establish
cross-stream order. Outcome and corresponding update can arrive in either
order; correlate exact identities, never assume the latest update is yours.

Consumers install only matching lifetimes and ancestry. Duplicate/older state
does not advance installed state. Pressure, retention loss, source gaps, and
replacement are visible. A gap stops delta installation until a full snapshot
or explicit resynchronization is installed. Never guess missing edits.
Stream replacement rotates the incarnation; equal cursor numbers do not join
unrelated streams. Unsupported recovery requires a fresh authorized attach.

Update acknowledgement returns reserved delivery credit and describes
installed, duplicate, older, missing-predecessor, or fenced state. It proves
neither Buffer mutation nor presentation. Reserve bounded control capacity for
gap/close independently of data credit. Slow subscribers cannot block edits or
consume another subscriber's terminal reservation. Unsubscribe is idempotent;
bounded incomplete shutdown exposes remaining ownership, not false success.

## Client keymaps and resolver

The Client Family Core owns normalized input and resolver mechanics. Client
modules contribute keymaps/commands through that Family's public contract;
the selected resolver policy belongs to Client/module policy, not the Kernel.
Presentation adapters normalize device events without deciding edit meaning.

```text
client.register_keymap(module, scope, bindings) -> registration | failure
client.resolve(input, context_generation) -> resolution
resolution = PendingPrefix | LocalCommand | SemanticIntent | Unmapped | error
client.unregister_keymap(registration) -> Removed | failure
```

A binding declares normalized sequence, context predicates, command identity,
explicit priority, and owner generation. Registration validates command/grant
availability and sequence/context bounds. Identical selectors and priorities
conflict unless explicitly replaced by their authorized owner; registration
order is never an implicit precedence rule. Registrations cannot outlive their
module, and revocation fences pending resolution against the old generation.

The first profile selects active contexts from most specific local scope to
Family default; within a scope, larger explicit priority wins. Equal active
matches return `AmbiguousBinding` rather than running either command. A pending
chord has a bounded length and monotonic deadline. If a complete match is also
a prefix, wait for its continuation until that deadline, then select the
complete match; a prefix-only timeout clears pending input without an edit.
A mismatch clears the prefix and resolves the mismatching input once afresh.
Mode/focus changes invalidate pending chords from the old context.

Example: a Markdown-view binding for `Ctrl-S` outranks a Family default only
while that local scope is active. It creates an explicit save request for the
selected Buffer; the key itself never enters a Kernel transaction. User
configuration can install a higher-priority binding through the same API.

Committed text/composition is distinct from a logical key. A character key and
its text commit cannot insert twice. Control keys, text/paste, pointer input,
and composition have declared Family policies. Pointer coordinates must map
through current Client-owned view correspondence to semantic coordinates;
stale or ambiguous mapping rejects rather than guessing.

Local commands change Client state only. Editing commands submit public
semantic intents and wait for their actual acceptance before committing
editing-dependent selection changes. Rendering observes the resulting
Representation update separately. Input gaps reset/fence pressed, chord, and
composition state and require a fresh input stream; they never replay an
uncertain edit. Buffer outcome/update delivery continues during that recovery.

## Behavior checks

Exercise subscribe-during-edit without a missed state, duplicate/out-of-order
delivery, slow-consumer gaps/resync, denied observation, unload, and incomplete
close. Resolve context/priority conflicts, prefix timeout/mismatch, text/IME,
and input loss. The composed journey must link one input through its accepted
transaction and installed update to Client presentation, while another
Client's focus remains independent.
