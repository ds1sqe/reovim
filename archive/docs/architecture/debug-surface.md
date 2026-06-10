# Debug Surface Architecture

Status: Phase 0 + Phase 1 + Phase 2 landed (2026-04-24). Phase 3 (real
example driver) deferred behind #769 Phase 5. See the per-phase
entries in [`CHANGELOG.md`](../../CHANGELOG.md).

This document describes the debug-surface capability: a **driver-owned**
observability channel the CLI reaches through a bidirectional gRPC
stream. It is intended for contributors who want to add a debug
protocol to a driver, extend the transport, or understand the registry
and observer pump.

## 1. Overview

A driver cdylib implements the `ClientDebugSurface` trait and exports a
vtable. The host (the reovim server process in embedded mode) loads
the cdylib, wraps it in a `LoadedClientDebug` loader adapter, and
registers it with a `ClientDebugRegistry` in a composition root. A
remote inspector (CLI, test harness) opens a `DebugStream` bidi RPC
against the server, which routes the stream's `probe` / `observe` /
`drive` messages into the registry, which in turn drives the loaded
driver.

Two design invariants hold end-to-end:

1. **The transport is dumb.** Frame bodies, drive commands, and drive
   responses are opaque `bytes` at the proto layer. Only driver names
   and schema names are structured in the wire format. The CLI does
   not decode payloads.
2. **The driver owns the vocabulary.** Driver authors define both the
   schema names (listed in `DebugProbe`) and the byte-level meaning of
   each schema's traffic. Adding a new observe- or drive-schema never
   requires an RPC change or a CLI change.

This is the same philosophy as the client-render ABI (cdylib implements
a trait, host calls through a vtable) but applied to observability
rather than rendering.

## 2. Layer Map

```
┌─────────────────────────────────────────────────────────────────┐
│  INSPECTOR (CLI or test harness)                                │
│  clients/cli/src/commands/debug.rs                              │
│    probe / observe / drive  —  pure byte transport              │
├─────────────────────────────────────────────────────────────────┤
│  gRPC                                                           │
│  uapi/protocol/proto/reovim/v3/client_debug.proto               │
│    ClientDebugService::DebugStream (bidi-stream)                │
├─────────────────────────────────────────────────────────────────┤
│  HANDLER                                                        │
│  server/lib/server/src/grpc/client_debug.rs                     │
│    ClientDebugServiceImpl  —  state machine + observer pump     │
├─────────────────────────────────────────────────────────────────┤
│  REGISTRY                                                       │
│  server/lib/server/src/client_debug_registry.rs                 │
│    ClientDebugRegistry  —  name → Arc<Mutex<Box<dyn Driver>>>   │
│    DebugDriverHandle / DebugObserverHandle  (object-safe)       │
├─────────────────────────────────────────────────────────────────┤
│  LOADER                                                         │
│  clients/lib/subsys/driver-loader/src/client_debug.rs           │
│    LoadedClientDebug  —  dlopen + vtable validation + trampoline│
├─────────────────────────────────────────────────────────────────┤
│  ABI (FFI BOUNDARY)                                             │
│  REOVIM_CLIENT_DEBUG_DRIVER_VTABLE  —  #[repr(C)]               │
│  rc == -2 → panic                                               │
├─────────────────────────────────────────────────────────────────┤
│  DRIVER CDYLIB                                                  │
│  uapi/driver-macros:: declare_client_debug_driver!              │
│  impl ClientDebugSurface for YourDriver                         │
│  clients/lib/subsys/debug/src/client_debug.rs  (trait)          │
└─────────────────────────────────────────────────────────────────┘
```

Both sides of the ABI boundary share the subsys-tier trait definitions
in `clients/lib/subsys/debug/`. The loader links it via rlib; the
driver links it via rlib; the vtable marshals between them as opaque
`#[repr(C)]` function pointers.

## 3. Driver ABI (Summary)

The full portable contract is in
[`docs/architecture/driver-abi-v1.md`](./driver-abi-v1.md). The
client-debug specialization adds these constraints on top:

- **Exported symbol**: `REOVIM_CLIENT_DEBUG_DRIVER_VTABLE`. Exactly
  one per cdylib.
- **Trait surface**:
  [`ClientDebugSurface`](../../clients/lib/subsys/debug/src/client_debug.rs)
  — `probe()`, `construct()`, `observe()`, `drive()`, `shutdown()`.
  The observer trait is
  [`DebugObserver`](../../clients/lib/subsys/debug/src/observer.rs) —
  `next_frame()`.
