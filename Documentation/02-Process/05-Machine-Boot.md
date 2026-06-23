# 2.5 — Machine Boot

**Scope.** RTOS-itself boot: the system-kernel boot layer that sits between
`arch::_start` and the editor-core boot, its subsystems, the boot-proof
model, the device lifecycle, connected-device handling at runtime, and the
scheduler park/wake seam. Over-OS mode skips this layer — the host OS *is*
the underlying system role.

**Heritage.** Design-stage. The bare-metal floor and the editor-core
`EditorInit`/`EditorCore` handoff exist (`#796`/`#797`); the middle machine-boot
layer is the 0.16 design that makes a single-seat Pi appliance real. Mode
overview: `01-Architecture/06-OS-Modes.md`.

**Locked rules.** None new (design-stage). `machine.*` event families are
provisional and must follow LOG2 rendering.

---

## 1. The missing middle layer

The existing boot model is a good floor but is missing a middle layer:

```text
arch::_start
  -> system-kernel boot
     -> boot-profile selection
     -> IRQ/timer/memory/device/block/fs init
     -> editor-core boot
     -> platform runtime launch (tui over hosted OS, console on Pi)
```

RTOS-itself needs a real system kernel, not just `arch` plus the editor core.
`arch/` stays the smallest unsafe/hardware boundary (no policy);
system-kernel boot owns the services that are too large to hide inside
`arch::sys`, then calls the existing editor-core boot as a payload. This keeps
`editor/lib/core` from becoming an
accidental operating-system kernel.

| Layer | Owns |
|---|---|
| `arch/` platform floor | CPU entry, asm, MMIO primitives, timer registers, UART bytes, memory-map handoff |
| system kernel | scheduler, IRQ routing, driver model, block cache, filesystem, console device, power/shutdown |
| editor core | sessions, domains, streams, state, services, view/update scheduling |
| client/platform runtime | input/render adaptation (hosted terminal vs framebuffer + keyboard) |

## 2. Machine-kernel subsystems

Minimum real-kernel subsystems, ordered by what unblocks a credible
appliance:

1. **Scheduler.** Tickless event scheduler driven by the generic timer +
   IRQ wake. The first profile can stay cooperative/non-preemptive for
   editor tasks, but the IRQ path must be real. Priority bands and bounded
   preemption come only where input/display/block latency requires them.
   The park/wake seam is in §6.
2. **Interrupts.** GIC-400 setup, vector table, IRQ mask/ack/eoi, timer
   IRQ, UART IRQ, then PCIe/xHCI IRQ. This is the line between toy polling
   and a real appliance.
3. **Memory.** Physical memory map, page allocator, DMA-safe regions,
   MMU/cache policy. The current arena is enough for boot/selftest, not
   USB/block/display.
4. **Block.** EMMC2/SD driver (or a simpler early target), request queue,
   block cache, flush/barrier semantics, error model. Persistence is not
   credible until block writes have ordering and failure semantics.
5. **Filesystem.** A real persisted store, not only ramfs. FAT32 is
   attractive for SD interop; a log-structured store may fit PS1 atomicity
   better — a spec decision on atomic save for non-POSIX block stores is
   open (`02-Process/04-Persistence.md`).
6. **Device model.** A static bus/device/driver registry first (platform
   devices, PCIe root/VL805, USB device/interfaces, HID keyboard,
   framebuffer, block). Avoid Linux-scale hotplug until needed.
7. **Console / TUI substrate.** A framebuffer text console and input
   pipeline producing the same reovim raw-input records — not an ANSI/termios
   emulation (`01-Architecture/06-OS-Modes.md` §5).

## 3. Boot proof model

Logging machine facts is not enough; the system-kernel boot layer must **prove**
required facts before later phases can rely on them. Boot produces a typed
system boot proof (not a user-facing config) before launching the editor core.

```text
arch::_start
  -> system-kernel boot
     -> collect raw environment facts
     -> prove required invariants for the selected boot profile
     -> build system boot proof
     -> publish device-inventory messages
     -> hand system services + proof to editor launch
```

Proofs are **profile-gated**:

