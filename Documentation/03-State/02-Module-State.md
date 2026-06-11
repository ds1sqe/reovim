# 3.2 — Module State

**Scope.** View slots, module-owned opaque state, slot lifecycle,
and the contract that lets the kernel destroy slot bytes safely on
unload.

**Locked rules.** Carried forward (view-slot rules); references
`LF8`, `LF9` for cleanup ordering.

---

## 1. View slot model

```rust
pub struct ViewSlot {
    pub kind:             ModuleSlotKind,    // declared in module manifest
    pub owner_cdylib_id:  CdylibId,
    pub handle:           *mut c_void,       // opaque
    pub drop_fn:          unsafe extern "C" fn(*mut c_void),
    pub size:             u32,               // for limit checks (L23)
    pub flags:            u32,               // hostapi_reentrant, send_safe, ...
}
```

Slot kinds are declared in the module's manifest:

```toml
[[view_slot]]
kind = "vim.cursor-state"
abi  = "1.0"
```

`ModuleSlotKind` is a stable string interned by the kernel into a
`SlotKindId(u32)`.

## 2. Lifecycle

```
1. handler asks HostApi.view_slot_alloc(client, buffer, window, kind, init)
2. kernel verifies (client, buffer, window) is current focus
3. kernel calls module's slot_init(handle_ptr, init) via vtable
4. on success, kernel records the slot in registry
5. handler reads/writes through HostApi.view_slot_get / set
6. on detach (window close, buffer close, client leave),
   kernel calls slot.drop_fn while module is still Active
7. on module unload, all slots owned by the cdylib drop in step 5
   of 2.2 §5 (Destroying state)
```

## 3. Limits

- **L23**: max view-state size per slot (configured limit).
- **per-window**: bounded count of slots; cap from
  `kernel.host.[limits].max-slots-per-window`.

Allocation past either cap returns `ResourceExhausted`.

## 4. Drop discipline

> Drop functions must:
> - be `extern "C"`,
> - run in finite time (bounded; FAIL3),
> - panic-contain (AB13),
> - not call HostApi unless `drop_may_call_hostapi` flag is set.

A drop that exceeds its bounded wait is recorded as
`cdylib.unload.fail rollback=partial`.

## 5. Cross-window slot access — forbidden

A slot is keyed by `(ClientId, BufferId, WindowId, SlotKindId)`.
Modules cannot read another window's slot through any HostApi.
Cross-instance state must go through the service registry (3.3) or
a session-scoped service.

## 6. Buffer-scoped slots

Some kinds need buffer-scope rather than window-scope (e.g. a
parser cache). The manifest declares scope:

```toml
[[view_slot]]
kind  = "treesitter.parser-cache"
abi   = "1.0"
scope = "buffer"             # default: window
```

Buffer-scoped slots are keyed `(BufferId, SlotKindId)` and shared
across clients/windows attached to that buffer.

## Open items

1. Slot scope vocabulary — current spec has `window` and `buffer`.
   `session` and `client` may be added if a use case appears.
2. Whether slots can be enumerated by the kernel for diagnostics.
   Default: yes via DS12 `inventory.snapshot`.

## Conformance

| Behaviour | Fixture |
|---|---|
| Slot init/drop round-trip | Allocate slot, write, read, detach window — verify drop_fn called. |
| Drop on module unload | Allocate slots, unload module — verify all drops fired before `dlclose`. |
| Cross-window isolation | Module attempts to read slot owned by another (client, window) → `NotFound`. |
| Limit | Allocate past `max-slots-per-window` → `ResourceExhausted`. |
