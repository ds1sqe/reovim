# 6.2 — Versioning and Vtables

**Scope.** Common vtable header, append-only slot rule with
`size_of_self` guard, AbiVersion / ApiVersion semantics, error
convention, panic isolation, the userspace covenant (AB15), and
the retirement of driver-ABI-v1's `c_int` returns (AB14).

**Heritage.** v3 `06-ABI/02-Versioning.md`; v4 README §8;
`archive/docs/architecture/driver-abi-v1.md`.

**Locked rules.** `AB1..AB4` carried, `AB3` reshaped, `AB12..AB13`
new. `AB5`, `AB6` deprecated.

---

## 1. AbiVersion and ApiVersion

```rust
#[repr(C)]
pub struct AbiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub _pad:  u16,
}

#[repr(C)]
pub struct Version {     // ApiVersion (kind-specific semantic contract)
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub _pad:  u16,
}
```

- `AbiVersion` is the **binary epoch**. Major mismatch → reject.
  Minor `cdylib > kernel` → reject. Patch ignored at load.
- `Version` is the **semantic contract** within a fixed ABI: a
  driver-render API can grow new operations across api-minor without
  breaking the ABI shell.

## 2. Common vtable header

```rust
#[repr(C)]
pub struct VtableHeader {
    pub abi:          AbiVersion,
    pub api:          Version,
    pub size_of_self: usize,           // bytes in this vtable instance
    pub kind:         ManifestKind,    // e.g. driver-server, module-client
    pub flags:        u32,             // reserved unless explicitly assigned
}
```

Every vtable begins with these fields in this order. Loader reads
only the header before calling any function pointer.

> **AB3 (reshaped) — Append-only vtables require common header +
> size guard.** New vtable slots append after existing ones; loaders
> of older kernels read only `size_of_self >= offset + size_of(slot)`
> bytes. *Class*: runtime + ABI tests.

### 2.1 Load rules

```
1. read header bytes (sizeof(VtableHeader)) by mmap or symbol read
2. if cdylib.abi.major != kernel.abi.major   → reject IncompatibleAbi
3. if cdylib.abi.minor > kernel.abi.minor    → reject IncompatibleAbi
4. if cdylib.kind != expected (per manifest) → reject IllegalKind
5. read size_of_self; if < kernel-known minimum → reject ShortVtable
6. read appended slots only when size_of_self covers them
```

## 3. Error convention

```rust
#[repr(C, i32)]
pub enum ErrorCode {
    Ok                      = 0,
    Generic                 = 1,
    IncompatibleAbi         = 2,
    IncompatibleApi         = 3,
    NotFound                = 4,
    Conflict                = 5,
    InvalidArgument         = 6,
    ResourceExhausted       = 7,
    ProtocolViolation       = 8,
    Busy                    = 9,
    Stale                   = 10,
    PermissionDenied        = 11,
    Panic                   = 12,
    Cancelled               = 13,
    Timeout                 = 14,
    SchemaInvalid           = 15,
    NamespaceConflict       = 16,
    IllegalTrustClass       = 17,
    ConfigSliceTooLarge     = 18,
    Utf8Invalid             = 19,
    IllegalProjectHostSection = 20,
    IllegalKind             = 21,
    ShortVtable             = 22,
    CodecGone               = 23,
    NotActive               = 24,
    RollbackFailed          = 25,
    // ... future codes appended; values >= 240 reserved
}
```

For rich diagnostics, every fallible vtable slot accepts an
optional caller-owned `ErrorBuf`:

```rust
#[repr(C)]
pub struct ErrorBuf {
    pub data: *mut u8,
    pub cap:  usize,
    pub len:  usize,
}
```

The cdylib writes a UTF-8 message into `data[..cap]`, sets `len`,
and returns `ErrorCode`. Caller owns `data`.

## 4. Memory ownership

| Surface | Ownership rule |
|---|---|
| Caller-owned-for-call | Pointer valid only for the call duration. |
| Callee-owned-until-release | Pointer valid until `destroy_*` call; release fn declared next to type. |
| Copy-at-boundary | Bytes copied across the boundary; both sides own their copy. |

No pointer crosses without one of the three.

## 5. Panic isolation

> **AB12 — Single panic isolation rule.** Every host-to-cdylib and
> cdylib-to-host entrypoint wraps its body in `catch_unwind` and
> converts panic to `ErrorCode::Panic` plus a DS12 event
> (`dispatch.handler.panic`). No Rust unwind crosses an `extern "C"`
> boundary. Crates that host or call FFI thunks build with
> `panic = "unwind"`.
>
> *Class*: compile-time (panic profile) + runtime
> (`catch_unwind` coverage).

> **AB13 — Cleanup callbacks panic-contained.** Shutdown, destroy,
> drop, unregister callbacks all wrap. Panic during cleanup converts
> the owner to `TombstonedFailedUnload` and emits
> `cdylib.unload.fail rollback=failed`.
> *Class*: runtime.

### 5.1 Deprecated

`AB5` and `AB6` from v3 are deprecated; their bodies are subsumed by
`AB12`; the IDs are never recycled.

## 6. `ErrorCode` is the sole fallible-return convention

driver-ABI-v1 used `c_int` for fallible vtable slot returns. The
v0.15 implementation that shipped it is archived (non-normative);
no live cdylib consumes the `c_int` surface. v4 therefore makes the
clean break **before** an ecosystem exists, rather than carrying a
compatibility loader for zero external consumers.

