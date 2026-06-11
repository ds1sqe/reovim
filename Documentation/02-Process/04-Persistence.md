# 2.4 — Persistence

**Scope.** The kernel's persistent state surface: what is written across
restarts, what is reconstructed, the restore order, and how
runtime-allocated identifiers are remapped.

**Heritage.** #763 cursor restore; #789 storage-neutral atomicity.

**Locked rules.** `PS1..PS2` new; references `CR9` (carrier
persistence with stable Domain names) and `LF15` (shutdown
sequence).

---

## 1. State boundary

| Persisted | Volatile (rebuilt) |
|---|---|
| Buffer bytes (per source) | `BufferId` (allocated at restore) |
| Buffer Domain attachments (by stable Domain name) | `DomainAttachmentId` |
| Cursor / viewport per (client, buffer, window) — as `CursorBlob` / `PositionBlob` | `WindowId`, `LayoutNodeId` |
| MRU buffer list | clients' `ClientId` |
| Force-override map? | NO — single-process, never persisted (CFG5) |
| Module per-client state? | YES, via module's persistence handler |
| Service state? | service-specific; see 3.3 |
| Stream state? | NO; streams reconstruct from substrate |

## 2. State root

State root: `$XDG_DATA_HOME/reovim/state/`.

Layout:

```
state/
├── sessions/<session-id>/
│   ├── buffers/<buffer-name>.bytes      # canonical bytes
│   ├── domains.toml                      # DomainAttachment summary by stable name
│   ├── cursors.toml                      # per (client, buffer, window) byte-encoded carriers
│   ├── viewports.toml                    # per window byte-encoded carriers
│   ├── modules/<module-name>.bin         # per-module persistence opaque blob
│   └── commit.toml                       # PS1 commit marker, written last
└── mru.toml                              # MRU file list
```

