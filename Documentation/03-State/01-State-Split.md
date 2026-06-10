# 3.1 — State Split

**Scope.** What lives in the Kernel, what lives per-Session, what lives in
module-owned opaque state, and the rules for moving state across
those boundaries.

**Heritage.** v3 `03-State/01-State-Split.md`.

**Locked rules.** Carried from v3.

---

## 1. Tiers

```
Kernel (instance-wide)
└─ Session (named editing context, multi-client)
   └─ Window-scoped view (per (client, buffer, window))
      └─ Module-owned view-state (opaque to kernel)
```

## 2. Kernel-level

The Kernel owns:

- registries (`Inventory`, `DomainRouter`, `ServiceRegistry`,
  `HandlerRegistry`, `ProjectorRegistry`, `ManifestRegistry`,
  `StreamRuntime`),
- effective config (`EffectiveConfig`),
- correlation allocator,
- DS12 event bus,
- force-override map,
- the session map (sharded `arch/`-provided RwLock).

Kernel state survives session deletes and client disconnects.

## 3. Session-level

Session owns:

- `buffers: Vec<BufferId>` membership,
- `clients: HashMap<ClientId, ClientView>`,
- `focus: HashMap<(ClientId, BufferId, WindowId), Vec<FocusEntry>>`,
- session-scoped registers,
- turn gate + state lock (see 2.3).

Session state survives client disconnects within the session's
lifetime; deleting the session destroys it.

## 4. Window-scoped view

For each `(ClientId, BufferId, WindowId)` triple the kernel allocates
**view slots** holding module-owned opaque state (see 3.2). The
kernel knows the slot exists and has a `drop_fn` for it; it does
not interpret the bytes.

## 5. Module-owned opaque state

Modules allocate per-`(client, buffer, window)` state through:

```c
view_slot_alloc(client_id, buffer_id, window_id,
                ModuleSlotKind kind, void* init_data, usize init_len,
                ViewSlotHandle* out)
```

The handle is opaque to kernel; the kernel stores `(handle, drop_fn,
owner_cdylib_id)`. Module reads/writes through HostApi by handle.

## 6. Movement rules

| From → To | Allowed | Notes |
|---|---|---|
| Kernel → Session | YES | only at session creation; carries config snapshot for that session |
| Session → Kernel | NO | session terminates; state goes to disk via persistence |
| Session → Window-scope | YES | window open allocates slots |
| Window-scope → Session | NO | view-state never bubbles back |
| Module → Module | NO | modules cannot share view-slots; use ServiceRegistry |
| Module ↔ Persistence | YES | module persistence handler (2.4 §6) |

## 7. Multi-client invariants

Within one session:

- buffers are shared (single content);
- modes, cursors, active buffers, viewports are per-client;
- view-slots are per-(client, buffer, window) — distinct triples,
  distinct slots.

A client switching its active buffer or window does not migrate
view-slots; the slots for the new (client, buffer, window) are
allocated lazily by handler dispatch.

## Open items

1. Whether Kernel-level state can be quiesced (snapshot for restart
   without process kill). Out of v4 target.
2. Per-session resource caps (max buffers, max windows, max
   clients). Currently global; per-session caps may be added.

## Conformance

This chapter constrains shapes. Rule fixtures live in registry
chapters (3.2, 3.3) and lifecycle (2.2).
