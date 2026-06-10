# 3.3 — Service Registry

**Scope.** ABI-safe service registration and lookup; borrow-only vs
leased lookup; ownership and panic-containment of service rows.

**Heritage.** v3 `03-State/03-Services.md` (replaced); v4 README §9;
review item "ServiceRegistry is not ABI-safe".

**Locked rules.** `SVC1..SVC6`.

---

## 1. ServiceDescriptor

```rust
#[repr(C)]
pub struct ServiceDescriptor {
    pub key:              ServiceKey,         // stable interned string
    pub owner_cdylib_id:  CdylibId,
    pub abi:              AbiVersion,
    pub api:              Version,
    pub type_name:        StrSlice,           // for diagnostics; not equality
    pub vtable_ptr:       *const c_void,      // service vtable, #[repr(C)]
    pub vtable_size:      usize,              // size of self for AB3-style guard
    pub handle:           *mut c_void,        // opaque service object
    pub flags:            u32,                // §3
    pub drop_fn:          unsafe extern "C" fn(*mut c_void),
}
```

> **SVC2 — `ServiceDescriptor` is `#[repr(C)]` and versioned.**
> `vtable_ptr` is a `#[repr(C)]` service vtable; `vtable_size`
> guards future appended slots; `abi` and `api` follow AB3 / AB4
> rules. *Class*: ABI / runtime.

## 2. Lookup forms

### 2.1 Borrow-only

```c
ErrorCode hostapi_service_borrow(ServiceKey key, ServiceBorrow* out);
```

`ServiceBorrow` carries a pointer that is valid **only** for the
duration of the calling HostApi call. Caller must not retain.

### 2.2 ServiceLease

```c
ErrorCode hostapi_service_lease(ServiceKey key, ServiceLease* out);
ErrorCode hostapi_service_lease_release(ServiceLease lease);
```

`ServiceLease` holds the owner cdylib's `RefGuard` until the lease
is dropped or times out. Unload waits for outstanding leases per
FAIL3 timeouts.

> **SVC1 — Lookup is borrow-only or via `ServiceLease`; retained
> raw pointers forbidden.** *Class*: spec-asserted; runtime
> instrumentation flags retention violations in DS12.

## 3. Flags

| Flag | Meaning |
|---|---|
| `SEND_SAFE` | Service handle may move across threads. |
| `SYNC_SAFE` | Multiple concurrent calls allowed. |
| `HOSTAPI_REENTRANT` | Service vtable may call HostApi during execution. |
| `DROP_MAY_CALL_HOSTAPI` | `drop_fn` may call HostApi. (Default off.) |

Flags must be set conservatively; the kernel enforces them at
runtime (debug) and in conformance (release).

> **SVC5 — Service flags declare thread-safety and re-entrancy.**
> *Class*: runtime + conformance.

## 4. Registration / removal

```c
ErrorCode hostapi_service_register(const ServiceDescriptor* desc);
ErrorCode hostapi_service_unregister(ServiceKey key);
```

Registration:
- duplicate `ServiceKey` → `ErrorCode::Conflict`;
- mismatched `abi.major` → `ErrorCode::IncompatibleAbi`;
- runs while caller cdylib is in `Loaded` (init) or `Active` (post-init).

Unregister:
- callable only by the owner cdylib (verified via `owner_cdylib_id`);
- if outstanding leases exist, transitions row to `DrainingHidden`
  and waits per FAIL3 bound; on timeout returns `ErrorCode::Busy`
  but row remains hidden.

## 5. Owner unload

> **SVC3 — Owner unload removes service rows before `dlclose`.**
> Step 5 of 2.2 §5 calls `drop_fn` on every service this cdylib
> owns; row state moves to `Revoked`. *Class*: runtime.

> **SVC6 — Leases carry owner generation.** A lease that observes
> generation mismatch fails on next use with `ErrorCode::Stale`.
> *Class*: runtime.

## 6. Drop callback

> **SVC4 — Service drop callbacks panic-contained.** `drop_fn` runs
> under `catch_unwind` (AB13); panic → `TombstonedFailedUnload`.
> *Class*: runtime.

## 7. Service vtable shape

A service is exposed as a `#[repr(C)]` vtable:

```rust
#[repr(C)]
pub struct ExampleServiceVtable {
    pub header: VtableHeader,           // common AB3 header
    pub op_a:   unsafe extern "C" fn(*mut c_void, /* args */) -> i32,
    pub op_b:   unsafe extern "C" fn(*mut c_void, /* args */) -> i32,
    // ...
}
```

The `*mut c_void` first param is the service `handle` from the
descriptor.

## 8. Forbidden surface

- No `Box<dyn Any>`. The v3 model used `Box<dyn Any>` for service
  handles; v4 explicitly removes it from the cdylib-facing surface.
- No `TypeId`. Service identity is the `ServiceKey` string + the
  vtable's `kind` field.
- No raw `Arc`/`Rc` across the boundary; service ownership is
  expressed by `owner_cdylib_id` + `drop_fn`.

## Open items

1. Whether lease timeouts can be per-service (configurable in the
   service's own schema) or always come from `kernel.host.[limits]`.
   Default: kernel-wide.
2. Whether the kernel enforces `SEND_SAFE` / `SYNC_SAFE` at call
   time (debug build) — runtime would need to capture call thread
   identity.
3. Cross-cdylib service consumption: a module consuming a driver's
   service. Allowed; lease semantics handle it. Document the
   pattern in the chapter body.

## Conformance

| Rule | Fixture |
|---|---|
| SVC1 | Borrow-pointer retained past return → instrumentation flags violation. |
| SVC2 | Service descriptor with appended vtable slot loads on newer kernel; older kernel reads through `vtable_size`. |
| SVC3 | Owner unload with two registered services → both `drop_fn`s run before `dlclose`. |
| SVC4 | Panicking `drop_fn` → owner tombstoned, DS12 event. |
| SVC5 | Service marked `!SEND_SAFE` invoked from non-owner thread → debug instrumentation flags. |
| SVC6 | Lease held across owner unload → next use returns `Stale`. |