> **AB14 — `ErrorCode` is the only fallible-return type at the ABI
> boundary.** Every fallible vtable slot and HostApi function
> returns `ErrorCode` (`#[repr(C, i32)]`). Bare `c_int` returns are
> forbidden in v4 vtables. There is no `c_int → ErrorCode`
> compatibility loader; driver-ABI-v1 is retired with the archived
> v0.15 implementation, and `archive/docs/architecture/driver-abi-v1.md`
> is heritage.
> *Class*: ABI / CI (signature grep over `uapi/` crates).

A cdylib returning an `i32` value outside the defined `ErrorCode`
discriminants is treated as `ErrorCode::Generic` and a DS12
`dispatch.handler.error` note records the raw value — misbehaving
cdylibs cannot smuggle undefined codes past observability.


## 6.1 The userspace covenant

> **AB15 — Do not break userspace. The released ABI is eternal.**
> From the first stable release (v1.0.0) onward, every surface
> declared stable MUST keep working for every artefact built
> against it, indefinitely:
>
> - **cdylib ABI** — a cdylib built against a shipped `AbiVersion`
>   major loads and runs on every later kernel. New ABI majors may
>   be *added*; support for a shipped major is never removed.
> - **Wire protocol** — shipped proto field numbers, message
>   shapes, and service signatures are never removed or reused.
>   A major protocol revision is a new service (`ReovimV2`)
>   served *alongside* the old one, not a replacement.
> - **Persisted state** — every shipped `state_version` remains
>   readable; restore migrates forward, never rejects.
> - **Config** — shipped schema vocabulary, layer semantics, and
>   the env-var grammar are append-only. A field may be deprecated
>   (WARN + migration shim); it is never removed or repurposed.
> - **Type catalog** — shipped `#[repr(C)]` layouts and enum
>   discriminants (including `ErrorCode` values) are frozen per
>   major, and shipped majors live forever.
> - **Library root** — shipped install-layout paths and lockfile
>   schema versions remain discoverable.
>
> Deprecation is permitted (documented, warned, observable via
> DS12). Removal is not. "Nobody uses it anymore" is not an
> argument the spec accepts — if it shipped stable, somebody's
> workflow depends on it.
>
> Pre-1.0 (`v0.x`) surfaces are explicitly exempt; that exemption
> is what licenses AB14's clean break. The exemption dies at
> v1.0.0 and never returns.
>
> *Class*: spec / CI (CF4 golden tests are the enforcement
> mechanism: golden artefacts for shipped surfaces are permanent
> and may only be added to, never edited or deleted).

This is the Linux kernel's first rule transplanted: internal
mechanisms (kernel structs, registries, lock tiers) may churn
freely between releases; the boundary userspace builds against
does not. Every other versioning rule in this chapter is
machinery; AB15 is the covenant that machinery serves.

## 7. Manifest kinds

```rust
#[repr(C, u8)]
pub enum ManifestKind {
    Unknown          = 0,   // reserved-invalid; loader rejects with IllegalKind
    ModuleServer     = 1,
    DriverServer     = 2,
    ModuleClient     = 3,
    DriverClient     = 4,
    CapabilityClient = 5,
    DomainServer     = 6,
    ProviderServer   = 7,
    StreamScheme     = 8,   // stream substrate schemes (4.4); own kind, not a driver sub-kind
}
```

This set is final for ABI major 1. Locked semantics:

- `Unknown = 0` is reserved-invalid. A vtable header carrying it
  is rejected at load with `IllegalKind`; it never means
  "forward-compatible unknown".
- Sub-kinds (driver `render` / `input` / `display` / `chrome`,
  capability `cell` / `dom` / ..., stream `pty` / `watch` / ...)
  are **not** ManifestKind variants. They discriminate by the
  per-vtable `kind` string in the manifest's `[[vtable]]` entries
  (CL6) and by the vtable symbol name. The enum stays small;
  adding a sub-kind is an `ApiVersion` minor bump, not an ABI
  change.
- Vtable-kind strings are stable for the lifetime of an ABI major.

The kind in the vtable header MUST match the manifest's `kind`
field; mismatch → `IllegalKind` at load (AB-conformance fixture).


## 8. Reshape notes

**Reshape note (AB3).** v3 AB3 required append-only vtables. v4
sharpens the mechanism: the common `VtableHeader` with
`size_of_self` is mandatory, and the loader's read rule (§2.1) is
normative. Intent unchanged; the guard is now structural.

## Open items

1. Whether `flags` on `VtableHeader` carries sub-kind selection
   hints in addition to the manifest `[[vtable]]` kind strings.
   Current default: no — `flags` stays reserved; the manifest is
   the only sub-kind authority.
2. Generated C header tooling.

## Conformance

| Rule | Fixture |
|---|---|
| AB3 | Append a slot to a sample vtable; older kernel reads through `size_of_self`. |
| AB12 | Panicking handler returns `ErrorCode::Panic`; process survives. |
| AB13 | Panicking shutdown → tombstoned-failed-unload event. |
| AB14 | CI grep: no `-> c_int` fallible slot in `uapi/` vtables. Runtime: out-of-range `i32` from a slot maps to `Generic` + DS12 note. |
| (ManifestKind) | Enum-discriminant golden test; `Unknown = 0` header rejected with `IllegalKind`. |
| (header layout) | Golden offset/size tests for `VtableHeader`, `AbiVersion`, `Version`, `ErrorBuf`. |
