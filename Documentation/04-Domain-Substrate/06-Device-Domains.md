# 4.6 — Device Domains

**Scope.** Editing hardware and devices — memory, registers, GPIO,
fuses, device I/O — as buffers. Effect typing for I/O, the acquire/render
split, the commit barrier, change-record directionality, and the
capability traits a device Domain implements.

**Heritage.** Design-stage. This is a 0.16 goal on two counts. The
*substrate contract* is in scope because text is consumer #1 and the
bare-metal appliance (`01-Architecture/06-OS-Modes.md`) brings
memory/register/GPIO consumers #2/#3. And — revising the earlier
deferral — **the first concrete device Domain ships in 0.16**: the Pi
appliance is the *earned* consumer (rule of three satisfied, not invoked
to defer), so one real device Domain (GPIO or mem) is built cross-mode
(Over-OS host `/dev` + bare-metal device service). The discipline that
remains is narrower: build the *one* earned Domain, not all four (register,
fuse, …) speculatively. The bridge contract (§8) is the gating
prerequisite. Vision companion: `future/design-spine.md` §IV.

**Locked rules.** None new (design-stage). Candidate rules below are
proposed, not yet gating.

---

## 1. The two assumptions that break

The Domain substrate today encodes **text-domain policy as substrate
mechanism**. Two editor invariants are true for text and false for
hardware:

- **Read can have a side effect.** Reading a device cell can mutate the
  source: read-to-clear status registers, FIFO/RX pop, read-triggers-action,
  volatile values.
- **Write can be "can't undo."** A device write can be physically
  irreversible (OTP/fuse burn), a side-effecting command (reset/erase/DMA),
  or an order-dependent sequence where a partial write leaves an invalid
  state.

Three baked-in assumptions break:

| Substrate fact (text) | Hidden assumption | Breaks on hardware |
|---|---|---|
| render is called every cycle, pure over an in-memory snapshot | re-reading the source is free/idempotent | read-to-clear, FIFO pop, triggering reads |
| buffer = source of truth; edits are pure byte transforms | the buffer *is* the data | the device is the truth; buffer is only a snapshot |
| persist save/load are separate, deferrable steps | writes reach the world only on explicit save | each keystroke-write could hit hardware irreversibly |

The fix is not a hardware *feature*. It is **demoting these assumptions
from hidden mechanism to declared, per-Domain effect properties**, then
branching the pipeline (acquire / render / stage / commit) on them. Text
becomes the trivial instance — pure read, reversible write, no
constraints — which is the zero-point of the new axis and keeps the
substrate add-only.

## 2. Effect typing

A Domain declares effect classes — per-Domain, refinable per-region/cell
(see Open items). Two axes match the two broken invariants. These
describe the **source**, not the editor; declaring them lets the
substrate enforce the correct pipeline generically instead of assuming
text behaviour.

**Read effect (acquisition semantics):**

```text
Pure         idempotent; re-read free               (text, file, RAM dump cache)
Volatile     changes under you; re-read safe but diff is meaningless (counter, sensor)
Destructive  read mutates source: read-to-clear, FIFO/RX pop  -> re-read FORBIDDEN
Triggering   read causes external action beyond the cell
```

**Write effect (commit semantics):**

```text
Reversible   stage + undo + re-read to confirm      (text, file, scratch RAM)
Irreversible commit-once, physically permanent       (OTP/fuse) -> no preview, typed confirm
Sequenced    atomic ordered transaction; partial write = invalid device state
Triggering   write has side effects beyond stored value (reset, erase, DMA, power)
```

## 3. Acquire / render split (read has side effects)

`render` stays **pure over the in-memory snapshot** — it never touches the
source. This is already true for text; the contract **locks it as an
invariant**: render never triggers a read. Acquisition becomes explicit
and effect-typed:

- `acquire(region, mode)` is the **only** side-effecting read.
  - `Pure`/`Volatile`: the editor MAY auto-acquire (load, controlled poll
    with a visible "live" indicator).
  - `Destructive`/`Triggering`: acquire is **only ever user-initiated**.
    The substrate guarantees **no implicit re-acquisition**: no prefetch,
    no render-ahead, no scroll-back re-read, no hover preview, no
    speculative refresh.
- The buffer becomes an explicit **captured-at-T snapshot** with
  staleness/generation metadata, not "live truth." Generation fencing
  mirrors the connected-device generation rule in
  `02-Process/05-Machine-Boot.md`.

