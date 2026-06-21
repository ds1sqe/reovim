# 1.6 — OS Modes

**Scope.** The two execution modes reovim targets in 0.16 — **Over-OS**
(hosted on an existing OS) and **RTOS-itself** (the single-purpose runtime
on bare metal) — the mode-invariance invariant that keeps the editing
mechanism byte-identical across modes, the platform-floor doctrine that
keeps them one codebase, the three-kernel model as it instantiates per
mode, and the unified launch model.

**Heritage.** Design-stage. 0.16's thesis is "environment is not a host":
the freestanding/bare-metal target is the 0.16 goal, and the bare-metal
aarch64 selftest image under QEMU raspi4b already boots. Multi-OS hosted
ports (Windows/macOS/Solaris) are a separate later track —
`future/multi-os-ports.md`. Vision companion: `future/design-spine.md` §I.

**Locked rules.** None new (design-stage). DAG6 governs the platform floor
(`01-Architecture/02-Project-Layout-and-DAG.md`).

**Related chapters.** Three-kernel model: `02-Process/01-Kernel-Types.md §0`.
Platform contract seam: `06-ABI/05-Platform-Contract.md`. Device-domain
bridge: `04-Domain-Substrate/06-Device-Domains.md §8`. Client input
deferral: `08-Client/02-Platform-Runtimes.md`.

---

## 0. The mode-invariance invariant

**The editing mechanism is byte-identical whether reovim runs over
Linux/Windows or freestanding on bare metal.** This is the chapter's
thesis; every design decision below is downstream of it.

- **Above the contract (invariant — must not change a line by mode):**
  the editor kernel — domains, undo-tree, streams, buffer algebra, and
  the device-Domain effect/stage/commit semantics — plus the client's
  editing-facing model — normalized raw-input → commands, the frame/cell
  render model. Same code, same behavior, same UX on every target.
- **Below the contract (varies — and only here):** host syscalls vs
  hardware MMIO; terminal vs framebuffer; host events vs USB HID; host
  `/dev` vs the system device service; `dlopen` vs in-kernel loader.

`kabi/platform` (`06-ABI/05-Platform-Contract.md`) is that line. The
seam exists to *guarantee* this invariant, not for cleanliness. Anything
that would make editing behave differently by mode is, by definition, a
leak across the line and a bug.

**Two honest enforcement caveats:**

1. **Device editing.** This invariant holds for device editing *only if*
   the bridge contract (`04-Domain-Substrate/06-Device-Domains.md §8`)
   presents one mode-uniform interface to the Domain. The bridge is
   therefore the load-bearing guarantee of the invariant for device
   Domains, not a convenience.
2. **Client-input leg.** Today the invariant is mechanically guaranteed
   only on the output side. The `RawInput` vocabulary is a real ABI type
   and the frame/cell model is the render contract. Input decode is still
   **inline in the tui platform**, not behind a contract, so nothing yet
   *prevents* a future bare-metal HID path from diverging from the
   terminal path. The invariant is asserted there but not enforced until
   the input-source path is contract-bound — deferred to
   `08-Client/02-Platform-Runtimes.md`.

**Why the invariant holds by construction.** `kabi/platform` carries
**canonical POSIX values owned by `uapi/posix`**, not a Linux passthrough.
Every target's mechanism is canonicalized to those values *below* the line
by its provider, so the editor above the line observes one personality on
every machine. The invariant is therefore a tautology, not a discipline:
there is no mode-specific value left for the editor to observe, because
canonicalization already happened underneath. The slot types are specified
in `06-ABI/05-Platform-Contract.md`.

## 1. The two modes

1. **Over-OS mode** — reovim runs as a hosted program on an existing OS.
   Linux/macOS/Windows provide process isolation, files, sockets, terminal
   or window system, clocks, threads, and dynamic loading. reovim keeps its
   internal kernel model: domains, streams, state, scheduling, module and
   service boundaries. The host OS is treated as a hardware/provider
   substrate reached only through `arch/` and runtime-owned adapters.