| Profile | Required proofs |
|---|---|
| `selftest` | CPU mode, stack/BSS, timer, UART/semihost exit |
| `headless-diag` | CPU mode, timer, UART TX/RX or diagnostic output path |
| `recovery` | console output, ≥1 input path, block read path if storage repair is enabled |
| `appliance` | timer IRQ, GIC, framebuffer, USB HID keyboard, persistent block/fs, panic output path |

Failure policy:

- A missing **required** proof for the selected profile aborts before
  editor-core `EditorInit::boot`.
- Optional devices may enter `Degraded` and still boot if the profile
  allows it.
- Every degraded/failed optional device emits a structured machine event
  with the reason and the profile rule that allowed boot to continue.

Development/diagnostic modes do **not** silently weaken appliance proofs —
they are explicit profiles. `appliance` refuses to boot the editor UI
without framebuffer + USB keyboard + persistent store.

## 4. Device lifecycle

A small explicit state machine; avoid Linux-scale hotplug first.

```text
Declared -> Probing -> Present -> Bound -> Ready
                     \-> Absent
                     \-> Failed
          Bound -> Degraded
          Ready -> Lost     (runtime; see §5)
```

- `Declared`: static platform device or bus root from the board profile.
- `Probing`: driver is reading registers/descriptors.
- `Present`: hardware responded; identity known.
- `Bound`: a driver owns the device.
- `Ready`: a subsystem can depend on it.
- `Degraded`: usable with reduced capability (e.g. framebuffer without
  EDID).
- `Absent`/`Failed`: not usable; boot continues only if profile rules
  allow.

Initial device classes: `cpu`, `memory`, `timer`, `irq`, `console`,
`display`, `bus`, `input`, `block`, `fs`, `power`.

### 4.1 Device-info payload

A device message carries enough identity to debug the machine without
guessing — fixed fields plus optional class-specific extensions:

```text
device_id          stable boot-local id
stable_path        e.g. "platform/pcie0/usb0/1-1:hid0"
class              cpu|memory|timer|irq|display|input|block|fs|power|bus
driver             e.g. "gic400", "bcm2711-mailbox-fb", "vl805-xhci", "usb-hid-kbd"
state              declared|probing|present|bound|ready|degraded|absent|failed
vendor device revision     PCI/USB ids where applicable
mmio_base mmio_len irq dma_bits
capacity_bytes             block/memory
width height pitch format  display
error_code reason          failures (bounded static reason label)
profile_required           whether the current profile requires this device
```

Rendered lines (LOG2) read like:

```text
machine.device.ready stable_path=platform/timer0 class=timer driver=arm-generic-timer freq_hz=54000000 irq=30 profile_required=true
machine.device.ready stable_path=platform/fb0 class=display driver=bcm2711-mailbox-fb width=1280 height=720 pitch=5120 format=xrgb8888 profile_required=true
machine.proof.ok profile=appliance required=timer,irq,display,input,block,fs
```

### 4.2 Machine boot event families

Machine-kernel DS12 families, separate from editor `boot.stage.*`,
provisional names, structured (not free-form), rendered by LOG2. Start with
typed event structs per family; a generalized field map waits until the
third independent family needs it.

`machine.boot.profile`, `machine.env.probe.{start,ok,fail}`,
`machine.device.declared`, `machine.device.probe.{start,ok,fail}`,
`machine.driver.bind.{ok,fail}`, `machine.device.{ready,degraded,absent}`,
`machine.proof.{ok,fail}`.

## 5. Connected-device handling (runtime)

Boot proves the *minimum* environment for the profile; runtime connection
handling manages devices that appear, disappear, reset, or change state
after the machine is serving. Boot must not treat every missing optional
device as failure; runtime must not treat every disconnect as a panic. The
profile decides which losses are fatal.

| Device kind | Boot treatment | Runtime treatment |
|---|---|---|
| Required keyboard | present or boot enters recovery/headless | disconnect pauses input + degraded; reconnect can resume |
| Required display | present or boot enters recovery/headless | mode-loss fatal/degraded per profile |
| Persistent root store | must prove mount/commit before editor starts | removal forces read-only/degraded or safe shutdown |
| Extra USB kbd/mouse | optional | hotplug attach/detach |
| Removable storage | optional unless selected as state root | mount only after explicit trust/profile rule |
| Diagnostic serial | optional for appliance, required for diag | attach/detach without affecting editor state |

