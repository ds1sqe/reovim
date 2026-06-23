# 2.1 — Editor Core and EditorInit Types

**Scope.** The one-kernel-plus-cores model (§0), then the shapes of
`EditorInit` (the boot actor) and `EditorCore` (the steady-state root), the
registries, and the per-field locking model **for the editor core**. Concrete
`#[repr(C)]` layouts that cross the ABI boundary
live in `06-ABI/03-Type-Catalog.md`; this chapter describes the
in-process Rust shapes.

**Heritage.** The single struct was named `Pid1` until 2026-06; the
rename split it into the `EditorInit`/`EditorCore` pair because the state root
has no process lifetime or action of its own — the boot actor does.

**Locked rules.** None directly; references `CC*`, `LF*` (handoff:
LF13), `SVC*`.

---

## 0. One kernel, two product cores

New architecture prose reserves unqualified **kernel** for the World-layer
system kernel. The product-facing Math mechanisms are **cores**:

| Component | Mechanism (WHAT) | Policy (its modules) | Substrate below | Corruption layer |
|---|---|---|---|---|
| **System kernel** | sched, IRQ, memory, device model, block, fs, console, power; bridge from `uapi/*` to `kabi/*` | system drivers, board profiles | `kabi/*` providers + `arch/` hardware below them | World |
| **Editor core** | sessions, domains/buffer-algebra, undo-tree, streams, EventBus, state, services | server modules + drivers | `uapi/*` system surface | Math |
| **Client core** | raw-input normalization, frame/cell render, projection/codec, capability slots, module host | client modules + drivers | `uapi/*` system surface **+** wire protocol | Math |

Each follows the same mechanism/policy/ABI fractal: policy flows one-way into
a mechanism surface, contracts are stable, and drivers/modules are loaded or
registered through closed seams rather than imported directly. The word
"kernel" is no longer used for all three because `system/lib/kernel` is now a
real crate and the ambiguity is operationally expensive.

Two asymmetries are real, not incidental:

- **The full system-kernel role is conditional.** RTOS-itself mode fills the
  role inside Reovim. Over-OS mode uses the same bridge vocabulary but the
  host OS provides most system services below it
  (`01-Architecture/06-OS-Modes.md`). "Do we split the editor and the
  system?" reduces to "is the full system role filled by Reovim?"
- **The client core has more substrates** — the up-face system surface, its
  input/output capabilities (`05-View/02-Raw-Input.md`,
  `05-View/03-Projections.md`), *and* the wire protocol. On bare metal the
  client's I/O bottoms out on system-kernel devices, so the system kernel sits
  below **both** the editor core and the client core.

### 0.1 Sovereignty

The distinction is now **sovereignty**, not shared kernel-hood:

- **Sovereign components** own a *truth*: the **system kernel** owns machine
  state; the **editor core** owns edit state.
- **Derived component:** the **client core** owns *no* truth — it renders the
  editor core's truth and normalizes input back. Same fractal, same Math
  layer, non-sovereign.

Crate names: system → `system/lib/kernel`; editor core →
`editor/lib/core` / `reovim-editor-core`; client core path is not hardened yet.

### 0.2 Editor / system split — by contract, not by privilege

The editor core and system kernel are distinct **by contract**: the editor
core stays on the `uapi/*` up-face, while the system kernel bridges `uapi/*` to the
`kabi/*` down-face (`06-ABI/05-Platform-Contract.md`). What forces it (any
one suffices):

- **Invariance.** The editor core must stay platform-invariant. The
  moment it holds IRQ/board/device code it changes per target.
- **Corruption layer.** Editor core = **Math** (the formal-verification
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

Discipline: **never let the editor core grow hardware or mode-varying
code "temporarily."** Anything that varies by what is underneath goes below
the up-face — into the hosted system role or into `system/lib/kernel` plus
providers in RTOS-itself mode.

The remainder of this chapter details the current **editor core**
`EditorInit` / `EditorCore` shapes.

## 1. Two types, one handoff

Boot and steady state are different types, not different flags:

- **`EditorInit`** exists only during boot. It exclusively owns the
  editor-core state under construction (`&mut self`, no locks), runs
  the boot stages (2.2 §1), and is *consumed* by the handoff.
- **`EditorCore`** is the steady-state root. It is inert shared state —
  registries and maps behind the hostapi — with no run loop of its
  own. It can only be constructed by `EditorInit::boot` (LF13).

```rust
impl EditorInit {
    pub fn new(args: LauncherArgs) -> EditorInit;                  // boot stage 0
    pub fn boot(mut self) -> Result<Arc<EditorCore>, BootError>; // stages 1..6,
                                                             // then the move
}
```

On success, every retained field moves into `EditorCore`, wrapped in
its per-field lock exactly once; boot-only state dies with `EditorInit`
(the Linux `__init`-section analog). On failure, `EditorInit` drops
whole — no partially-constructed editor-core state escapes. The
boot/steady-state boundary is the type system, not a runtime stage
check.

## 2. `EditorInit` shape

```rust
pub struct EditorInit {
    args:            LauncherArgs,      // boot-only; dies with EditorInit
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
`EditorInit`. The `event_bus` is already `Arc` because the log ring and
early-boot stderr (LOG8) are live from stage 0; it transfers into
`EditorCore` unchanged, so timestamps and ring content are continuous
across the handoff.

## 3. `EditorCore` shape

> **Boot-core subset note.** The struct below is the spec target. The
> boot-core realization (#778 Phase 2) carries a documented subset —
> `abi`, the boot anchor, the event bus, the log ring — and grows
> monotonically toward this shape as later phases land their fields'
> consumers (registries, session/buffer/window maps, correlation
> allocator). Placeholder fields are not fabricated ahead of their
> consumers.

```rust
pub struct EditorCore {
    pub abi:                Arc<EditorCoreAbi>,
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

`EditorCore` is shared via `Arc<EditorCore>`; per-field locks own
concurrency. There is no central `Mutex<EditorCore>`. One `EditorInit::boot`
run produces one editor-core instance, regardless of OS process — the
embedded launcher (1.3) hosts an editor-core instance and a client
runtime in the same process.

> Note (non-normative): `ShardedMap<K, V>` is an in-repo sharded
> concurrent map realized by target-neutral `lib/*` algorithms over the
> installed system/backend primitives. The editor core names the `uapi/*`
> up-face and target-neutral libraries; it does not name `arch` or `kabi`
> directly.

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

Boot-time cdylib loads populate these registries through `EditorInit`
under exclusive ownership; runtime load/unload operates on the
same registries through `EditorCore` under the per-field locks. Same
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
> primitive built over the installed system/backend sync primitives
> (condvar + ticket counter); it provides the same acquire/release
> contract as a FIFO async mutex without a third-party runtime dependency.

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

1. Whether `Inventory` is per editor-core instance or per OS process
   (relevant when the embedded launcher hosts an editor-core instance
   alongside a client runtime in one process). Default: per
   instance.
2. `EffectiveConfig` lifetime — currently bound to the editor-core
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