2. **RTOS-itself mode** — reovim is the single-purpose runtime on the
   machine. On the Raspberry Pi 4 target this means booting the image
   directly and owning timing, interrupts, framebuffer output, USB keyboard
   input, storage, panic disposition, and static module discovery. There is
   no process model to emulate unless a real need forces one; the first goal
   is a reliable single-seat editor appliance.

**Shared invariant — the editor and client kernels do not fork by mode.**
Both modes feed the same protocol/state/domain machinery; the system kernel
is conditional (present in RTOS-itself, absent in Over-OS where the host OS
plays that role — see §6 and `02-Process/01-Kernel-Types.md §0`). The
difference lives *below* the `kabi/platform` contract boundary:

| Concern | Over-OS | RTOS-itself |
|---|---|---|
| Stable boundary | Host OS API via `arch/` | Pi hardware/firmware via `arch/` |
| Scheduler park/wake | futex / WaitOnAddress / host wait | WFI + generic timer + interrupt wake |
| Display | terminal, window system, or hosted runtime | framebuffer console/render driver |
| Keyboard/input | host terminal/window events | xHCI + USB HID keyboard |
| Persistence | host filesystem (POSIX/OS semantics) | SD/block store + explicit PS1 restatement |
| Modules | dlopen / LoadLibrary loader | static linker-section registry first |

## 2. The platform floor doctrine

`arch/` is the single platform floor: `#![no_std]`, zero external deps,
all platform-specific code concentrated in `arch/src/sys/` behind one
boundary. Actual floor consumption across the workspace is ~14 functions
(slab alloc over a page primitive; thread spawn; futex-backed sync;
monotonic/realtime clock; read/write; `openat` as panic flush sink;
exit/gettid). No sockets, no fork/exec, no general file I/O in product
crates. The kernel's own scheduler is a synchronous tick loop over `arch`
types with no OS calls.

Because every OS dependency sits behind that ~14-function seam, and the
scheduler is a synchronous coordinator, **reovim-as-OS is not a rewrite**:
it is a fourth `sys/` backend plus a handful of device drivers, with the
existing selftest runner as the natural first boot payload.

**Not a HAL.** Do not add a public "HAL" layer above `arch/` — it would
duplicate the `arch/` contract and blur the DAG. The `arch/src/sys/`
target-backend convention already *is* the platform floor: Linux backends
bind to the kernel syscall ABI, system-library targets to their
vendor-stable API, freestanding targets to hardware/firmware. Reserve
"HAL" for private freestanding-backend internals and "BSP" for
board-specific code. Keep the public floor signatures fixed; split
freestanding internals by hardware role (`boot`, `console`, `time`,
`memory`, `power`, later `block`/`framebuffer`/`usb`/`irq`) only when
pressure appears.

### 2.1 Bare-metal floor mapping (target `aarch64-unknown-none`)

| Floor service | Linux today | Bare-metal replacement | Difficulty |
|---|---|---|---|
| entry | `_start` reads stack args | EL2→EL1 drop, stack, BSS clear, MMU identity map, caches; boot-args blob instead of argv | small, asm-heavy |
| alloc | mmap under slab | static physical-RAM arena under the SAME slab | tiny |
| time | clock_gettime | ARM generic timer (CNTPCT/CNTFRQ) | tiny |
| read/write fd 0/1 | syscalls | PL011 UART (polled first; GIC IRQ later) | small |
| panic flush + exit | fd + exit_group | UART fd; PSCI SYSTEM_OFF / wfe halt | tiny |
| thread + futex | clone + futex | deferrable; later per-core stacks + context switch, WFE/SEV wait queues, spinlocks on 4×A72 | the only hard one |
| openat / fs | syscalls | ramfs from an initrd-style blob; SD (EMMC2) + FAT32 later | medium |