Runtime state transitions extend the boot machine:

```text
Ready -> Suspended -> Ready
Ready -> Lost -> Reprobing -> Ready
Ready -> Lost -> Removed
Ready -> Failed
Degraded -> Repaired -> Ready
```

**Generation fencing.** Every transition must revoke or fence stale handles
before new input/storage events flow. A reconnected device receives a fresh
generation; queued events from the old generation are discarded. (This is
the same generation rule the Stream device Domain relies on —
`04-Domain-Substrate/06-Device-Domains.md` §6.2.)

Connected-device event families (provisional): `machine.device.{connect,
disconnect}`, `machine.device.reprobe.{start,ok,fail}`,
`machine.device.generation.bump`, `machine.device.handle.revoked`,
`machine.device.policy.blocked`, `machine.storage.readonly`,
`machine.shutdown.device_loss`. Payloads carry `(device_id, generation,
stable_path, class, driver, state_before, state_after, profile_required,
reason, error_code)`, plus bus-local identity for bus devices
(`bus_path, vendor, device, class_code, subclass, protocol`, and a
`serial_hash` — serials are hashed/redacted in public logs).

### 5.1 Input-device rules

Key/mouse events carry `(device_id, generation)` internally until
normalized into reovim raw input; the pipeline drops stale-generation
events. Key state is cleared on disconnect or generation bump (no stuck
modifiers); repeat timers cancelled; reconnect starts a clean HID state
machine. Multiple keyboards feed one normalized stream while logs retain
device identity. Layout is not hard-coded in the HID driver — HID reports
become normalized physical key events first, then layout maps to text/key
commands.

### 5.2 Storage-device rules

Storage is more dangerous than input; loss must be explicit. If the device
backs the state root, loss immediately closes new writes and moves
persistence to `ReadOnlyLostBacking` or starts controlled shutdown, per
profile. Dirty buffers stay in memory — no fake successful save.
Reappearance does **not** auto-resume writes unless the block-identity
proof matches the prior generation and filesystem recovery succeeds.
Removable storage not selected as state root is inert until explicitly
mounted/trusted.

### 5.3 Trust boundary

The system kernel can detect and identify connected devices, but trust
policy is explicit, so "connected-device support" never becomes ambient
authority: the boot profile may allow/deny classes; an unknown HID keyboard
may be accepted for input in appliance mode; unknown storage is never
auto-mounted writable as state; user-identifying descriptors are
redacted/hashed in public logs. This is the machine-layer half of the
device-Domain trust gate (`04-Domain-Substrate/06-Device-Domains.md` §7).

## 6. Scheduler park/wake seam

The archive runtime's `tick()` never blocks (busy-poll); pacing was
external (a tokio interval in the hosted TUI). Under the no_std floor tokio
does not exist — a pacing vacuum in-repo code must fill, on the critical
path of the runtime realization independent of OS-mode. One seam serves all
targets.

- **Mechanism (`arch/`):** one primitive pair `park_until(Option<Instant>)`
  + `unpark`. Linux: futex wait-with-timeout. Bare metal: WFI +
  generic-timer compare (CNTP_CVAL) — the next deadline literally programs
  the compare register. Windows: `WaitOnAddress` timeout.
- **Policy (sched):** a HIGH/LOW split (Linux hrtimer/timer-wheel
  precedent):
  - **HIGH** deadline tier: min-heap of `Instant` deadlines, a
    `next_deadline()` API. Consumers: frame pacing (16 ms moves from the
    external interval into kernel sched), input coalescing, key repeat.
    Precision matters; these fire.
  - **LOW** timeout tier: coarse hashed wheel (~10–50 ms buckets) for
    cancelable timeouts (debounce, autosave, saturator, connection
    timeouts). O(1) insert/cancel; usually canceled; precision irrelevant.
  - **Tickless loop:** `park_until(min(high.next, low.next_bucket))`; events
    wake via `unpark` → input latency = wake cost, not tick cadence (the
    fastest-reaction goal); on Pi, idle = WFI instead of spin.

