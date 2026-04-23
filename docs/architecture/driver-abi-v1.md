# Reovim Driver ABI — v1

Status: Phase 0 landed (2026-04-23). Applies to the client-render
driver trait; extends to server-side drivers in Phase 4 and additional
client-side driver kinds in Phase 5.

This document is the portable contract. The `uapi/driver-macros/` crate
is one Rust producer of conforming cdylibs; any implementation (Rust,
C, Zig, etc.) that matches this shape loads cleanly into a reovim host.

## 1. Overview

A **driver** is a platform- or backend-specific implementation of a
subsys-owned trait (for client-side: `ClientRender`, `ChromeDriver`,
etc.; for server-side: `NetDriver`, `TextSyntaxDriver`, etc.). Drivers
ship as runtime-loaded shared libraries (`.so` on Linux, `.dylib` on
macOS, `.dll` on Windows) resolved from the documented library root
(see Phase 1 sub-plan). The host reads an exported **vtable** — a
`#[repr(C)]` struct of function pointers and version fields — and
calls through it to construct and drive the concrete implementation.

The runtime-loaded model replaces compile-time linkage of driver
crates. See epic #769 for motivation.

## 2. Exported symbol

Every driver cdylib exports exactly one vtable symbol. Name
convention:

```
REOVIM_<KIND>_DRIVER_VTABLE
```

where `<KIND>` is an uppercased driver kind. For the client-render
driver:

```
REOVIM_CLIENT_RENDER_DRIVER_VTABLE
```

The symbol must be a `#[unsafe(no_mangle)] pub static` (Rust spelling)
declaration of the kind's vtable struct, placed in the cdylib's
read-only data segment. Consumers resolve it via
`libloading::Symbol<*const <Kind>VTable>`.

Only one driver-kind vtable per cdylib. A cdylib that exports zero or
multiple vtable symbols is rejected by the loader.

## 3. Vtable header

Every driver vtable, for any kind, **must** begin with the following
three fields in this order:

| Field | Type | Purpose |
|---|---|---|
| `abi_version` | `u32` | Layout epoch. Incremented on any binary change. |
| `api_version` | `Version` (12 bytes: three `u32`s) | Semantic version of the kind-specific trait contract. |
| `size_of_self` | `usize` | `mem::size_of::<KindVTable>()` observed by the driver build. |

The host validates these three fields by **pure memory reads**, before
dereferencing any function pointer. This is the key safety property:
a malformed cdylib cannot execute any code until the header check
passes.

Version policy:

- `abi_version` is **exact-match**. Any disagreement → load rejected.
- `api_version` is **semver-compatible**: same major, driver's minor
  must be `>=` host's required minor.
- `size_of_self` is **exact-match**. A mismatch indicates the driver
  was compiled against a different vtable layout (fields added, removed,
  or reordered).

## 4. Vtable body

After the header, the vtable carries kind-specific function pointers.
Each pointer is an `unsafe extern "C" fn` with C ABI parameters.

### 4.1 Lifecycle slots (every kind)

Every driver kind exposes at least:

- `probe() -> <Kind>DriverProbe` — pure metadata; callable without
  constructing the driver.
- `construct(platform, out_instance, out_err) -> c_int` — create an
  instance.
- `shutdown(instance, out_err) -> c_int` — drain outstanding work.
- `destroy(instance)` — free the instance. Infallible.
- `destroy_error_string(ptr)` — free a driver-allocated error string.
  See §6.

The concrete parameter lists and return signatures are defined
per-kind in the subsys crate that owns the trait.

### 4.2 Kind-specific slots (client-render example)

Beyond the lifecycle slots, the client-render vtable adds:

- `target(instance, out_vtable, out_handle, out_err) -> c_int` —
  borrow the driver's current `RenderTarget` as a sub-handle. See §7.

Each additional slot is a function pointer with its own out-param
convention per §6.

## 5. Version constants

Each driver kind's subsys crate publishes two `const` values:

```rust
pub const REOVIM_<KIND>_DRIVER_ABI_VERSION: u32 = 1;
pub const REOVIM_<KIND>_DRIVER_API_VERSION: Version = Version::new(1, 0, 0);
```

