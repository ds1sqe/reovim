# Reovim Driver ABI — v1

Status: #769 Phases 0–6 landed (2026-04-25). The v1 spec covers the
`client_render` driver kind in full (Phase 0 PoC validated; macro +
vtable infrastructure operational). Server-side driver kinds and
additional client-side driver kinds are described in this document but
their cdylib migrations are deferred: trait redesigns for FFI-routability
are tracked at #774; the concrete `ClientRender` implementor is tracked
at #753 client-foundation resumption. The spec is reimplementer-ready
independent of which driver kinds have shipped cdylib implementations.

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

## 1b. Driver kinds covered by v1

This section records which driver kinds the v1 spec describes the binary
contract for, and which have shipped cdylib implementations.

| Kind | Spec coverage | cdylib shipped? | Tracker |
|---|---|---|---|
| `client_render` | Full — vtable shape, version constants, PoC macro at `uapi/driver-macros/src/client_render.rs`, ABI types at `clients/lib/subsys/render/src/abi.rs` | No (concrete implementor deferred) | #753 |
| `net_grpc` (server) | Full — vtable layout in §13.5, ABI types at `server/lib/subsys/net/src/abi.rs`, host loader at `server/lib/subsys/driver-loader/`, codegen macro at `uapi/driver-macros/src/net_grpc.rs` | Yes | landed in #774 sub-plan 02 |
| `command` (server) | Vtable shape described in §4; trait redesign pending | No | #774 |
| `text_session` (server) | Vtable shape described in §4; trait redesign pending | No | #774 |
| `text_syntax` (server) | Vtable shape described in §4; trait redesign pending | No | #774 |
| `text_input` (server) | Vtable shape described in §4; trait redesign pending | No | #774 |
| `buffer` (server) | Full — vtable layout in §13.6, ABI types at `server/lib/subsys/buffer/src/abi.rs`, host loader at `server/lib/subsys/driver-loader/src/buffer.rs`, codegen macro at `uapi/driver-macros/src/buffer.rs` | Yes | landed in #774 sub-plan 03 |
| `chrome_surface` family (client) | Vtable shape described in §4; trait redesign pending | No | #774 |
| `display` (client) | Vtable shape described in §4; trait redesign pending | No | #774 |

The empty cdylib-migration column for v0.16.0 is by design: #769 delivers
the binary contract, the macro framework, the loader infrastructure, and the
build pipeline. #774 fills the migrated set against this foundation.

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
  **Must be idempotent.** Host safe-wrappers call `shutdown` from `Drop`
  unconditionally regardless of prior shutdown state; a driver whose
  serve loop already shut down internally must treat repeat invocations
  as a no-op success (return 0).
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

## 13.5. Reference — net-grpc vtable layout

The net-grpc server driver inverts tonic's Router construction. The
host builds a `Vec<ServiceDescriptor>` (one per gRPC service) and
hands it to the driver; the driver builds the tonic Router inside its
own runtime and serves until the host signals shutdown via a
file-descriptor.

```rust
#[repr(C)]
pub struct NetGrpcVTable {
    pub abi_version:         u32,
    pub api_version:         Version,
    pub size_of_self:        usize,
    pub probe:               unsafe extern "C" fn() -> NetGrpcDriverProbe,
    pub construct:           unsafe extern "C" fn(
                                 out_instance: *mut *mut c_void,
                                 out_err:      *mut *mut c_char,
                             ) -> c_int,
    pub serve:               unsafe extern "C" fn(
                                 instance:        *mut c_void,
                                 config:          *const FfiTransportConfig,
                                 descriptors_ptr: *const FfiServiceDescriptor,
                                 descriptors_len: usize,
                                 shutdown_fd:     ShutdownFd,
                                 bind_ready_fd:   ShutdownFd,
                                 port_writeback:  *const AtomicU16,
                                 out_err:         *mut *mut c_char,
                             ) -> c_int,
    pub shutdown:            unsafe extern "C" fn(
                                 instance: *mut c_void,
                                 out_err:  *mut *mut c_char,
                             ) -> c_int,
    pub destroy:             unsafe extern "C" fn(instance: *mut c_void),
    pub destroy_error_string: unsafe extern "C" fn(ptr: *mut c_char),
}

#[repr(C)]
pub struct NetGrpcDriverProbe {
    pub kind: [u8; 64],   // b"net_grpc\0..."
    pub name: [u8; 128],  // human-readable (e.g. b"reovim-driver-net-grpc\0...")
}

#[repr(C)]
pub struct FfiTransportConfig {
    pub kind:            u8,            // 0=Tcp, 1=UnixSocket, 2=Stdio (rejected)
    pub host_ptr:        *const c_char, // populated when kind == 0
    pub host_len:        usize,
    pub port:            u16,
    pub path_ptr:        *const c_char, // populated when kind == 1
    pub path_len:        usize,
    pub enable_grpc_web: u8,            // 0|1
}

#[repr(C)]
pub struct FfiServiceDescriptor {
    pub service_name_ptr: *const c_char, // diagnostic only
    pub service_name_len: usize,
    pub handle:           *mut c_void,   // host-owned BoxCloneService
    pub dispatch_call:    unsafe extern "C" fn(
                              handle:      *mut c_void,
                              req_bytes:   *const u8,
                              req_len:     usize,
                              ctx:         *mut c_void,
                              complete_cb: unsafe extern "C" fn(
                                               ctx:        *mut c_void,
                                               status:     c_int,
                                               resp_bytes: *const u8,
                                               resp_len:   usize,
                                           ),
                          ) -> c_int,
    pub destroy_handle:   unsafe extern "C" fn(handle: *mut c_void),
}

#[repr(transparent)]
pub struct ShutdownFd(pub RawFd);
```