Staging (rule of three): land the seam + one deadline-ordered timer with
`next_deadline()` (replaces the full-scan timer map) first; split HIGH/LOW
behind the same API only when cancelable-timeout churn is measurable.
Design the API so the split is additive — no caller assumes a single
structure.

## 7. Threaded interrupt handling

IRQ is World-layer, system-kernel-owned, below the platform contract
(`06-ABI/05-Platform-Contract.md`). Interrupts exist only on bare metal;
on Over-OS the host kernel owns them and delivers fd-readable /
futex-wake / syscall-return. The IRQ effect surfaces upward only as
vocabulary already defined here and in the device model:

- **`unpark`** (§6) — "the loop should wake."
- **`poll()` / `Readiness`** (the `kabi/device` Stream plane,
  `04-Domain-Substrate/06-Device-Domains.md §6`) — "data ready."

On bare metal the IRQ top-half sets those signals; on Over-OS the host
fd/futex state sets them. **Same face above the airlock** — the
editor/client core never sees an interrupt.

**Threaded-IRQ: no driver/cdylib code runs in interrupt context.** The
top-half is a tiny fixed system-kernel routine, never loaded code:

```text
HW IRQ (GIC)
  -> system-kernel TOP-HALF  [IRQ context · WCET-bounded · no alloc · no policy]:
        ack -> identify source -> MASK source -> mark pending -> unpark -> EOI
  -> scheduler wakes (park_until returns)            [normal context]
  -> system-kernel BOTTOM-HALF under budget:
        drain device via the DRIVER VTABLE (normal-context poll/service)
        -> produce normalized event / Chunk
  -> editor/client tick() consumes via the SAME event/stream path as Over-OS
```

Consequences:

- **No interrupt-context driver entry.** Drivers are pure normal-context
  vtables; trusted-native cdylib code never runs with interrupts masked.
  This removes the worst hazard of hot-loaded native drivers.
- **Small, fixed top-half → verifiable TCB.** A hand-checked
  `asm!`-terminal axiom; the part that must be proven correct is minimal.
- **NAPI demotion.** Under flood, the bottom-half keeps the source masked
  and polls until drained, then re-unmasks — bounded work under load
  (converts unbounded interrupt rate to bounded drain work at the boundary).
- **Real-time.** Top-half WCET-bounded; bottom-half budgeted per source;
  deadline classes order the drain (input ≈ hard-deadline); priority
  inheritance in system-kernel locks (the Pathfinder lesson).

Code ownership:

| Piece | Owner |
|---|---|
| CPU IRQ primitives — vector-table install, DAIF mask/unmask, vector stub | `arch` |
| IRQ subsystem — GIC programming, source→driver routing, pending-set, threaded top-half, `unpark` | system kernel (board-specific GIC backend) |
| Per-device IRQ work — registers its IRQ line at bind | loadable driver, normal-context only |

**Boundary case.** The interrupt controller (GIC) and timer are **not
hot-unloadable** — you cannot unload the thing routing all interrupts.
Like the loader itself, they are part of the boot mechanism, bound
during machine boot (`irq`/`timer` BootProof requirements for
`appliance`), even though they use the same vtable contract. The "no
compiled-in driver" rule (§2, Machine-kernel subsystems) holds for
*device* drivers (UART/xHCI/GPIO/block); the IRQ controller is
system-kernel mechanism, not a hot-pluggable device.

## Conformance

| Behaviour | Fixture |
|---|---|
| Proof gate | `appliance` boot with a missing required proof (e.g. no framebuffer) aborts before editor-core `EditorInit::boot` and emits `machine.proof.fail`. |
| Degraded optional | An optional device entering `Degraded` boots under a profile that allows it and emits the allowing rule. |
| Generation fence | A keyboard reconnect bumps generation; queued old-generation key events are dropped and modifiers cleared. |
| Storage loss | State-root storage loss closes new writes (no fake save) and moves persistence to read-only or controlled shutdown per profile. |
| Tickless wake | An input event during idle wakes the loop via `unpark` rather than waiting for the next timer deadline. |
| IRQ isolation | No driver code runs in interrupt context; the top-half calls only fixed system-kernel primitives (ack, mask, unpark, EOI). |