**The scheduler insight.** The runtime's `tick()` is a single-threaded
synchronous coordinator; modules run synchronously in the caller's
context. On bare metal, **`tick()` IS the idle loop of the machine.** A
first OS image needs no preemptive scheduler, no SMP, no futex: one core
spinning tick, UART RX feeding the input ring. The thread/futex floor is
only needed for the later thread-per-connection runtime tier — a
single-seat appliance may never need it.

### 2.2 The arch hard-split — three knowledges

The single `arch/` floor of §2 is one crate today. As it matures it
**hard-splits into three crate families**, sorted by which of three
knowledges each unit of code carries — never by target alone:

| Family | Knows | Owns | Returns |
|---|---|---|---|
| `arch-sys-{target}` | hardware only | the raw machine mechanism (syscalls, MMIO, asm) | the machine's **NATIVE** values |
| `platform-{target}-{strategy}` | hardware **and** down-face contract | provider gate: adapt NATIVE machine effects to `kabi/*` slot values, install the vtable | provider-facing `kabi` values/results |
| `arch-floor-{target}` | the language only | lang items: `#[panic_handler]`, `#[global_allocator]`, `_start` | nothing — it crosses to the product at LINK, not import |

The split rule is a function of knowledge: a unit of `arch` code belongs to
exactly one family by which knowledge it carries. **The `ioctl` borderline**
makes this concrete — a raw `ioctl` request number and the bytes it moves
are hardware-only (`arch-sys`); the mapping of an `ioctl` result into a
provider-facing `kabi` result is down-face contract knowledge (the provider);
the product-visible POSIX meaning belongs above that, in the
`system/lib/kernel` bridge. The impedance match is split by face:
NATIVE↔`kabi` in the provider, `kabi`↔`uapi` in the bridge.

**Not a HAL — reconciled with §2.** This split is *not* the HAL §2 forbids.
A HAL is a uniform abstraction layer the product imports *above* `arch/`;
this adds nothing above `arch/`. The provider sits *below* `kabi`, is
selected at the composition root (§6), and `arch-sys` stays native — so
there is no second contract above the floor to drift against. The §2 "Not a
HAL" rule stands unchanged; the three-knowledges split refines `arch/`'s
internal structure, it does not raise a layer over it. The
`arch → {}` firewall (no product source names `arch`; three things cross at
LINK) is the DAG rule that keeps the split honest —
`01-Architecture/02-Project-Layout-and-DAG.md`.

## 3. Target classes

| Class | Stable boundary | Examples |
|---|---|---|
| kernel-ABI | OS kernel syscall ABI (stable) | Linux x86_64, Linux aarch64 |
| system-library | vendor libc/libSystem (syscalls unstable) | macOS, Solaris/illumos, Windows (`future/multi-os-ports.md`) |
| freestanding | hardware/firmware | bare-metal aarch64 (Pi 4) |

The spec does **not** enumerate every target and does **not** forbid new
ones (DAG6); it mandates each target's floor be sovereign and in-repo.
Bare metal has no libc tension. The system-library class forces the DAG6
"lowest stable boundary" amendment — deferred to `future/multi-os-ports.md`
until an OS port is scheduled.

## 4. The three kernels by mode

The reovim runtime has three kernels (`02-Process/01-Kernel-Types.md §0`):
the **editor kernel** (sessions, domains, streams, state, scheduling —
sovereign, Math layer), the **client kernel** (raw-input normalization,
frame/cell render, projection/codec — derived, Math layer), and the
**system kernel** (sched, IRQ, memory, device model, block, fs, console,
power — sovereign, World layer).

The editor and client kernels are **mode-invariant**: they never absorb
device drivers, board mechanics, or IRQ handling. Device reality stays
below the `kabi/*` down-face and `arch::sys`; editor and client policy stay
on the `uapi/*` up-face. The `system/lib/kernel` bridge is the only normal
crate family that may name both faces (`06-ABI/05-Platform-Contract.md`).

