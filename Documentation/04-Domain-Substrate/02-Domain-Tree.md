# 4.2 — Domain Tree

**Scope.** Buffer-attached Domain tree, focus chains, pending
attachments, and the resolution transaction.

**Heritage.** review items "DomainTree focus_refs vs cold children"
and "Pending(DomainId) lacks resolution context".

**Locked rules.** `DT1..DT9` carried (restated §10; DT8 reshaped),
`DT11..DT16` new. `DT10` lives in 4.1 with the dispatch rules.

---

## 1. Attachment shape

```rust
pub struct DomainAttachment {
    pub id:           DomainAttachmentId,
    pub buffer_id:    BufferId,
    pub domain_id:    DomainId,
    pub scope:        DomainScope,
    pub parent:       Option<DomainAttachmentId>,
    pub children:     Vec<DomainAttachmentId>,
    pub child_ordinal: u32,            // append order within parent/root list
    pub struct_refs:  u32,            // structural lifetime
    pub focus_refs:   u32,            // focus-chain lifetime
}
```

> **DT11 — Attachment lifetime separates structural refs and focus
> refs.** A cold structural child may have `focus_refs == 0`. It is
> removed only when its structural owner removes it AND `focus_refs == 0`.
> *Class*: runtime.

## 2. Scope

```rust
pub struct DomainScope {
    pub start: PositionCarrier,
    pub end:   PositionCarrier,
    pub flags: u8,                    // §4
}
```

Scope endpoints are position carriers in the **parent** Domain's
position vocabulary.

## 3. Focus chain

```rust
pub enum FocusEntry {
    Pending(PendingAttachmentId),
    Resolved(DomainAttachmentId),
}

pub type FocusChain = Vec<FocusEntry>;

// Session-level:
pub focus: HashMap<(ClientId, BufferId, WindowId), FocusChain>;
```

The chain is non-empty after `view_init`; only the **leaf** may be
`Pending` (L27 invariant).

## 4. PendingAttachment

```rust
pub struct PendingAttachment {
    pub id:                PendingAttachmentId,
    pub domain_id:         DomainId,
    pub parent:            Option<DomainAttachmentId>,
    pub scope:             DomainScope,
    pub buffer_id:         BufferId,
    pub client_id:         ClientId,
    pub window_id:         WindowId,
    pub bootstrap_payload: Vec<u8>,         // domain-defined init bytes; owned
                                            // copy — the entry outlives the
                                            // focus push that created it
    pub replay_queue_id:   ReplayQueueId,
}
```

> **DT12 — Pending focus entries use `PendingAttachmentId` with
> resolution context.** The pending entry carries everything needed
> to allocate the live attachment and view slot when the cdylib
> reaches `Active`. *Class*: runtime.

## 5. Resolution transaction

When the cdylib hosting `domain_id` reaches `Active`:

```
1. resolve bootstrap_payload through the domain's on_attach handler
2. allocate DomainAttachment (struct_refs = 1, focus_refs = 1)
3. allocate view slot if the module declared one for this kind
4. replay queued raw inputs from PendingAttachment.replay_queue_id,
   or cancel them if window_id no longer exists
5. CAS swap FocusEntry::Pending → FocusEntry::Resolved in the chain
```

> **DT13 — Resolution is transactional.** Either every step
> succeeds and the chain is updated, or all partial state is
> destroyed and the entry remains `Pending` (with a fail counter for
> the replay queue policy). *Class*: runtime.

If the cdylib transitions `Active → Failed` during resolution, the
pending entry is dropped and any window opened on it shows the
"module loading failed" placeholder.

## 6. Tree mutation

```c
hostapi_domain_attach(buffer_id, parent_attachment, domain_id,
                      scope, bootstrap, &out_attachment_id);
hostapi_domain_detach(attachment_id);

hostapi_focus_push(client, buffer, window, attachment_id);
hostapi_focus_pop (client, buffer, window);
```

`detach` requires `struct_refs == 1` and `focus_refs == 0` to
succeed. Otherwise returns `ErrorCode::Busy`.

