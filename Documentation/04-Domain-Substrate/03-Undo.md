# 4.3 — Undo

**Scope.** Buffer-level undo/redo, byte-edit origin, undo group
boundaries.

**Heritage.** Flagged as "not yet chapter-consolidated" — this is a
minimum-viable shape with v0.16 defaults resolved in §Open items.

**Locked rules.** None new in this revision.

---

## 1. Origin

Every byte edit carries an origin:

```rust
pub enum Origin {
    User { client_id: ClientId },
    Module { cdylib_id: CdylibId, name: Arc<str> },
    External { source: ExternalSource },   // file watcher, autoreload, ...
    Restore,                                // from persistence
    Replay,                                 // PM5 replay queue
}
```

Origin gates undo participation: `Restore` and `Replay` do not
participate in undo by default.
`External` edits are recorded as non-undoable marker groups: they
preserve the timeline for diagnostics and redo invalidation, but an
undo command skips them and never rewrites bytes sourced from outside
the session.

## 2. UndoStack

```rust
pub struct UndoStack {
    pub group_open:   Option<UndoGroupId>,
    pub groups:       Vec<UndoGroup>,
    pub redo_groups:  Vec<UndoGroup>,
}

pub struct UndoGroup {
    pub id:          UndoGroupId,
    pub origin:      Origin,
    pub edits:       Vec<EditRecord>,
    pub timestamp:   SystemTime,
}

pub struct EditRecord {
    pub byte_range:  Range<usize>,
    pub old_bytes:   Bytes,
    pub new_bytes:   Bytes,
}
```

## 3. Group boundaries

Modules open and close undo groups via HostApi:

```c
hostapi_undo_group_open  (BufferId, Origin* origin);
hostapi_undo_group_close (BufferId);
```

Auto-close conditions:
- group exceeds `kernel.host.[limits].max-edits-per-group`,
- group exceeds `max-undo-group-age-ms` since open,
- buffer detach.

There is no group-merge primitive in v0.16. A module that needs a
macro, paste, or replay to undo as one unit must open one group before
emitting the first edit and close it after the last edit. Numbered
history or richer merge policy is module state, not kernel undo
mechanism.

## 4. Cross-buffer undo

Undo is per-buffer. Multi-buffer "session-wide undo" is
**out of target**.

## 5. Bounded resources

| Cap | Field |
|---|---|
| Max groups in stack | `max-undo-groups-per-buffer` |
| Max edits per group | `max-edits-per-group` |
| Max bytes retained per buffer | `max-undo-bytes-per-buffer` |

Eldest groups dropped on cap; DS12 event at drop.

## Open items (resolved for v0.16)

1. ~~External edit participation~~ — resolved (§1): external edits
   are marker groups and are not undoable.
2. ~~Module-driven group merging~~ — resolved (§3): no merge
   primitive in v0.16; modules group explicitly.
3. ~~Carrier-based undo~~ — out of v0.16 target. Phase 4 undo records
   byte ranges; Domain-specific carrier undo requires a future spec.

## Conformance

| Behaviour | Fixture |
|---|---|
| External marker | Apply user edit, external edit, user edit; undo skips the external marker and never rewrites its bytes. |
| Group boundary | Module opens one group, emits three edits, closes; one undo reverses all three. |
| Group cap | Exceed `max-edits-per-group` → group auto-closes and DS12 emits. |
| Redo invalidation | Undo a group, then apply a new user edit → redo stack clears. |
