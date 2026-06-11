# 4.3 — Undo

**Scope.** Buffer-level undo/redo, byte-edit origin, undo group
boundaries.

**Heritage.** Flagged as "not yet chapter-consolidated" — this is a
minimum-viable shape; details TBD before lock.

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

## Open items (must resolve before lock)

1. Whether `External` edits participate in undo (e.g. autoreload).
   Default: no — external edits are recorded as a marker in the
   stack but are not undoable.
2. Module-driven group merging — current spec gives modules
   open/close control but no merge primitive. Use case may emerge
   (e.g. macro replay merging into one group).
3. Carrier-based undo — currently undo records byte ranges. A
   future model might record edits as `(PositionCarrier, payload)`
   so non-text Domains undo cleanly. Out of target.

## Conformance

This chapter is a sketch. Conformance fixtures land when §Open #1
is resolved.