Children keep stable append order. The kernel assigns
`child_ordinal` when an attachment is inserted under a parent (or in
the root list when `parent == None`). Ordering is not derived from
`scope.start`: scope carriers are Domain-specific bytes, and the
kernel has no semantic comparator for them. Domains that need
presentation order expose it through projectors, not through tree
storage.

A successful detach runs the LF12 teardown sequence (2.2 §8):
`OnDetach` → `OnPersistSave` → subscription cancellation → slot
drops → handle release, leaf-first across the focus chain, every
step bounded and non-blocking.

Detach requested during handler/projector dispatch is queued as a
mutation command and applied at CC17 step 7 (2.3 §2). The currently
invoked row is protected by its captured owner generation/refguard
until the slot returns; the detach can affect subsequent dispatches
but cannot invalidate the in-flight invocation.

## 7. Focus walks

Input dispatch walks the focus chain leaf-to-root until a handler
claims the input:

```
for entry in chain.rev() {
    match entry {
        Resolved(id) => {
            if let Some(handler) = lookup_handler(id, OnRawInput) {
                if handler.invoke(input) == Claimed { break; }
            }
        }
        Pending(pid) => {
            // route to PM5 replay queue; do NOT fall through
            replay_queue_push(pid, input);
            break;
        }
    }
}
```

(Pending leaves are bumpers — input never falls through them to
ancestors.)

## 8. Focus transition (Q-4)

A focus mutation publishes one paired `FocusTransition` record:

```rust
pub struct FocusTransition {
    pub session_id:    SessionId,
    pub client_id:     ClientId,
    pub buffer_id:     BufferId,
    pub window_id:     WindowId,
    pub seq:           u64,                  // monotonic per session
    pub before:        Vec<FocusEntrySnapshot>,
    pub after:         Vec<FocusEntrySnapshot>,
}

pub enum FocusEntrySnapshot {
    Pending(PendingAttachmentId, DomainId),  // domain_id snapshotted
    Resolved(DomainAttachmentId, DomainId),
}
```

> **DT14 — Focus changes publish one paired `FocusTransition` record.**
> Atomic mutation runs under `Session.state`; publication runs in the
> apply phase per CC17. *Class*: runtime.

> **DT15 — `ON_FOCUS_TRANSITIONED` carries before/after snapshots.**
> Pending entries appear in snapshots; pending entries do NOT receive
> the callback. *Class*: runtime.

> **DT16 — Per-session sequencing.** A monotonic per-session counter
> `seq` orders transitions; observers may reconstruct order by `seq`.
> *Class*: runtime.

Observers register via:

```c
hostapi_focus_transition_subscribe (SessionId, SubscriberHandle* out);
```

Subscriber callbacks fire **after** the apply phase (CC17). The
kernel does not guarantee cross-observer execution order; observers
that need pairing rely on the snapshot, not on callback completion
sequencing.

## 9. Bounded resources

| Cap | Field |
|---|---|
| Max attachments per buffer | `max-attachments-per-buffer` |
| Max children per attachment | `max-children-per-attachment` |
| Max focus chain depth | `max-focus-chain-depth` |
| Max replay queue entries (per pending) | `replay-queue-max` |

Breach returns `ErrorCode::ResourceExhausted` and emits DS12.

## 10. Carried rules (DT1..DT9)

The rule bodies below are the normative texts.

> **DT1 — Buffer = bytes; Domain = policy over substrate.** A
> Buffer is bytes + identity + byte-edit subscription — no
> interpretation, no codec, no Domain inside it (Linux inode
> analog). A Domain is policy registered on a `DomainId` (4.1)
> over a substrate. The substrate may be: raw bytes of a Buffer
> (root attachment); a parent attachment's projection (child
> attachment, scope endpoints in the parent's position
> vocabulary, §2); or a virtual byte stream synthesised by the
> Domain. *Class*: kernel-enforced (compile) — the Buffer contract
> has only byte ops.