Drivers initialize their vtable's `abi_version` and `api_version`
fields from these constants (so rebuilding against a newer subsys
bumps the driver's reported versions automatically).

The host at runtime uses `Version::new(M, m, 0)` as the minimum
required API for the kind. The driver's reported `api_version.major`
must match; `api_version.minor` must be `>=` required.

## 6. Error convention

Fallible slots return `c_int`:

| Value | Meaning |
|---|---|
| `0` | Success. Out-params populated. |
| positive | Domain-specific soft signal (not used in v1). |
| `-1` | Error. `*out_err` is a driver-allocated null-terminated C-string. |
| `-2` | Panic caught at the FFI boundary (see §8). |
| `-3` | ABI-contract violation. |

When a slot returns `-1`:

1. The driver writes a pointer from `CString::into_raw` (or the C
   equivalent) into the `*mut *mut c_char` out-param.
2. The host reads the string, copies into an owned value, then calls
   the vtable's `destroy_error_string(ptr)` to free it.
3. The host **never** calls `libc::free` or any other free function
   directly; ownership round-trips through the driver's allocator via
   the `destroy_error_string` slot exactly once.

When a slot returns `-2`:

1. The out-param is undefined. The driver's trampoline may or may not
   have written to it.
2. If non-null, the host defensively routes the pointer through
   `destroy_error_string` to prevent leaks.

### 6.1 Memory ownership rules

- **Driver allocates, driver frees.** There is no shared allocator
  between host and driver. Every pointer that crosses the boundary
  from driver → host has a corresponding `destroy_*` slot on the
  driver side.
- **Host never frees driver pointers directly.** The host cannot use
  its own `libc::free`, `Box::from_raw`, or `CString::from_raw` on a
  driver-allocated pointer, because the driver may have been built
  against a different `libc` / allocator.
- **Round-trip is exactly once.** A pointer allocated by the driver
  MUST be returned to the driver's `destroy_*` slot exactly once. Use
  after free or double free is UB.

## 7. Sub-handle pattern

Some slots return a **borrowed trait object** — e.g.,
`ClientRender::target(&mut self) -> &mut dyn RenderTarget`. Rust's fat
pointer for `dyn Trait` is not repr(C). We carry the borrow as a
**(vtable, handle) pair** across the FFI boundary:

```
pub target: unsafe extern "C" fn(
    instance: *mut c_void,
    out_vtable: *mut *const RenderTargetVTable,
    out_handle: *mut *mut c_void,
    out_err: *mut *mut c_char,
) -> c_int;
```

- `*out_vtable` receives a pointer to a `#[repr(C)] RenderTargetVTable`
  — typically a static in the cdylib's `.rodata`.
- `*out_handle` receives an opaque `*mut c_void` whose meaning is
  defined by the driver. Commonly it aliases the driver instance
  itself; the submit trampoline casts it back to `*mut DriverType`.
- The sub-handle's lifetime is tied to the driver instance's lifetime.
  The host expresses this with a lifetime parameter on its safe
  wrapper: `LoadedRenderTarget<'a>` borrows `&'a mut
  LoadedClientRender`.

## 8. Panic isolation

FFI + `panic = "abort"` is undefined behaviour. The reovim workspace
locks `panic = "unwind"` for `[profile.dev]` and `[profile.release]`
in the root `Cargo.toml`, and CI greps `**/Cargo.toml` to reject any
`panic = "abort"` introductions.

Every `unsafe extern "C" fn` trampoline generated by
`declare_*_driver!` wraps its body in
`std::panic::catch_unwind(AssertUnwindSafe(|| ...))`:

- On `Ok(rv)` the trampoline returns `rv` (possibly mapped through an
  error allocator into `-1`).
- On `Err(_)` the trampoline returns `-2` and does not write to the
  out-params.

The host's safe wrapper translates `-2` into the kind's error type
(e.g. `LoadError::DriverPanicked`, `RenderError::InvalidData("driver
panicked at FFI boundary")`).

## 9. Probe metadata

The probe slot is pure data. It reports the driver's `kind` and
`name` as fixed-size byte arrays (null-terminated). Callers MUST NOT
construct the driver to read the probe — this enables fast scan of a
directory of cdylibs without paying per-driver construction cost.

For client-render:

```rust
#[repr(C)]
pub struct ClientRenderDriverProbe {
    pub kind: [u8; 64],   // null-terminated, max 63 bytes + nul
    pub name: [u8; 128],  // null-terminated, max 127 bytes + nul
}
```

## 10. Naming conventions for future kinds

When adding a new driver kind (`<kind>`):

1. Subsys crate `server/lib/subsys/<kind>/` or
   `clients/lib/subsys/<kind>/` publishes:
   - A Rust trait `<Kind>Driver` (the type-safe contract).
   - An `abi` module with `#[repr(C)] <Kind>DriverVTable` and any
     sub-vtables.
   - Two version constants:
     `REOVIM_<KIND>_DRIVER_ABI_VERSION: u32` and
     `REOVIM_<KIND>_DRIVER_API_VERSION: Version`.
2. `uapi/driver-macros/` adds a `declare_<kind>_driver!` proc-macro
   that emits the exported vtable static plus all trampolines.
3. `clients/lib/subsys/driver-loader/` (or the server equivalent) adds
   a `Loaded<Kind>` safe wrapper plus error variants.

The exported symbol name is `REOVIM_<KIND>_DRIVER_VTABLE`, uppercased.

## 11. Compatibility guarantees

- **Major-version bumps** to the API version (same kind's
  `REOVIM_<KIND>_DRIVER_API_VERSION`) break source compatibility for
  drivers. Drivers must be recompiled.
- **Minor-version bumps** add backwards-compatible slots (appended to
  the vtable). Old drivers keep loading against the new host because
  the host reads header + old slots only. New hosts refuse to call a
  new-minor slot on an old-minor driver.
- **ABI-version bumps** (`REOVIM_<KIND>_DRIVER_ABI_VERSION`) indicate
  a breaking binary-layout change (field reorder, type change).
  Drivers built against an older `abi_version` are rejected at load
  time by the header check.
- `size_of_self` mismatch is always a load failure. It catches the
  case where a driver adds a private field before recompiling.

## 12. Non-goals

- This ABI is not ratified as a public/stable third-party contract. It
  is reovim-internal; changes require a version bump and workspace
  rebuild.
- It does not specify threading or concurrency semantics beyond
  `Send + Sync` on Rust traits (the trait marker is enforced by the
  macro; FFI does not carry it).
- It does not provide a transport for Rust-specific types
  (`String`, `Vec`, `Box<dyn ...>`). Drivers must marshal to C-compatible
  shapes at the boundary.

## 13. Reference — client-render vtable layout

```rust
#[repr(C)]
pub struct ClientRenderVTable {
    pub abi_version:         u32,
    pub api_version:         Version,
    pub size_of_self:        usize,
    pub probe:               unsafe extern "C" fn() -> ClientRenderDriverProbe,
    pub construct:           unsafe extern "C" fn(
                                 platform: *mut c_void,
                                 out_instance: *mut *mut c_void,
                                 out_err: *mut *mut c_char,
                             ) -> c_int,
    pub target:              unsafe extern "C" fn(
                                 instance: *mut c_void,
                                 out_vtable: *mut *const RenderTargetVTable,
                                 out_handle: *mut *mut c_void,
                                 out_err: *mut *mut c_char,
                             ) -> c_int,
    pub shutdown:            unsafe extern "C" fn(
                                 instance: *mut c_void,
                                 out_err: *mut *mut c_char,
                             ) -> c_int,
    pub destroy:             unsafe extern "C" fn(instance: *mut c_void),
    pub destroy_error_string: unsafe extern "C" fn(ptr: *mut c_char),
}

#[repr(C)]
pub struct RenderTargetVTable {
    pub abi_version:  u32,
    pub size_of_self: usize,
    pub submit:       unsafe extern "C" fn(
                          handle: *mut c_void,
                          data: *const u8,
                          len: usize,
                          out_err: *mut *mut c_char,
                      ) -> c_int,
}
```