**Driver-side typed wrappers**: `tonic::transport::Server::add_service`
requires a type-level `NamedService` bound (`const NAME: &'static str`).
Per-instance runtime names cannot satisfy this; the driver therefore
generates one typed proxy wrapper per stable `reovim.v3.<X>Service`
proto identifier (12 today). See `uapi/driver-macros/src/net_grpc.rs`
for the codegen and `server/lib/subsys/net/src/service_descriptor.rs`
for the host-side shape.

**Runtime ownership**: the driver creates and owns its own
`tokio::runtime::Runtime` (multi-thread). This sidesteps cross-cdylib
TLS coupling on `Handle::current()`. The host invokes `serve` from a
worker thread (via `spawn_blocking` from the host runtime) and the
driver `block_on`s its own runtime internally.

**Shutdown**: the host writes one byte to the writable end of the
shutdown pipe; the driver wraps the readable end in
`tokio::io::unix::AsyncFd` and triggers graceful shutdown when the fd
becomes readable. Ownership stays with the host — the driver does not
close the fd on drop.

**Bind-ready signal**: symmetric to shutdown. The driver writes one
byte to the host-owned `bind_ready_fd` after the listener is bound and
the bound port has been written to `port_writeback`. The host-side
port-reporter polls the AtomicU16 once `bind_ready_fd` is readable.

**Port writeback**: the host allocates an `AtomicU16` via
`Box::leak(Box::new(AtomicU16::new(0)))` (single 2-byte
program-lifetime leak) and passes the raw pointer through the
trampoline; the driver casts it back to `&'static AtomicU16` and stores
the bound port with `Ordering::Release`.

## 13.6. Reference — buffer vtable layout

The buffer server driver factories `Arc<dyn Buffer>` instances over
runtime-loaded byte storage. The contract surface is the closed
`subsys-buffer::Buffer` trait (per-instance) plus the
`subsys-buffer::BufferDriver` trait (factory + registry). The cdylib
exports a 27-slot vtable covering the lifecycle, factory, per-buffer
methods, multi-codec attachment slots, async edit subscription, and
allocator-hygiene destructors.