The **system kernel is conditional by mode** (§6): RTOS-itself mode
carries all three kernels; Over-OS mode has only the editor and client
kernels, with the host OS playing the system-kernel role. RTOS-itself
additionally needs the system kernel for scheduler/IRQ routing/driver
model/block cache/filesystem/console/power — too large to hide inside
`arch::sys`. Its boot sequence, proof model, and device lifecycle are
specified in `02-Process/05-Machine-Boot.md`. The system kernel does not
live in the editor-kernel crate.

## 5. Launch model

Both modes converge on one semantic launch contract — a `LaunchPlan`:
selected editor-kernel config, selected client platform, transport kind,
module-registry kind, persistence root. Only the *physical* launch
mechanism differs.

**Over-OS** is a process command:

```text
reovim                          # default: embedded TUI
reovim --client tui             # explicit hosted TUI
reovim --client tui --external --connect <addr>
reovim --server                 # headless server runtime
reovim --subprocess --client tui
```

Embedded mode links server + selected platform runtime into one process
over an in-memory transport (the target default); subprocess is for
isolation/testing/ops, not the ordinary first-run path. (The current
in-process UDS launcher is a temporary detail that exercises the real UDS
carrier while `execve` is absent from `arch/`.)

**RTOS-itself** has no process launcher. Firmware loads `kernel8.img`, then:

```text
Pi firmware -> arch::_start -> system-kernel bridge boot
  -> boot-profile selection -> editor Init::boot -> console platform runtime
```

The RTOS launch unit is a **boot profile**, not a command:

| Profile | Purpose |
|---|---|
| `selftest` | Run arch/machine/editor selftests; exit/halt with a diagnostic code. |
| `appliance` | Normal editor: framebuffer console + USB keyboard + embedded editor kernel. |
| `recovery` | Minimal framebuffer/UART diagnostic shell for storage/config repair. |
| `headless-diag` | UART-only diagnostics when display or USB is untrustworthy. |

**TUI naming.** Hosted terminal UI remains **TUI** (termios + ANSI +
terminal input). RTOS local UI is **console** / **fb-console**, not a TUI
clone: it shares the reovim frame/cell + normalized-raw-input model but
replaces termios/ANSI/UDS with HID input, framebuffer drawing, and an
in-memory transport. The reusable unit is "reovim renders a cell/frame
model and consumes normalized raw input," not "TUI writes escape
sequences." Hosted TUI and Pi console are two backends for that model.

## 6. Mode instantiation

The mode-invariance invariant (§0) collapses to one question: **"Do we split
the editor and the system?"** = "Is the system-kernel slot filled?"

```
Over-OS:
  editor + client → uapi/* → hosted system role / bridge
    → kabi/platform + kabi/device → hosted provider → arch sys (host syscalls)
  device Domains reach hardware through the up-face device surface, then the
  hosted bridge/provider reaches /dev-/ioctl services below `kabi`
  [system/lib/kernel crate ABSENT — the host OS plus hosted bridge fill the role]

RTOS-itself:
  editor + client → uapi/* → system/lib/kernel bridge
    → kabi/platform + kabi/device → freestanding provider/device code
    → arch (bare-metal floor)
  device Domains reach hardware via the up-face device surface; the system
  bridge maps that to `kabi/device` below
  (04-Domain-Substrate/06-Device-Domains.md §8)
  [system kernel additionally loads device drivers, block, fs, console]
```

In Over-OS mode the editor and client kernels reach the `uapi/*` up-face; the
host OS plus hosted bridge/provider fill the system role over host syscalls,
`/dev`, and fd-I/O. In RTOS-itself mode the same two kernels still reach the
same up-face; `system/lib/kernel` fills the bridge role and maps common system
semantics to `kabi/*` providers below. The system kernel does not own `arch`
by import; raw facts and MMIO stay in provider/floor code.

The composition root (the `reovim` binary / boot image) is the only place
that wires these: it selects the contract implementor and installs the
platform vtable at `Init::boot`. Neither the editor kernel nor the client
kernel sees a mode branch — their code is identical. The mode branch lives
entirely at the composition root and below the contract.