This is "observing is touching": a naive renderer polling a clear-on-read
FIFO would destroy data by displaying it. The render contract references
access type; an effectful region shows a placeholder ("read has side
effect — read explicitly") and reading becomes a deliberate act.

## 4. The commit barrier (write can't undo)

The device write model maps onto the persistence seam:

- Edits **stage in-buffer** → undoable, free, reversible *because nothing
  reached hardware yet*. Undo only ever touches the staging overlay.
- The buffer view becomes a **diff/staging view**: acquired snapshot vs
  staged edits; pending writes highlighted (Decor view-only tier).
- **Commit is the one-way gate:**
  - gated by write-effect class (confirmation; **typed confirmation** for
    `Irreversible`/`Triggering`),
  - **transactional + ordered** for `Sequenced` (all-or-nothing; never
    leave a partial sequence),
  - emits **structured audit events** (device id, region, before/after
    where readable — `machine.*` families,
    `02-Process/05-Machine-Boot.md`),
  - **undo barriers at commit**: staging is undoable, the commit is not.
- After commit, **re-acquire** to show new device truth — but only where
  reads are non-destructive.

This generalizes the existing non-undoable `External` origin
(`04-Domain-Substrate/03-Undo.md` §1): a device commit produces an
undo-barrier origin.

## 5. Change-record directionality

The commit barrier is **not** where recording stops. A change is still
**recorded** past the barrier — it just becomes a **non-backward** record
(forward-only, no inverse). History continues; it loses its inverse.

Directionality is **two independent capabilities** on a change record, not
a single undo/redo pair:

```text
backward (undo)    requires captured prior state AND a reversible source
forward (replay)   re-apply the same change forward
```

| Change | backward | forward | inverse |
|---|---|---|---|
| text `Reversible` edit | yes | yes | `EditRecord{old,new}` |
| `Irreversible` commit (OTP) | no | maybe (no-op / re-burn) | `old = None` |
| `Triggering` write | no | "replay" re-fires side effect | none |
| `Sequenced` | whole-transaction only | whole-transaction only | per-transaction |

Two record sinks:

- **Undo stack** — backward-capable records only (in-memory staging
  edits).
- **Change journal** — append-only; ALL changes incl. non-backward
  committed hardware writes. Forward-replayable where the source allows;
  never invertible.

The commit barrier is the transition: a record moves from the undo stack
to the journal, dropping its inverse.

This requires one substrate extension. `EditRecord.old_bytes`
(`04-Domain-Substrate/03-Undo.md` §2) is `Bytes`; a non-backward record
is structurally **`old = None`** — "no inverse available." So the device
contract widens it to `old: Option<Bytes>`. This is *exactly* what a
`Destructive` read also produces: the prior value was never safely
captured, so its change has no old state and is inherently non-backward.
The read-side and write-side halves of the design meet at `old = None`.

The write journal also degrades undo gracefully where rewind is
impossible: it needs no reads (you know what you wrote) → safe even for
effectful-read regions. Replay (reset device + replay journal) reproduces
the state sequence, turning a debug session into a regression fixture. The
journal costs hot-path latency: gate it behind debug mode, out of
`#[bounded]` paths or within budget.

## 6. Capability traits (Linux-faithful, null-fop = optional capability)

The model adopts Linux `/dev`: a device is a name binding to a driver
vtable (`file_operations`); unsupported operations are null pointers —
the same opaque `#[repr(C)]` vtable dispatch the reovim driver ABI
already uses. Where reovim **must** diverge: Linux leaves effects untyped
because its client is a programmer issuing explicit syscalls who read the
man page; reovim's client is an editor UI that auto-re-reads on scroll and
offers undo. Effect typing (§2) is not gilding Linux — it is the cost of
having a render/undo pipeline a syscall API does not have.

A device Domain implements **only** the capability traits it supports
(mirroring null fops). No God-trait. Text implements **none** of these —
it stays the pure in-memory buffer with the existing handler/projector.

```rust
pub trait Effectful: Send + Sync { fn effects(&self) -> &EffectMap; }

// 1. STREAM / FOLLOW  (char device: poll + read/write) — forward-only, NO address
pub trait Stream: Effectful {
    fn poll(&self) -> Readiness;                                         // Linux .poll — PURE
    fn read_forward(&self, max: usize) -> Result<Chunk, DomainError>;    // Linux read — CONSUMES
    fn write_forward(&self, bytes: &[u8]) -> Result<usize, DomainError>; // Linux write — send
}

// 2. ADDRESSED  (block/mmap) — random-access cells, OVERWRITE-only (no shift)
pub trait Addressed: Effectful {
    fn acquire(&self, addr: Addr, width: AccessWidth) -> Result<Word, DomainError>;
    fn commit(&self, changes: &[Change], confirm: Confirmation) -> Result<Committed, DomainError>;
}

// 3. CONTROL  (ioctl) — typed command, not an edit; inherently Triggering
pub trait Control: Effectful {
    fn command(&self, cmd: CmdId, args: &[u8], confirm: Confirmation) -> Result<Reply, DomainError>;
}
```

`RenderProjector` is unchanged for all three: pure paint over whatever the
buffer currently holds.

### 6.1 The plane split dissolves the address question

There is no single address model — each plane carries its own. (This is
why the earlier flat `Range<usize>` buffer model was rejected: it smuggled
text-editing assumptions — flat 0-based contiguous addressing +
insert/delete-with-shift — back into the substrate.)

| Device kind | plane | addressing |
|---|---|---|
| serial / log / sensor stream | Stream | none (forward position only) |
| `/dev/mem`, framebuffer, block | Addressed | `Addr(u64)` — physical addr / mmap offset / block no. |
| GPIO attr, ioctl | Control | command / attr **id**, not an address |

`Addr(u64)` is correct precisely because the only plane that uses an
address is the genuinely-flat one; named things went to Control as ids.

### 6.2 "Read with follow" (tail -f) — the Stream shape

The buffer is an **append-only accumulation of bytes already consumed**.

```text
loop:  poll()          -> Readiness   (PURE — drives the "live" indicator, no consume)
       Readable(n)     -> read_forward(n) -> Chunk -> APPEND to buffer, advance fwd cursor
       render(buffer)  -> pure paint of accumulated history
```

```rust
pub enum  Readiness { Pending, Readable(usize), Hangup }
pub struct Chunk { pub bytes: Bytes, pub generation: u64 }
```

- The source is destructive-forward: consumed bytes are gone from the
  device but live in our buffer → scrollback reads the buffer, never
  re-touches the source.
- `poll()` being pure is what makes "is there more?" ambient and safe
  while the consuming act (`read_forward`) stays explicit — the "read has
  a side effect" problem solved the way Linux solved it.
- `Chunk.generation` fences device reconnect (the connected-device
  generation rule).

### 6.3 "Write at some dev" — pick the plane the device declares

| Device kind | write expressed as | reversibility |
|---|---|---|
| serial / stream | `write_forward(bytes)` | none — sent is sent; the send IS the commit boundary |
| mem / register / fb | stage `Change{addr,width,new,old}` → `commit()` | undo the staging; commit overwrites in place; non-backward past barrier |
| ioctl / control | `command(cmd, args, confirm)` | none — Triggering, gated by typed `Confirmation` |

Data writes to an addressed device flow through stage→commit (undoable
until commit). Stream sends and control commands are inherently one-way
and go straight through the gated barrier.

## 7. Layer ownership

- **Substrate (this chapter)** — effect typing, acquire/render split,
  commit barrier semantics, capability traits. Mechanism.
- **`ext/server/domain/*`** — concrete device Domains (mem, register,
  GPIO, fuse). Implement the effect-typed contract. The first earned
  Domain (GPIO or mem) ships in 0.16; the rest stay unbuilt until earned.
- **Client modules** — dangerous-action confirmation UX, live/staleness
  indicators, staged-diff rendering.
- **Machine layer** (`02-Process/05-Machine-Boot.md`) — trust gating
  (editing `/dev/mem`/OTP requires capability, not ambient authority) and
  audit-event transport.

### 7.1 Driver vs Domain — the same peripheral, both sides of the airlock

A peripheral appears on **both** sides of the airlock, by role. The two
never merge:

| Role | What it is | Layer | Home |
|---|---|---|---|
| Hardware that runs the machine | xHCI + USB enum + HID → input; GPIO MMIO (UART pins, reset, LED) | **World** — system-kernel driver | `ext/system/drivers/{xhci, usb-hid-kbd, gpio, emmc, …}` |
| Content the user edits | GPIO-as-buffer; register/serial opened as a buffer | **Math-ish** — editor device Domain | `ext/server/domain/{gpio, mem, register}` |

The **driver** touches MMIO → World, system-kernel-owned, *below* the
contract. The **Domain** edits a buffer → editor-owned, *above* it. GPIO is
the sharp case: the system kernel owns the controller (driver) *and* the
user wants to edit pins (Domain) — opposite sides of the airlock, ownership
arbitrated by the trust boundary (`02-Process/05-Machine-Boot.md` §5.3).
The Domain never touches MMIO; it reaches device access through the bridge
(§8).

## 8. The device bridge (`kabi/device`)

A device Domain must reach device access it must **not** touch directly
(MMIO is World/system-owned; the Domain is above the airlock, §7.1). The
seam that carries that access is a contract: `kabi/device`.

**Resolved: the bridge is a SEPARATE contract, not part of the platform
handle** (`06-ABI/05-Platform-Contract.md`). Device access is a different
axis from machine services — only device Domains need it, and some targets
expose no editable devices — so folding it into the platform handle would
bloat the Math-layer kernels' core dependency. `kabi/device` is its own
seam: a **per-device `#[repr(C)]` vtable** (`DeviceHandle`) carrying exactly
the capability planes of §6 (Stream / Addressed / Control) plus `effects()`,
under the same vtable doctrine as the platform handle.

```text
Domain (ext/server/domain/gpio)            [Math, above the airlock]
  declares: plane = Addressed, effects = {read: Volatile, write: Triggering}
        │ open(device_ref) → DeviceHandle  (capability-gated; no ambient authority)
        ▼
  kabi/device  DeviceHandle = #[repr(C)] vtable:
        Stream    poll / read_forward / write_forward
        Addressed acquire(addr,width) / commit(changes, confirm)
        Control   command(cmd, args, confirm)
        effects() → EffectMap
        ▼ provider below the airlock (differs by mode; SAME face to the Domain)
   Over-OS:    arch-hosted adapter → host syscall → /dev/mem, /sys/class/gpio, serial fd
   bare metal: system-kernel device service → routes to the loaded driver vtable
```

On bare metal this is **three vtables in series** — Domain → device-handle
vtable (bridge) → system kernel → **driver vtable** (§7.1) → MMIO — all the
same `#[repr(C)]` doctrine as the platform handle.

**Who holds the `device_ref → DeviceHandle` registry** (= where the
capability gate lives): the **provider below the bridge**, per mode — the
**system-kernel device service** on bare metal, the **arch-hosted adapter**
on Over-OS. `open(device_ref)` is a call *into* that provider: it resolves
the ref against the device model, checks the capability
(`02-Process/05-Machine-Boot.md` §5.3), and only then mints the handle. The
Domain never names a raw device or holds the table — it presents a
`device_ref` and receives (or is denied) a handle. The gate stays on the
**World** side of the airlock, never in the Math-layer Domain, in both
modes.

**The bridge MUST present a single mode-uniform interface to the Domain.**
This is the enforcement point for the mode invariant
(`01-Architecture/06-OS-Modes.md` §0): if the Domain saw a different shape
over host `/dev` than over the bare-metal device service, device editing
would fork by mode. Only the *provider* below the bridge may differ; the
face the Domain sees may not. Because the first concrete device Domain is a
0.16 feature, this seam is on the critical path to "edit GPIO on the Pi" and
is specified before either side is built.

> Candidate rule — *device editing is capability-gated, never ambient.*
> A Domain reaches a device only through a `DeviceHandle` minted by the
> provider's `open(device_ref)`; there is no path from a Domain to MMIO or
> to a driver vtable. *Class*: depgraph (no `domain → driver` / `domain →
> arch` edge) + runtime (open refuses without capability).

## Open items

1. **`kabi/device` granularity** — one `DeviceHandle` vtable with optional
   (null-fop) planes, or one vtable per plane (Stream/Addressed/Control)?
   Mirrors the per-Domain-vs-per-region effect-typing question below.
2. **Granularity of effect typing** — per-Domain only, or
   per-region/per-cell? Register maps are heterogeneous (one page mixes RW
   data, read-to-clear, write-1-to-clear). Likely needs a region
   descriptor, not a single Domain flag.
3. **Bit-level semantics** — write-1-to-clear and similar need bit-granular
   effect classes; a byte-range record may be too coarse.
4. **Acquire as a handler id?** — add `OnAcquire` alongside the persist
   handlers, or generalize the load handler with an effect-mode argument?
5. **Staging vs `Volatile` sources** — a staged write against a value that
   changed under you between acquire and commit needs a
   compare-and-commit / re-acquire-before-commit policy.
6. **Stale-snapshot rendering** — does `render` need to know it is
   painting a stale destructive snapshot to draw the staleness
   decoration, or does the module own that purely from generation
   metadata?

## Conformance

| Behaviour | Fixture |
|---|---|
| Render purity | A `Destructive`-read Domain never has its source touched by a render cycle; only explicit acquire consumes. |
| No implicit re-acquire | Scroll-back / resize / hover on a `Destructive` region issues zero acquisitions. |
| Commit barrier | Stage three edits on an `Addressed` Domain, undo all three (no hardware write); commit; undo after commit is refused. |
| Non-backward record | An `Irreversible` commit produces a journal record with `old = None` and is absent from the undo stack. |
| Sequenced atomicity | A `Sequenced` commit that fails mid-sequence leaves no partial write and emits a structured audit event. |
| Stream follow | `poll()` on a Stream Domain consumes nothing; `read_forward` appends and advances; scrollback re-reads the buffer only. |
| Bridge gate | `open(device_ref)` without the capability is refused; a Domain has no depgraph edge to a driver or to `arch`; the same Domain binds over host `/dev` (Over-OS) and the device service (bare metal) with one face. |