- **`probe()` is static.** It returns a `DebugProbe` value without
  touching driver state. The host reads it pre-construct during the
  cdylib scan.
- **`observe(selector)` returns `Box<dyn DebugObserver + Send + '_>`.**
  The observer borrows `&mut self` of the driver, so two concurrent
  observers on one instance are a compile error at the trait level
  and a mutex hold at the registry level.
- **`drive(command) -> Vec<u8>`.** The response `Vec` is allocated by
  the driver; its buffer is returned to the host through the ABI's
  `destroy_bytes` slot so the host's allocator reclaims it.
- **Panic discipline**: every vtable trampoline wraps the driver
  closure in `catch_unwind`. A panic sets `rc == -2` and returns a
  stub value; the host surfaces this as `RegistryError::DriverPanicked`
  and evicts the driver. See §4 below.
- **Shrink before destroy**: drivers call `Vec::shrink_to_fit` before
  passing buffers across the ABI so the host's `destroy_bytes` slot
  sees the same `(ptr, len, cap)` triple the driver allocated.

A driver is declared with
[`declare_client_debug_driver!(MyDriver)`](../../uapi/driver-macros/src/client_debug.rs)
from
[`uapi/driver-macros/`](../../uapi/driver-macros/). The macro emits
the full trampoline set and the `#[unsafe(no_mangle)] pub static`
vtable.

## 4. Registry + State Machine

### 4.1 Registry

[`ClientDebugRegistry`](../../server/lib/server/src/client_debug_registry.rs)
is the server-side store. Its shape:

```
ClientDebugRegistry {
    drivers: RwLock<HashMap<String, Arc<AsyncMutex<Box<dyn DebugDriverHandle>>>>>,
}
```

- The outer `parking_lot::RwLock` guards the `HashMap`. Mutated only at
  composition-root setup and on panic eviction; the hot path is
  read-only lookup that clones the `Arc` and releases the read lock.
- Each driver entry is wrapped in `Arc<tokio::sync::Mutex<_>>`. One
  observe / drive / probe operation acquires the mutex and holds it
  for the operation's full duration.
- `DebugDriverHandle` and `DebugObserverHandle` are object-safe
  adapter traits. Both `LoadedClientDebug` (cdylib-backed) and in-tree
  test stubs implement them, so the registry can hold heterogeneous
  drivers in one map without a generic parameter.

### 4.2 Observer lifetime trick

`ClientDebugSurface::observe` returns
`Box<dyn DebugObserver + Send + '_>` — the observer borrows `&mut self`
of the driver for its lifetime. The registry needs to hand the
observer across an async task boundary, which requires a `'static`
type, without actually detaching the borrow.

The fix lives in
[`ClientDebugRegistry::observe`](../../server/lib/server/src/client_debug_registry.rs):
an `ObserverPump` struct keeps both the `OwnedMutexGuard<Box<dyn
DebugDriverHandle>>` and the observer `Box`, in that order, and
`std::mem::transmute`s the observer's `'_` to `'static`. Safety rests
on field-declaration order: Rust drops fields in declaration order,
so the observer is dropped before the guard, keeping the `&mut self`
borrow valid for the observer's entire lifetime. The `transmute` is
`#[allow(unsafe_code)]`-ed with a full SAFETY comment at the call
site.

### 4.3 gRPC-level state machine

[`run_stream`](../../server/lib/server/src/grpc/client_debug.rs) in the
handler implements a two-state machine per `DebugStream`:

| State | Accepts | Yields on unrecognized |
|---|---|---|
| `NotSelected` | `Select(driver_name)` | non-terminal `Error` frame |
| `Selected(name)` | `Select`, `ObserveStart`, `ObserveStop`, `DriveCmd` | non-terminal `Error` frame |

Transitions:

- `Select` cancels any active observer, fetches probe metadata, and
  transitions to `Selected(name)`. A second `Select` swaps drivers
  without closing the stream.
- `ObserveStart` spawns a `tokio::spawn(observer_pump)` task with a
  `oneshot::channel()` cancellation handle. The pump drains frames
  from the registry's `ObserverPump` and sends them to the outbound
  `mpsc::Sender`.
- `ObserveStop` fires the cancellation oneshot and awaits the pump
  task to completion, releasing the mutex guard.
- `DriveCmd` acquires the registry mutex, calls `drive()`, and sends
  one `DriveResp`.

### 4.4 Panic eviction