```rust
#[repr(C)]
pub struct BufferVTable {
    pub abi_version:               u32,
    pub api_version:               Version,
    pub size_of_self:              usize,
    pub probe:                     unsafe extern "C" fn() -> BufferDriverProbe,
    pub construct:                 unsafe extern "C" fn(
                                       out_instance: *mut *mut c_void,
                                       out_err:      *mut *mut c_char,
                                   ) -> c_int,
    // BufferDriver factory surface.
    pub create_buffer:             unsafe extern "C" fn(/* … */) -> c_int,
    pub open_buffer:               unsafe extern "C" fn(/* … */) -> c_int,
    pub list_buffers:              unsafe extern "C" fn(/* … */) -> c_int,
    pub get_buffer:                unsafe extern "C" fn(/* … */) -> c_int,
    pub close_buffer:              unsafe extern "C" fn(/* … */) -> c_int,
    pub destroy_id_list:           unsafe extern "C" fn(*mut u64, usize),
    // Per-Buffer surface (handle = *mut c_void from create/open/get).
    pub buffer_id:                 unsafe extern "C" fn(*mut c_void) -> u64,
    pub buffer_size:               unsafe extern "C" fn(*mut c_void) -> usize,
    pub buffer_is_modified:        unsafe extern "C" fn(*mut c_void) -> c_int,
    pub buffer_file_path:          unsafe extern "C" fn(/* … */),
    pub buffer_set_file_path:      unsafe extern "C" fn(/* … */) -> c_int,
    pub buffer_read_bytes:         unsafe extern "C" fn(/* … */) -> c_int,
    pub buffer_apply_edit:         unsafe extern "C" fn(/* … */) -> c_int,
    pub buffer_write_to:           unsafe extern "C" fn(/* … */) -> c_int,
    // Multi-codec attachment slots.
    pub buffer_attach_codec:       unsafe extern "C" fn(/* … */) -> c_int,
    pub buffer_detach_codec:       unsafe extern "C" fn(/* … */) -> c_int,
    pub buffer_list_codecs:        unsafe extern "C" fn(/* … */) -> c_int,
    // Async edit subscription (forward-compat slots).
    pub buffer_subscribe_edits:    unsafe extern "C" fn(/* … */) -> c_int,
    pub buffer_unsubscribe_edits:  unsafe extern "C" fn(/* … */) -> c_int,
    pub buffer_destroy:            unsafe extern "C" fn(*mut c_void),
    // Driver lifecycle close-out.
    pub destroy:                   unsafe extern "C" fn(*mut c_void),
    pub destroy_error_string:      unsafe extern "C" fn(*mut c_char),
    pub destroy_byte_buffer:       unsafe extern "C" fn(*mut u8, usize),
    pub destroy_file_path:         unsafe extern "C" fn(*mut c_char, usize),
    pub destroy_codec_name_array:  unsafe extern "C" fn(*mut FfiCodecName, usize),
}

#[repr(C)]
pub struct BufferDriverProbe {
    pub kind: [u8; 64],   // b"buffer\0..."  (must equal "buffer")
    pub name: [u8; 128],  // human-readable (e.g. b"reovim-driver-text-buffer\0...")
}
```

See `server/lib/subsys/buffer/src/abi.rs` for the full slot signatures
and ABI types (`FfiByteEdit`, `FfiCodecAttachment`, `FfiCodecName`).

**Buffer handle representation**: each `*mut c_void` returned by
`create_buffer` / `open_buffer` / `get_buffer` is a thin pointer to a
boxed `Arc<dyn Buffer>` allocated by the cdylib. The host releases the
handle by calling `buffer_destroy(handle)`; the cdylib reboxes and
drops, releasing one Arc refcount.

