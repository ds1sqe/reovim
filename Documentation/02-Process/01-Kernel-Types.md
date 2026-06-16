# 2.1 — Kernel and Init Types

**Scope.** The three-kernel model (§0), then the shapes of `Init`
(the boot actor) and `Kernel` (the steady-state root), the
registries, and the per-field locking model **for the editor
kernel**. Concrete `#[repr(C)]` layouts that cross the ABI boundary
live in `06-ABI/03-Type-Catalog.md`; this chapter describes the
in-process Rust shapes.

**Heritage.** The single struct was named `Pid1` until 2026-06; the
rename split it into the `Init`/`Kernel` pair because the state root
has no process lifetime or action of its own — the boot actor does.

**Locked rules.** None directly; references `CC*`, `LF*` (handoff:
LF13), `SVC*`.

---

## 0. The three kernels (system · editor · client)

A *kernel* = a mechanism core that loads policy over a stable ABI and
reaches its substrate through a single seam (`06-ABI/05-Platform-Contract.md`).
Reovim has three, each the same mechanism/policy/ABI fractal at a different
altitude:

| Kernel | Mechanism (WHAT) | Policy (its modules) | Substrate below | Corruption layer |
|---|---|---|---|---|
| **System** (machine) | sched, IRQ, memory, device model, block, fs, console, power | device drivers, board profiles | `arch/` (hardware) | World |
| **Editor** (server) | sessions, domains/buffer-algebra, undo-tree, streams, EventBus, state, services | server modules + drivers | platform contract | Math |
| **Client** | raw-input normalization, frame/cell render, projection/codec, capability slots, module host | client modules + drivers | platform contract **+** wire protocol | Math |

Each loads policy one-way (modules → api), exposes mechanism + api over a
closed subsys of trait contracts, and is extended by drivers that implement
those contracts — loaded, never linked (`02-Process/05-Machine-Boot.md`).

Two asymmetries are real, not incidental:

- **The system kernel is conditional.** RTOS-itself mode has all three;
  Over-OS mode has two and the host OS plays the system-kernel role
  (`01-Architecture/06-OS-Modes.md`). "Do we split the editor and the
  system?" reduces to "is the system-kernel slot filled?"
- **The client kernel has more substrates** — the platform contract, its
  input/output capabilities (`05-View/02-Raw-Input.md`,
  `05-View/03-Projections.md`), *and* the wire protocol. On bare metal the
  client's I/O bottoms out on the system kernel's devices, so the system
  kernel sits below **both** the editor and the client.

### 0.1 Sovereignty (the naming axis)

All three are kernels — each has the module/subsys/driver fractal and a
Math-or-World core. The distinction is **sovereignty**, not kernel-hood:

- **Sovereign kernels** own a *truth*: the **system kernel** (machine
  state) and the **editor kernel** (edit state).
- **Derived kernel:** the **client kernel** owns *no* truth — its
  Math-layer core renders the editor kernel's truth and normalizes input
  back. Same fractal, same Math layer, non-sovereign.

Crate names: system → `lib/machine-kernel`; editor → `server/lib/kernel`
(this chapter); client → `client-kernel`.

### 0.2 Editor / system split — by contract, not by privilege

The editor and system kernels are distinct **by contract**: the split *is*
the platform seam (`06-ABI/05-Platform-Contract.md`). What forces it (any
one suffices):

- **Invariance.** The editor kernel must stay platform-invariant. The
  moment it holds IRQ/board/device code it changes per target.
- **Corruption layer.** Editor kernel = **Math** (the formal-verification
  target); system kernel + `arch` = **World** (hardware-coupled,
  replaceable). Opposite sides of the airlock.

What the split is **not**, for 0.16:

- **Not a runtime / privilege boundary.** Single-seat unikernel appliance:
  one image, one cooperative tick loop, co-resident kernels, no MMU
  privilege wall. The boundary is a source contract, not an address space.
- **Not two schedulers.** The system kernel provides the *wake mechanism*
  (`park_until` + timer IRQ); the editor's `tick()` *is* the machine idle
  loop and consumes it. A separate preemptive machine scheduler appears
  only with SMP / multiple payloads — out of 0.16.

Discipline: **never let the editor kernel grow hardware or mode-varying
code "temporarily."** Anything that varies by what is underneath goes below
the contract — `arch`'s hosted role now, the system kernel later.

The remainder of this chapter details the **editor** kernel's `Init` /
`Kernel` shapes. The system and client kernels carry their own boot actors
of the same form.

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

> **Boot-core subset note.** The struct below is the spec target. The
> boot-core realization (#778 Phase 2) carries a documented subset —
> `abi`, the boot anchor, the event bus, the log ring — and grows
> monotonically toward this shape as later phases land their fields'
> consumers (registries, session/buffer/window maps, correlation
> allocator). Placeholder fields are not fabricated ahead of their
> consumers.

```rust
pub struct Kernel {
    pub abi:                Arc<KernelAbi>,
    pub config:             Arc<EffectiveConfig>,
    pub boot_anchor:        BootClock,
    pub lockfile:           Arc<Lockfile>,
    pub inventory:          Arc<RwLock<Inventory>>,

    pub sessions:           ShardedMap<SessionId, Arc<RwLock<Session>>>,
    pub buffers:            ShardedMap<BufferId, Arc<RwLock<Buffer>>>,
    pub windows:            ShardedMap<WindowId, Arc<RwLock<Window>>>,

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

> Note (non-normative): `ShardedMap<K, V>` is an in-repo sharded
> concurrent map realized as a fixed array of `arch/`-provided
> RwLock-guarded hash-map shards (1.2 §10 platform floor); `K`
> hashes to select the shard.

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
    pub turn_gate:    Arc<TurnGate>,         // serialises dispatch order (FIFO)
    pub state:        Mutex<SessionState>,   // protects field set
}
```

The two-lock model (`turn_gate` + `state`) replaces the prior
single-`Mutex<Session>`. See `02-Process/03-Concurrency.md`.

> Note (non-normative): `TurnGate` is an in-repo FIFO fair-queue
> primitive built over `arch/`-provided sync primitives (condvar +
> ticket counter); it provides the same acquire/release contract as
> a FIFO async mutex without a third-party runtime dependency.

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
   (lifecycle="reloadable"), this becomes an atomically-swappable
   shared reference (`arch/`-provided; no third-party `ArcSwap`).
   Out of target.
3. ~~Whether `Session.state` is an async or a non-async mutex (the
   pre-sovereignty draft weighed third-party primitives)~~ —
   resolved #782 as `std::sync::Mutex`; re-resolved #784 under
   `DAG6` (1.2 §10): `Session.state` is the `arch/`-provided Mutex;
   FIFO turn-ordering is provided by the in-repo `TurnGate` built
   over `arch/` sync primitives.

## Conformance

This chapter defines shapes only. The handoff rule and its
fixtures are LF13 (2.2). Per-registry rules carry their own
fixtures.