When any registry call returns `RegistryError::DriverPanicked` (rc ==
-2 from the FFI trampoline), the handler:

1. Drops the `ObserverPump` if one exists (releases the mutex guard
   before the outer write lock is taken).
2. Calls `registry.evict_on_panic(name)` — removes the entry from the
   outer `HashMap`.
3. Emits `Err(Status::internal("driver panicked"))` on the outbound
   channel and exits the stream.

The tonic outbound terminates with that `Status`, the client sees an
RPC error, and the driver is no longer reachable in the registry — a
subsequent `Select` for the same name surfaces `UnknownDriver` until
the composition root re-registers.

## 5. gRPC DebugStream

The wire definition is in
[`uapi/protocol/proto/reovim/v3/client_debug.proto`](../../uapi/protocol/proto/reovim/v3/client_debug.proto).

### 5.1 Client → server (`DebugStreamClientMsg.content` oneof)

| Variant | Field(s) | Meaning |
|---|---|---|
| `select` | `driver_name: string` | Select or re-select a driver. First message must be this variant. |
| `observe_start` | `schema: string` | Start an observer on a schema from the currently-selected driver. |
| `observe_stop` | — | Stop the active observer without closing the stream. |
| `drive_cmd` | `schema: string, body: bytes` | One-shot drive command. |

### 5.2 Server → client (`DebugStreamServerMsg.content` oneof)

| Variant | Field(s) | Meaning |
|---|---|---|
| `probe_resp` | `driver_name, description, observe_schemas[], drive_schemas[]` | Response to a successful `Select`. |
| `observe_frame` | `body: bytes` | A single observer frame from the active observer. |
| `drive_resp` | `body: bytes` | Response to the matching `DriveCmd`. |
| `error` | `message: string` | Non-terminal error. Stream stays open. |

### 5.3 Error vs terminal status

Three outcomes are possible:

- **Non-terminal error** — `DebugStreamError` frame. The stream stays
  open; the inspector can `Select` a different driver or retry.
  Produced by `UnknownDriver`, `RegistryError::DriverError` (bad
  selector or schema), and `RegistryError::Loader`.
- **Stream EOS** — server closes outbound without error. The
  inspector's stream returns `None` from `next()`. Produced when an
  observer naturally ends (`next_frame() == Ok(None)`) and the client
  has already half-closed.
- **Terminal `Status::internal`** — driver panicked. Stream closed,
  driver evicted.

The inspector must not assume that "the stream is open" implies
"driver is healthy" — only a `ProbeResp` following a `Select`
confirms a driver is reachable.

## 6. CLI Flow

[`clients/cli/src/commands/debug.rs`](../../clients/cli/src/commands/debug.rs)
owns the three verbs. Each verb is factored into two halves:

- `build_*_client_stream(...)` produces the outbound
  `impl Stream<Item = DebugStreamClientMsg> + Send + 'static`. No
  server, no gRPC — pure message construction.
- `consume_*_response_stream(stream, format)` consumes a stream of
  `Result<DebugStreamServerMsg, Status>` and returns the rendered
  output string. Takes `impl Stream` so unit tests can feed synthetic
  inputs without a tonic server.

The top-level `probe` / `observe` / `drive` functions compose the two
halves against the real `GrpcClient::debug_stream` wrapper. They are
the only functions that open the real RPC; everything else in the
module is exercised by unit tests.

For the `observe` verb, when `--count` is unset the CLI drops its
outbound sender after the initial `Select` + `ObserveStart`
handshake so the HTTP/2 outbound half-closes; the server's handler
then waits for the observer pump to naturally EOS before closing the
inbound. The drop sequence is documented in the module-level comment
at `clients/cli/src/commands/debug.rs` (module docs, §Testability
split plus the inline comment in `observe`).

The `--input SPEC` flag on `drive` accepts either an inline UTF-8
string or `@/abs/path` to read the file's raw bytes into the command
body. Binary payloads flow in verbatim; the CLI never inspects or
transforms them.

## 7. Extension Recipe

To add a client-debug surface to a driver of your own:

### 7.1 Declare the trait in the driver cdylib

