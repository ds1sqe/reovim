# 2.1 — Kernel and Init Types

**Scope.** The shapes of `Init` (the boot actor) and `Kernel` (the
steady-state root), the registries, and the per-field locking
model. Concrete `#[repr(C)]` layouts that cross the ABI boundary
live in `06-ABI/03-Type-Catalog.md`; this chapter describes the
in-process Rust shapes.

**Heritage.** v3 `02-Process/01-Types.md`; v4 README §5. The single
struct was named `Pid1` until 2026-06; the rename split it into
the `Init`/`Kernel` pair because the state root has no process
lifetime or action of its own — the boot actor does.

**Locked rules.** None directly; references `CC*`, `LF*` (handoff:
LF13), `SVC*`.

---

## 1. Two types, one handoff

Boot and steady state are different types, not different flags:

- **`Init`** exists only during boot. It exclusively owns the
  kernel state under construction (`&mut self`, no locks), runs
  the boot stages (2.2 §1), and is *consumed* by the handoff.
- **`Kernel`** is the steady-state root. It is inert shared state —
  registries and maps behind the hostapi — with no run loop of its
  own. It can only be constructed by `Init::boot` (LF13).

```rust
impl Init {
    pub fn new(args: LauncherArgs) -> Init;                  // boot stage 0
    pub fn boot(mut self) -> Result<Arc<Kernel>, BootError>; // stages 1..6,
                                                             // then the move
}
```

On success, every retained field moves into `Kernel`, wrapped in
its per-field lock exactly once; boot-only state dies with `Init`
(the Linux `__init`-section analog). On failure, `Init` drops
whole — no partially-constructed kernel state escapes. The
boot/steady-state boundary is the type system, not a runtime stage
check.

## 2. `Init` shape

```rust
pub struct Init {
    args:            LauncherArgs,      // boot-only; dies with Init
    layers:          ConfigLayerStack,  // collapses into EffectiveConfig
    boot_anchor:     BootClock,         // monotonic zero + CLOCK_REALTIME
                                        // anchor, captured in new() (7.5 §4)
    lockfile:        Lockfile,
    inventory:       Inventory,         // plain mutable — no locks
    manifests:       ManifestRegistry,  // during boot
    domain_router:   DomainRouter,
    handlers:        HandlerRegistry,
    projectors:      ProjectorRegistry,
    services:        ServiceRegistry,
    stream_runtime:  StreamRuntime,
    event_bus:       Arc<DS12EventBus>, // log ring alive from stage 0 (LOG6)
    force_overrides: ForceOverrideMap,
}
```

Fields are private: nothing outside the boot path observes an
`Init`. The `event_bus` is already `Arc` because the log ring and
early-boot stderr (LOG8) are live from stage 0; it transfers into
`Kernel` unchanged, so timestamps and ring content are continuous
across the handoff.

## 3. `Kernel` shape

```rust
pub struct Kernel {
    pub abi:                Arc<KernelAbi>,
    pub config:             Arc<EffectiveConfig>,
    pub boot_anchor:        BootClock,
    pub lockfile:           Arc<Lockfile>,
    pub inventory:          Arc<RwLock<Inventory>>,

    pub sessions:           DashMap<SessionId, Arc<RwLock<Session>>>,
    pub buffers:            DashMap<BufferId, Arc<RwLock<Buffer>>>,
    pub windows:            DashMap<WindowId, Arc<RwLock<Window>>>,

    pub domain_router:      Arc<RwLock<DomainRouter>>,
    pub manifest_registry:  Arc<RwLock<ManifestRegistry>>,
    pub handler_registry:   Arc<RwLock<HandlerRegistry>>,
    pub projector_registry: Arc<RwLock<ProjectorRegistry>>,
    pub service_registry:   Arc<RwLock<ServiceRegistry>>,

    pub stream_runtime:     Arc<RwLock<StreamRuntime>>,
    pub event_bus:          Arc<DS12EventBus>,
    pub correlation_alloc:  Arc<CorrelationAllocator>,

    pub force_overrides:    Arc<ForceOverrideMap>,
}
```

`Kernel` is shared via `Arc<Kernel>`; per-field locks own
concurrency. There is no central `Mutex<Kernel>`. One `Init::boot`
run produces one kernel instance, regardless of OS process — the
embedded launcher (1.3) hosts a kernel instance and a client
runtime in the same process.

## 4. Registries

| Registry | Purpose | Owner of cdylib code? |
|---|---|---|
| `Inventory` | Per-`CdylibId` state, generation, manifest | yes |
| `DomainRouter` | `DomainId` interning, codec maps | yes (codec rows) |
| `ManifestRegistry` | Discovered manifests by kind/name | no (metadata only) |
| `HandlerRegistry` | Handler rows for dispatch | yes |
| `ProjectorRegistry` | Projector rows for dispatch | yes |
| `ServiceRegistry` | `ServiceDescriptor` rows | yes (drop_fn) |
| `StreamRuntime` | Stream substrate (S1..S10) | yes (per-scheme) |
| `force_overrides` | Force layer applied at boot | no |

Every registry row that holds a cdylib pointer carries
`owner_cdylib_id` (LF8). See per-registry chapters for row shapes.

Boot-time cdylib loads populate these registries through `Init`
under exclusive ownership; runtime load/unload operates on the
same registries through `Kernel` under the per-field locks. Same
rows, same LF rules — there is no separate boot loader (LF13).

## 5. Session shape

```rust
pub struct Session {
    pub id:           SessionId,
    pub buffers:      Vec<BufferId>,
    pub clients:      HashMap<ClientId, ClientView>,
    pub focus:        HashMap<(ClientId, BufferId, WindowId), Vec<FocusEntry>>,
    pub turn_gate:    Arc<TokioMutex<()>>,   // serialises dispatch order
    pub state:        Mutex<SessionState>,    // protects field set
}
```

The two-lock model (`turn_gate` + `state`) is the v4 fix to v3's
single-`Mutex<Session>`. See `02-Process/03-Concurrency.md`.

## 6. Buffer shape

```rust
pub struct Buffer {
    pub id:        BufferId,
    pub bytes:     Rope,
    pub domains:   Vec<DomainAttachment>,
    pub undo:      UndoStack,
    pub origin:    Origin,
    pub events:    BufferEventSubscribers,
}
```

(Detail in `04-Domain-Substrate/02-Domain-Tree.md` and
`04-Domain-Substrate/03-Undo.md`.)

## 7. Window shape

```rust
pub struct Window {
    pub id:        WindowId,
    pub buffer:    BufferId,
    pub layout:    LayoutNodeId,
    pub viewport:  ViewportCarrier,
}
```

(Detail in `05-View/01-Windows.md`.)

## Open items

1. Whether `Inventory` is per kernel instance or per OS process
   (relevant when the embedded launcher hosts a kernel instance
   alongside a client runtime in one process). Default: per
   instance.
2. `EffectiveConfig` lifetime — currently bound to the kernel
   instance's lifetime; if reload paths arrive
   (lifecycle="reloadable"), this becomes
   `ArcSwap<EffectiveConfig>`. Out of v4 target.
3. Whether `Session.state` is a `parking_lot::Mutex` or
   `tokio::sync::Mutex`. Implementation detail; spec only requires
   "non-async" lock for state and "async" gate for turn-ordering.

## Conformance

This chapter defines shapes only. The handoff rule and its
fixtures are LF13 (2.2). Per-registry rules carry their own
fixtures.