> **DT2 — Forest of trees; multi-children per scope allowed.** A
> buffer may carry multiple parallel root interpretations (e.g.
> one `elf` root and one `raw-hex` root side by side). Multiple
> children at the same `(parent, scope)` are allowed when their
> `domain_id` differs — alternative views the user is choosing
> between. Children at the same `(parent, scope, domain_id)` are
> deduplicated (DT8). *Class*: kernel-enforced (compile).

> **DT3 — View-state per (client, attachment, window).** Each
> focus into an attachment allocates a fresh view slot via the
> kernel allocator using the size + alignment declared in the
> module manifest (`[[view_slot]]`, 7.1 §3); the module's
> view-init initialises the bytes in place. Multiple clients in
> the same attachment hold independent slots — bytes and effective
> destructor are per-slot, never shared. The slot carries
> `(ptr, layout, drop_fn)`; drop runs the cdylib's `view_drop`
> panic-contained (AB13), then deallocates via the kernel
> allocator. Slot access runs under the per-session state lock
> (2.3); thread-safety of the inner bytes is the Domain's
> responsibility. *Class*: kernel-enforced (compile).

> **DT4 — Read-write derived attachments translate through the
> parent's encode path, kernel-mediated.** A child attachment that
> accepts edits translates them back to buffer bytes through the
> parent Domain's encode codec, routed via the ServiceRegistry
> (3.3) — never as a direct cdylib-to-cdylib call (6.1 §6).
> Failure is a normal codec error: edit rejected, diagnostic
> surfaced, buffer unchanged. *Class*: spec-asserted.

> **DT5 — Codec primitives dissolve into Domain-internal
> libraries.** Decode / Encode / Index are libraries inside the
> Domain's cdylib, not kernel API surface. There is no
> `CodecAttachment` and no per-client codec selection state; the
> kernel's codec knowledge is the DomainRouter registration rows
> (4.1) and the carrier contract (4.5). *Class*: kernel-enforced
> (compile); depgraph probe — the content-codec uapi crate is not
> a dependency of any kernel rlib.

> **DT6 — Dispatch follows focus; observation fans out.** Input
> dispatch (`OnRawInput`) walks the focus chain leaf-to-root; the
> first `Claimed` verdict ends the walk (§7), and overlapping
> attachments do NOT see claimed input. Observation events
> (post-edit byte notifications, lifecycle hooks, focus
> transitions) fan out to every attachment whose scope overlaps
> the event, regardless of focus; observation handlers do not
> produce buffer mutations. *Class*: spec-asserted (DomainRouter
> dispatch path).

> **DT7 — Per-(client, buffer, window) focus chain.**
> `Session.focus` maps `(ClientId, BufferId, WindowId)` to a
> chain; `chain[0]` is a root, the last entry is the focused
> leaf. Entries are `Pending(PendingAttachmentId)` (DT12) or
> `Resolved(DomainAttachmentId)`. `Pending` appears at the leaf
> only (L27 invariant): a child entry exists because a *resolved*
> parent's handler attached it. Pending→Resolved transitions are
> CAS-swapped under the session state lock (DT13). `focus_refs`
> increments on `Resolved` entries only. *Class*: kernel-enforced
> (compile) — the enum forces every consumer to match both
> variants.

> **DT8 (reshaped) — Attachment dedup by
> `(parent, scope, domain_id)`; lifetime by the DT11 refcounts.**
> For any `(parent, scope, domain_id)` tuple, at most one
> `DomainAttachment` exists in the tree. Focus chains referencing
> it bump `focus_refs`; structural owners hold `struct_refs`.
> Removal requires both to reach zero per DT11 — the prior
> single-counter eager removal is superseded. Sharing benefit
> unchanged: an ELF section table is parsed once; clients share
> the structural record while each holds its own DT3 view slot.
> *Class*: spec-asserted — the router maintains the refcounts;
> cdylibs cannot write them.

> **DT9 — Focus chain atomicity under the session state lock.**
> Every focus-chain mutation runs inside the per-session state
> acquisition (2.3). Multi-step transitions ("switch root" =
> pop-all then push-new) hold the lock from first pop to last
> push; observers see the pre-state or the post-state, never an
> intermediate empty chain. Cross-observer pairing — previously
> deferred — is resolved by DT14..DT16 + CC17: the mutation publishes
> one `FocusTransition` record in the apply phase. *Class*:
> kernel-enforced (compile + runtime) — the session state lock is
> the only write path to `Session.focus`.