```rust
use {
    reovim_client_subsys_debug::{
        client_debug::{ClientDebugSurface, DebugError, DebugProbe},
        observer::DebugObserver,
    },
    reovim_driver_macros::declare_client_debug_driver,
    std::ffi::c_void,
};

pub struct MyDriver { /* ... */ }

impl ClientDebugSurface for MyDriver {
    fn probe() -> DebugProbe {
        DebugProbe::new(
            "my-driver",
            "short human description",
            &["frames-v1"],        // observe schemas
            &["reset", "snapshot"], // drive schemas
        )
    }

    fn construct(_platform: *mut c_void) -> Result<Self, DebugError> {
        Ok(Self { /* ... */ })
    }

    fn observe(
        &mut self,
        selector: &[u8],
    ) -> Result<Box<dyn DebugObserver + Send + '_>, DebugError> {
        match selector {
            b"frames-v1" => Ok(Box::new(MyObserver::new(self))),
            other => Err(DebugError(format!(
                "unknown selector: {}",
                String::from_utf8_lossy(other),
            ))),
        }
    }

    fn drive(&mut self, command: &[u8]) -> Result<Vec<u8>, DebugError> {
        // opaque bytes in, opaque bytes out — driver-defined encoding
        Ok(command.to_vec())
    }

    fn shutdown(&mut self) -> Result<(), DebugError> { Ok(()) }
}

declare_client_debug_driver!(MyDriver);
```

### 7.2 Build the cdylib

The driver crate's `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
reovim-client-subsys-debug = { workspace = true }
reovim-driver-macros = { workspace = true }
```

`cargo build -p my-driver` produces
`target/debug/libmy_driver.so` (or `.dylib` / `.dll`).

### 7.3 Register in a composition root

A composition root is a bin (`apps/bin/*`) that wires a
`reovim-server` with a populated registry. As of Phase 1, no in-tree
bin does this — `Server::new(config)` installs an empty default
registry. A composition root that hosts debug drivers looks like
this (the same pattern used by the E2E harness at
[`clients/cli/tests/common.rs`](../../clients/cli/tests/common.rs)):

```rust
use {
    reovim_client_subsys_driver_loader::LoadedClientDebug,
    reovim_server::{ClientDebugRegistry, DebugDriverHandle, Server, ServerConfig},
    std::{path::Path, sync::Arc},
};

fn main() -> std::io::Result<()> {
    let registry = ClientDebugRegistry::new();

    // Load one or more cdylibs and register them by name.
    let driver = LoadedClientDebug::load_from_path(
        Path::new("/path/to/libmy_driver.so"),
    )
    .expect("load my-driver");
    registry.register("my-driver", Box::new(driver) as Box<dyn DebugDriverHandle>);

    let config = ServerConfig::grpc(12530);
    let server = Server::new(config)
        .with_client_debug_registry(Arc::new(registry));

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(server.run())
}
```

Key points:

- `ClientDebugRegistry::new()` is an empty registry. Call
  `register(name, driver)` for each cdylib before serving.
- `Server::new(config).with_client_debug_registry(Arc::new(registry))`
  replaces the empty default. The builder returns `Self` so it
  chains.
- `Box<dyn DebugDriverHandle>` lets you mix cdylib-backed and in-tree
  drivers in the same registry; the trait is in
  `reovim-server`'s public API.

### 7.4 Exercise it

From the inspector side (CLI, test harness, custom tool):

```
reovim cli --grpc 127.0.0.1:12530 debug probe --driver my-driver
reovim cli --grpc 127.0.0.1:12530 debug observe --driver my-driver --schema frames-v1
reovim cli --grpc 127.0.0.1:12530 debug drive --driver my-driver --schema snapshot --input @/tmp/req.bin
```

See [Debug Surface (CLI)](../user-guide/debug-surface.md) for the
end-user walkthrough.

## 8. Deferred Items

The following capabilities are intentionally not in the v1 debug
surface. They are tracked as follow-ups.

- **`pkg`-based no-arg `probe` listing.** The ability to run
  `reovim cli debug probe` (no `--driver`) and get a table of every
  installed driver's probe metadata requires the cdylib manifest
  index that is the subject of epic **#771**. Until that lands, the
  inspector must know a driver's registered name out-of-band.
- **Real example driver on TUI render.** #770 Phase 3 will add a
  `ClientDebugSurface` vtable to the TUI render driver cdylib,
  exposing frame-timing observation and a dump-buffer drive command.
  It is blocked on #769 Phase 5 (TUI render driver migration from
  in-tree library to runtime cdylib), because a non-cdylib driver has
  no place to host the second vtable.
- **Runbook / ops guide.** Production deployment (driver discovery
  path, authorization, rate-limits, per-driver isolation) is a
  separate story and is not covered by any #770 sub-plan.