### 6.1 The uapi↔kabi bridge per mode

The bridge/provider pair carries **graduated substance** by target class. The
product-facing POSIX values are fixed in `uapi/posix`; provider-facing slot
types live in `kabi/*`. Hardware/provider-specific crates do **not** import
`uapi/posix` directly unless a documented inescapable exception exists.

| Target class | Bridge/provider substance |
|---|---|
| kernel-ABI (Linux) | **thin** — the hosted bridge maps `uapi/posix` to provider-facing `kabi` slot values; the provider maps those to Linux kernel-ABI calls, often identity at the bit level but still not by importing `uapi`. |
| system-library (Windows/macOS) | **shim** — the hosted bridge/provider pair translates the vendor ABI to the fixed up-face values: errno remap, handle↔fd, open-flag and mode translation. |
| freestanding (bare metal) | **full** — `system/lib/kernel` implements common POSIX-like system semantics over hardware services, and freestanding providers/device code satisfy `kabi/*` over raw hardware. |

A **zero-arch provider** (`platform-linux-mock`) can also satisfy `kabi/*`
with no machine underneath. It canonicalizes nothing real but passes the same
behavioral conformance suite, which is what makes "provider" a contract role
and not a synonym for "the Linux backend."

**Composition is Kbuild-style, not Cargo features.** Each config selects its
provider through a dedicated per-config composition-root crate — there is no
feature flag toggling provider bodies inside one crate. In-tree system
drivers register through an in-tree `#[used]` / `#[link_section]`
distributed-slice (the linker's `__start_/__stop_` section symbols), **not**
an external registry crate — `linkme`/`inventory` are forbidden deps under
DAG5 (`01-Architecture/02-Project-Layout-and-DAG.md`).

**`#[vtable]`-style authoring.** A provider authors its ops table from a
contract trait via an in-tree macro that lowers the trait to the frozen
`#[repr(C)]` table plus per-slot `HAS_*` presence consts. The trait is the
authoring DSL only; it never becomes a runtime `dyn` seam. Full slot shape,
append-only evolution, and the macro contract are specified in
`06-ABI/05-Platform-Contract.md`.

## 7. Spec deltas this mode forces

| Delta | Owner | Needed for |
|---|---|---|
| PS1 persistence atomicity re-stated for non-POSIX block stores | `02-Process/04-Persistence.md` | RTOS persistence |
| Static module registry blessed alongside dlopen | `02-Process/02-Lifecycle.md` / `06-ABI/` (LF3) | RTOS module loading |
| Sched park/wake seam (`park_until`/`unpark`) + deadline-ordered timer | `02-Process/05-Machine-Boot.md` §sched | tickless loop, all targets |
| DAG6 "lowest stable boundary per target" | DAG6 | macOS/Solaris ports only — `future/multi-os-ports.md` |

## Conformance

| Behaviour | Fixture |
|---|---|
| Mode-invariant editor + client kernels | The editor and client kernel crates compile unchanged for both Over-OS and freestanding targets; they name the `uapi/*` up-face, and mode differences appear only in the bridge/provider/floor topology (`06-ABI/05-Platform-Contract.md`). |
| System-kernel conditionality | Over-OS mode has no `system/lib/kernel` crate; the host OS plus hosted bridge/provider fill the system role. RTOS-itself mode has `system/lib/kernel` bridging `uapi/*` to `kabi/*`; the editor/client kernel binary is unchanged (`02-Process/01-Kernel-Types.md §0`). |
| Freestanding floor | `aarch64-unknown-none` selftest image boots under QEMU raspi4b: UART write + generic timer + arena alloc + bare-metal entry, via the unchanged `arch_test!` runner. |
| LaunchPlan parity | A hosted `reovim` invocation and an RTOS `appliance` boot profile resolve to the same `LaunchPlan` fields. |