**Reshape note (DT1..DT9).** Carried forward with this spec's vocabulary:
`trait Domain` is removed, so DT1 reads "policy registered on a
`DomainId`"; the old `DomainScope::Derived(parent)` became the
`parent` field plus scope-in-parent-vocabulary shape (§1, §2);
DT7's `Pending(DomainId)` became `Pending(PendingAttachmentId)`
with owned resolution context (DT12). DT8 is the one semantic
change in the block: the single `focus_refs` counter is superseded
by the DT11 `struct_refs`/`focus_refs` split. DT9's deferred
cross-observer pairing is now resolved by DT14..DT16/CC17; the old
"future transactional API" sketch is dropped in favour of the
`FocusTransition` record. DT10 (priority bands) is restated in 4.1
§6 beside the dispatch rules it governs.

## Open items

1. ~~Children ordering~~ — resolved (§6): stable append order via
   `child_ordinal`; no kernel sort by Domain-specific carrier bytes.
2. ~~Detach during dispatch~~ — resolved (§6): enqueue and apply at
   CC17 step 7 after the in-flight slot returns.
3. ~~Q-4 D1..D5 sub-decisions~~ — resolved (§8, DT14..DT16):
   paired transition record, no cross-observer ordering guarantee,
   mutation under `Session.state` with publication in the apply phase,
   pending entries visible in snapshots but not called, legacy
   lost/gained events are derivable from before/after snapshots.

## Walking-skeleton subset note (#797)

A root-only single-entry focus chain `[Resolved(root_attachment)]`
with `struct_refs = focus_refs = 1` is the **minimal conformant tree**.
The following are deferred to their first multi-attachment consumer
(master Phase 4):

- `PendingAttachment` (DT12), multi-children (DT2), DT11 cold-child
  refcounts, and the DT14..DT16 focus-transition record.

The §1 struct is the spec target; the walking skeleton grows a
`FocusChain` as a single-element `[Resolved(root)]` array.

## Conformance

| Rule | Fixture |
|---|---|
| DT1 | Compile probe: Buffer contract exposes byte ops only; no Domain/codec type appears in its signatures. |
| DT2 | Two roots (`elf`, `raw-hex`) on one buffer coexist; attaching the same `(parent, scope, domain_id)` twice yields one record. |
| DT3 | Two clients focus the same attachment → two slots; dropping one runs one `view_drop`, the other slot is untouched. |
| DT4 | Derived edit with a failing parent encode → edit rejected, diagnostic emitted, buffer bytes unchanged. |
| DT5 | Depgraph probe: content-codec uapi is not a Cargo dep of any kernel rlib. |
| DT6 | Leaf claims input → ancestors never invoked; byte edit fans out to an overlapping non-focused attachment. |
| DT7 | Pending entry below a resolved entry is unrepresentable in fixtures; CAS swap observed under the state lock. |
| DT8 | Focus release with `struct_refs > 0` keeps the attachment; structural detach with `focus_refs > 0` returns `Busy`. |
| DT9 | Concurrent reader during switch-root sees the old chain or the new chain, never an empty one. |
| DT11 | Cold structural child with `focus_refs == 0` is not collected; structural detach removes it. |
| DT12 | Pending leaf carries enough context to reconstruct attachment without re-querying the focus push. |
| DT13 | Resolution failure mid-step → all partial state destroyed; chain unchanged. |
| Child order | Attach three children with unordered scope bytes → parent `children` remains append-ordinal order across repeated boots. |
| Detach during dispatch | Handler queues detach of its own attachment → current invocation completes; detach applies in CC17 step 7; next dispatch does not invoke it. |
| DT14..DT16 | Focus mutation publishes exactly one record with before/after snapshots and monotonic per-session seq; pending leaf appears in snapshots but receives no callback. |
| (focus walk) | Pending leaf swallows input; resolved ancestors never see it. |