State root may be relocated by `kernel.shell.[editor].state-root`
(#763).

## 3. Restore order

```
1. Kernel boots; kernel.host + kernel.shell loaded
2. lockfile + library-root resolved
3. Modules + drivers loaded; participant init runs with ConfigSlice
4. State scan begins:
   a. read MRU list
   b. for each session, restore buffers from bytes
   c. attach Domains: for each persisted attachment, resolve stable
      Domain name through DomainRouter.intern -> DomainId; if codec
      not yet registered, place attachment in deferred-restore queue
   d. restore cursor/viewport carriers per stored (client, buffer,
      window) tuples; remap IDs via fresh allocation
   e. fire each module's persistence handler with its blob
5. attach clients (transport open); replay deferred-restore once
   their codecs become live (PM5)
```

## 4. Stable identity vs runtime identity

- Domain names are stable strings (`text`, `hex`, `mesh.svg`, …);
  numeric `DomainId(NonZeroU32)` is allocation-order dependent.
- Persistence stores names. Restore intern through
  `DomainRouter.intern_named()` → fresh `DomainId`.
- Buffer identity in storage is the buffer source path / canonical
  name; `BufferId` is fresh.
- Window identity in storage is the layout-position path
  (e.g. `root/h1/v0`); `WindowId` is fresh.

> **CR9 — Persistence stores stable Domain names, not numeric
> `DomainId`.** Restore remaps. Defined in
> `04-Domain-Substrate/05-Coordination.md`.

## 5. Deferred restore queue

Carriers and attachments whose codecs are not yet registered at
restore time wait in a queue, bounded by
`kernel.host.[limits].max-deferred-restore-carriers`.

When a `(domain_id, inner_id)` codec registers (cdylib reaches
Active state), the queue is scanned and matching entries become
live. Entries that exceed `deferred-restore-grace-ms` without a
matching codec are emitted as DS12 events
`persistence.restore.skip` and dropped.

## 6. Module-side persistence

Every module that holds per-client state declares a persistence
handler in its vtable. Handler signature (sketch):

```c
typedef struct {
    ErrorCode (*save)(ClientId, BufferId, void* user_data,
                 ByteBuf** out_blob);
    ErrorCode (*load)(ClientId, BufferId, ByteSlice blob);
} ModulePersistVtable;
```

The blob is opaque to the kernel; round-trip is byte-identical. The
module is responsible for blob versioning.

## 7. Restore ordering vs config

Persistence restore runs **after** module/driver init (which itself
runs after config materialisation, §1). Modules see their `ConfigSlice`
before their persistence blob.

If a module's config is invalid (PM6 Failed), its persistence blob
is **not** dropped — it remains on disk for the next boot when the
module may be reconfigured / repackaged.

## 8. Save atomicity

> **PS1 — Session saves commit by forward-recoverable atomic
> visibility.** A save produces a complete **state generation**: all
> content belonging to one save is written as a unit before that
> generation becomes visible. Visibility is **atomic**: a reader
> observes either the previous complete generation or the new
> complete generation, never a mix of the two. Visibility is ordered
> after the **durability point**: the generation's content is durable
> before it becomes the current generation. A commit that has not
> reached the visibility point leaves the previous generation intact
> and recoverable (**forward recoverability**): the in-progress
> generation is either completed forward at next boot or discarded,
> and the previous generation is restored intact if completion is
> not possible.
>
> *Reference realization for POSIX filesystems.* A session save never
> mutates `sessions/<id>/` in place. It builds a complete replacement
> directory `sessions/<id>.new-<gen>` (each file written temp → fsync
> → rename; `commit.toml` written last, carrying `state_version` and
> the save generation), fsyncs the directory, then commits:
>
> ```
> 1. rename sessions/<id>      → sessions/<id>.old   (if it exists)
> 2. rename sessions/<id>.new-<gen> → sessions/<id>
> 3. delete sessions/<id>.old
> ```
>
> Restore reads only `sessions/<id>` with a valid `commit.toml`.
> Crash recovery at next boot is forward-preferring: if `<id>` is
> absent and a committed `.new-<gen>` exists, complete step 2; if
> only `.old` exists, rename it back. An uncommitted `.new-*`
> (no valid marker) is removed with a `persistence.restore.drop`
> event.
>
> Single-file state (`mru.toml`) uses plain temp → fsync → rename.
> *Class*: runtime + kill-fixture.

## 9. Shutdown save

> **PS2 — Shutdown saves every live session before owner unload.**
> At LF15 phase 2 (2.2 §9), every live session is saved via PS1
> after its views complete their LF12 sequences, sessions in
> id-ascending order, MRU last. Persist handlers run while their
> owning cdylibs are `Active` — LF15's load-bearing edge. Save
> failures are non-blocking: emit `persistence.save.fail` and
> continue; PS1 guarantees the previous on-disk state survives any
> failed or abandoned save (the swap never committed). Each
> handler is bounded by `persist-timeout-ms` (2.2 §7).
> *Class*: runtime.

## 10. Migration of state shape

State files have version markers:

```toml
[__meta]
state_version = "1.0"
```

Major version changes require a one-shot migration tool. v1 has
state version `1.0`; future migrations are out of target.

## Open items

1. Whether MRU and cursor restore happen at session-attach or at
   kernel boot. Default: at attach (so multi-client semantics are
   sane).
2. ~~Detach/save ordering~~ — resolved (2.2 §8, LF12):
   detach handler → persist save → subscriptions → slot drops →
   handle release, leaf-first; save failures are non-blocking and
   emit `persistence.save.fail`.
3. Whether cursor restore can re-use a `WindowId` that was
   recreated for the same layout-position path. Default: no — fresh
   ID, restored carrier.
4. ~~Atomic write semantics for state files~~ — resolved (PS1):
   forward-recoverable directory swap per session; per-file
   temp+fsync+rename for single-file state.

## Conformance

| Rule | Fixture |
|---|---|
| PS1 | Kill fixture at each crash window: before commit → old state restores, stray `.new-*` removed with event; between renames 1 and 2 → swap completed forward at next boot; after rename 2 → `.old` removed. No fixture run ever restores a mixed state. |
| PS2 | Shutdown trace with N sessions: N PS1 commits and the MRU write all precede the first `cdylib.unload.start`; one failing persist handler → `persistence.save.fail`, remaining sessions still saved, failed session's previous state intact on disk. |
| (CR9 reference) | Persist a session with a Domain whose `DomainId` differs across runs; verify cursors restore. |
| (deferred restore) | Persist a Domain whose codec isn't yet loaded at boot; verify queue → live transition once codec arrives. |
| (state version) | Open state from a v0.x build; expect named diagnostic, no crash. |