**Codec attachment side table**: `buffer_attach_codec` accepts a
host-allocated `FfiCodecAttachment` and the cdylib stores its fields in
a process-global side table keyed by `(buffer_addr, slot_id)`.
`buffer_detach_codec` looks up the slot, `mem::forget`s the
cdylib-side adapter (suppressing the cdylib's `destroy_handle` call),
and writes the original attachment back to the host so the host owns
the destruction. `buffer_destroy` sweeps the side table for any
still-live entries belonging to the dying buffer.

**Synchronous fan-out**: `buffer_apply_edit` mutates canonical bytes
then iterates every attached codec slot in `CodecAttachmentId` order,
calling each codec's `notify` synchronously inside the buffer's lock.
Each per-slot call is wrapped in `catch_unwind`; a panicked slot is
left in-place (subsys contract: panicked slots are not detached
mid-iteration). The host's `notify` callback may be invoked from any
thread the driver chooses.

**Allocator hygiene**: every host-facing heap array (`u8[]` from
`buffer_read_bytes`, `u64[]` from `list_buffers`, `FfiCodecName[]` from
`buffer_list_codecs`, file-path `c_char[]` from `buffer_file_path`,
err `c_char` from any slot) MUST be freed via the matching `destroy_*`
slot in this same vtable. Do not call `free()` directly from the host;
the cdylib's allocator and the host's may differ.

**Async edit subscription**: `buffer_subscribe_edits` and
`buffer_unsubscribe_edits` are forward-compat ABI slots reserved for
the `BufferSubscribable` trait. The current cdylib stub returns
`out_subscription_id = 0` and immediately calls the host's
`subscriber_destroy`. Wiring will land when async consumers (LSP,
file-watcher, debug surfaces) are introduced.

## Library-root discovery (Phase 1)

Runtime discovery of driver and module cdylibs uses the
`reovim-dylib-loader` crate's six-rule `PathResolver` with MERGE
semantics (never replace):

| # | Source                                              | Precedence |
|---|-----------------------------------------------------|------------|
| R1 | CLI `--driver <path>` (drivers only)               | highest    |
| R2 | CLI `--module <path>` / `-m` / `--lib` (modules)   | highest    |
| R3 | env `REOVIM_DRIVER_PATH` (colon-separated)         | middle     |
| R4 | env `REOVIM_MODULE_PATH` (colon-separated)         | middle     |
| R5 | env `REOVIM_LIBRARY_ROOT/<kind>/`                  | lower      |
| R6 | XDG data + `/usr/local/lib/reovim/<kind>/` + `/usr/lib/reovim/<kind>/` | lowest |

A path in a higher-precedence rule is searched first but does not
evict lower-precedence entries. Empty colon-separated segments are
filtered; an unset env var contributes zero entries. The two
specialized subsys loaders (`clients/lib/subsys/driver-loader/` and
`server/lib/subsys/module-loader/`) expose `from_path_scan(root)`
constructors that walk `<root>/{driver,modules}/` via this resolver
and return one per-entry result per candidate cdylib.

See `lib/dylib-loader/` for the mechanism and the
`02-loader-and-discovery.md` sub-plan under `~/docs/plans/reovim/769-abi-foundation/`
for the design rationale and O2–O4 resolutions.

## Bin layout (Phase 2a)

Phase 2a of #769 split the legacy `apps/bin/` monolith into five
composition-root bin crates under `apps/`, so each user-facing bin
owns exactly its subset of the driver/module stack:

| Bin          | Crate path       | Package name           | Role                                                                    |
|--------------|------------------|------------------------|-------------------------------------------------------------------------|
| `reovim-server` | `apps/server/` | `reovim-app-server`    | Server runtime composition root. Owns bootstrap, module loader, drivers.|
| `reovim-tui`    | `apps/tui/`    | `reovim-app-tui`       | Thin TUI launcher; delegates to `ext/client/platforms/tui::run`.        |
| `reovim-cli`    | `apps/cli/`    | `reovim-app-cli`       | One-shot gRPC CLI client (wraps `reovim-client-cli`).                   |
| `reovim-web`    | `apps/web/`    | `reovim-app-web`       | Web UI scaffold; SSR strategy lands in Phase 5 (O6).                    |
| `reovim`        | `apps/reovim/` | `reovim-app-launcher`  | Top-level launcher. Subprocess-only at 2a; Phase 2b adds embedded mode. |

**Launcher vs standalone.** At Phase 2a the `reovim` launcher
(`apps/reovim/`) is a pure subprocess passthrough: `reovim server …`
execs `reovim-server`, `reovim tui …` execs `reovim-tui`, and so on.
It declares zero `reovim-*` workspace deps (enforced by
`apps_reovim_launcher_scope.rs`). Phase 2b per locked decision L9
grows this crate into a dual-mode composition root — embedded default
plus subprocess fallback plus four transport variants — at which
point the `apps_reovim_launcher_scope.rs` probe relaxes to allow the
embedded-mode library edges into `apps/{server,tui,cli,web}`.

**Cargo install.** `cargo install reovim` at Phase 2a requires
`reovim-server` and `reovim-tui` to also be on `$PATH`; the launcher
spawns them as subprocesses. Phase 2b restores the single-bin OOB
behavior. See CHANGELOG for the transitional note.

**Depgraph guards.** Every bin carries a dedicated probe in
`lib/depgraph/tests/`: `apps_server_scope.rs`, `apps_tui_scope.rs`,
`apps_cli_scope.rs`, `apps_web_scope.rs`, and
`apps_reovim_launcher_scope.rs`. A global
`apps_standalone_bin_isolation.rs` forbids `apps/*` cross-deps at 2a
(empty allowlist); Phase 2b edits the allowlist to admit the embedded
launcher's library edges.

## Launcher composition (Phase 2b)

Phase 2b (locked decision L9) turns `apps/reovim/` into a dual-mode
composition root. The launcher owns composition policy; the server
and client runtimes own mechanism. One clap CLI, one
`reovim_app_launcher::run` entry point, two composition strategies,
four transport kinds.

### Mechanism vs policy

- **Mechanism** — what wire protocol connects server and client:
  `inproc` (tonic over an in-process `DuplexStream`), `uds` (tonic
  over a Unix-domain socket), `tcp` (tonic over `127.0.0.1:port`),
  or `pipe` (tonic over an OS pipe; server- and client-side both
  still placeholder — see status marker in the matrix below).
- **Policy** — how the launcher assembles server and client:
  *embedded* boots both in one process (default, `--transport
  inproc`); *subprocess* forks sibling bins over a real kernel
  transport (`--subprocess`); *external-grpc* forks only the client
  and points it at an already-running server (`--external-grpc
  HOST:PORT`, implies `--transport tcp`).

### Transport matrix

The seven supported `(launch-mode, transport)` pairs and the two
invalid-pair categories:

| Launch mode     | `inproc` | `pipe`      | `uds`    | `tcp`  |
|-----------------|----------|-------------|----------|--------|
| Embedded        | ✓ live   | placeholder | ✓ live   | ✓ live |
| Subprocess      | rejected | placeholder | ✓ live   | ✓ live |
| External-grpc   | rejected | rejected    | rejected | ✓ live |

- **live** — wired end to end; `/e2e` covers the keystroke→frame
  roundtrip.
- **placeholder** — CLI accepts the flag and the launcher falls into
  an `Unsupported` return with a pointer comment. Server-side
  `Server::run_pipe` and the client-side tonic-over-pipe connector
  are follow-on work; the matrix reflects the code you see today,
  not what a release artifact would enforce.
- **rejected** — semantically invalid. `TransportChoice::resolve`
  emits a typed error (`InprocRequiresEmbedded` or
  `ExternalGrpcRequiresTcp`) before the launcher opens any I/O.

### Crate layout

```
apps/reovim/src/
├── lib.rs                — pub fn run(cli: Cli) -> io::Result<()>
├── subprocess.rs         — clap CLI surface: Cli, Cmd, ClientKind
├── subprocess_compose.rs — subprocess orchestrator: spawn roles + SIGINT fan-out
├── embedded.rs           — embedded composition: run_inproc / run_uds / run_tcp
├── lifecycle.rs          — ShutdownCoord (tokio broadcast wrapper)
└── transport.rs          — LaunchMode, TransportKind, TransportChoice, resolve()
```

- `subprocess.rs` is purely the **CLI surface** — clap types (`Cli`,
  `Cmd`, `ClientKind`) and the passthrough subcommand dispatch
  (`reovim tui|cli|...` → `Command::new("reovim-<kind>")`).
- `subprocess_compose.rs` is the **subprocess orchestrator** — it
  spawns `reovim-server` plus the selected client bin and forwards
  SIGINT to each child in reverse spawn order (client first, server
  second), escalating to `Child::kill()` after a 5s grace window.
- `embedded.rs` wires the single-process composition: one tokio
  runtime hosts the server on a background task and the client on
  the main task; `Server::shutdown` drains the server when the
  client returns, with a 2s abort fallback
  (`SHUTDOWN_DRAIN_TIMEOUT`) for tonic connections that cannot
  converge.
- `lifecycle.rs` holds `ShutdownCoord`, a `tokio::sync::broadcast`
  wrapper that fans ctrl-c out to the server task.
- `transport.rs` holds the composition-level validation rules —
  clap admits every `(launch-mode, transport)` pair at the parse
  stage; `TransportChoice::resolve` rejects the semantically
  invalid combinations with typed errors so the launcher fails
  before any I/O.

### Embedded default and `cargo install`

`cargo install reovim` produces a single binary that boots the
embedded TUI on `inproc` by default — no sibling bins required on
`$PATH`. This restores the OOB behavior broken at Phase 2a.
Downstream packagers who prefer split bins can disable the
`embedded-tui` default feature to get a subprocess-only launcher
(the Phase 2a shape). The `embedded-cli` and `embedded-web`
features are scaffold-only and light up alongside the one-shot CLI
and SSR web paths.

### Verification

- `apps/reovim/tests/transport_matrix.rs` — enumerates the 7 valid
  pairs and the invalid-pair rejections; the 4 rejections run
  in-process; the 7 roundtrips are `#[ignore]`'d pending the `/e2e`
  harness.
- `apps/reovim/tests/embedded_smoke.rs` — asserts the lifecycle
  sequencing invariant (client exits, `Server::shutdown` fires,
  server task joins) with a mock client future.
- `apps/reovim/tests/subprocess_smoke.rs` — `#[ignore]`'d live-process
  SIGINT smoke pending the `/e2e` harness.
- `scripts/cross-install-check.sh` — `cargo install --path
  apps/reovim --root tmp/2b-install-check` then probes `--version`
  and `--help` under a sanitised `PATH`. Validates master-plan
  acceptance #6.
