# 6.5 — Platform Contract

**Scope.** The seam every kernel reaches its substrate through —
`kabi/platform`. The mechanism/implementation split (the contract
*declares*, `arch` *implements*), the `#[repr(C)]` platform vtable
("platform syscall table") of effectful primitives, where the value-type
layout lives, the boot-time `static` install and its lifecycle relative to
driver and module vtables, and the bounded value the decoupling buys. The
dependency-graph edges this contract implies (`kernels → kabi/platform` and
`→ lib/ds`, never `→ arch`; `arch → kabi`) are normative in
`01-Architecture/02-Project-Layout-and-DAG.md` (§2, §10, §11); this chapter
describes the contract's *shape*.

**Heritage.** Design-stage. Supersedes the archived `dyn Platform`
(`archive/clients/lib/subsys/platform/src/requirements.rs`) consumed as
`Box<dyn Platform>` — the archived `PlatformCapabilities` was snapshotted
into a `#[repr(C)]` struct before every FFI call
(`archive/.../module/src/ffi.rs`), i.e. it became a vtable at the seam
anyway. Also supersedes the earlier *facade* draft of this chapter, in which
`kabi/platform` re-exported `arch`'s value types: that re-export ran backward
through the airlock (`kabi → arch`) and formed a cycle with the provider edge
(`arch → kabi`). This chapter goes straight to the mechanism. Vision
companion: `01-Architecture/06-OS-Modes.md` (modes) and
`02-Process/01-Kernel-Types.md` (the three kernels this contract serves).

**Locked rules.** None new (design-stage). Candidate rules below are
proposed, not yet gating; they inherit the add-only/versioned discipline
of `06-ABI/02-Versioning-and-Vtables.md` (AB3, AB15).

---

## 1. The contract is the airlock, and it points one way

Every kernel — system, editor, client (`02-Process/01-Kernel-Types.md` §0) —
depends on a stable contract, `kabi/platform`, and **never on `arch`
directly**. The contract is the line that makes the editing mechanism
byte-identical across modes (`01-Architecture/06-OS-Modes.md` §0). It does
not exist for cleanliness; it exists to *guarantee* that invariant. Anything
that would make a kernel behave differently by mode is, by definition, a leak
across this line and a bug.

**Mechanism, not facade.** `kabi/platform` *declares* what a platform must
provide; `arch` (and, in RTOS mode, the system kernel) *implements* it.
Dependency flows `arch → kabi`, **never** `kabi → arch` — a contract that
imported its own implementor would run backward through the airlock. The
substrate splits by **what a thing is**:

| Part | What it is | Where it lives | Reached how |
|---|---|---|---|
| **Effectful primitives** | services that read the world or schedule — the allocator, `park`/`unpark`, clock, spawn, thread id, device/block I/O | declared in `kabi`, implemented in `arch` | a `#[repr(C)]` vtable, installed at boot (§3) |
| **Value-type layout + algorithm** | the data structures — `Seq`/`Map`/`Bytes`/`Shared`, the `Mutex`/`RwLock`/`Condvar` state machines | `lib/ds` (Math; `01-Architecture/02` §10) | direct, monomorphic; their *backend* (alloc, park) is reached through the handle |

This is the Math/World airlock applied to the type system: a value's *layout
and algorithm* are identical on every provider (Math, `lib/ds`); the
*allocator and park/unpark behind it* are provider-specific (World, `arch`),
injected at `Init::boot`. There is no re-export of `arch` types anywhere —
the data structures are `lib/ds`'s own, calling the installed primitives.

## 2. The mechanism/value decision rule

**Anything effectful is a `kabi` primitive (reached through the installed
handle); anything that is pure layout-and-algorithm is a `lib/ds` data
structure.** `Instant::now()` is a monotonic-clock read — a syscall — so it
is a handle primitive. `Seq::push` is a growth algorithm whose only effectful
step (allocation) calls the handle's allocator; the algorithm itself is
`lib/ds`. The earlier draft that put the value *types* in the contract as a
re-export of `arch` was wrong by this rule — the type is `lib/ds`'s, only its
backend is the contract's.

This keeps the `lib/ds` value types **concrete and monomorphic** —
`Seq`/`Map`/`Mutex` fast paths have zero indirection — while routing only the
genuinely effectful primitives (alloc/park/clock/io) through the handle.

## 3. The platform vtable (`#[repr(C)]`, `static`, boot-installed)

The handle is a **`#[repr(C)]` versioned vtable of `extern "C"` function
pointers** — a "platform syscall table" — built as a **`static`** and
installed at boot, macro-generated (the same shape as `declare_*_driver!`).
It is read through a global accessor in `kabi` on every effectful call.

This deliberately rejects a trait object or a generic:

- **One vtable doctrine.** Drivers and modules already cross their seam via
  `REOVIM_*_VTABLE` (`06-ABI/02-Versioning-and-Vtables.md`). The platform
  seam being the same mechanism unifies the system: platform vtable
  (kernel↔machine), driver vtable (kernel↔device), module vtable
  (kernel↔policy).
- **`no_std`-clean** under `DAG6` (1.2 §10) — no `Debug` bound, no
  `Box`/alloc, no fat pointer.
- **Composes at runtime by construction** — Over-OS installs `arch`'s table;
  RTOS installs the system kernel's table; *same kernel binary*. This is the
  mechanical guarantee of the §0 mode invariant (mode-as-swap;
  `01-Architecture/06-OS-Modes.md` §6).
- **Versioned from day one** — the seam carries a `VtableHeader`
  (`06-ABI/02 §2`), so "the kernel is unchanged when the provider changes" is
  enforced by the same add-only `size_of_self` discipline as the driver ABI
  (AB3). A primitive slot under-specified against one backend is **appended**,
  not broken — which is why the contract is built straight rather than staged
  behind a second provider.
- Cost = one indirect call per *effectful* primitive (alloc/park/clock/io);
  the `lib/ds` value types stay monomorphic. For clock/park/io this is
  syscall-equivalent. For `alloc` the fast path is on-core, so the indirection
  is real (see §4) — accepted as the price of the swappability invariant, paid
  once per allocation, never on the DS fast paths.

A generic `Kernel<P>` was the alternative; rejected — it is viral through
every kernel type and forecloses runtime composition (back to cfg-per-target).

### 3.1 Lifecycle: same form, different teardown

The platform vtable shares the `#[repr(C)]`-versioned *shape* of the driver
and module vtables but not their teardown machinery:

| Seam | Provider count | Install | Hot-unload | Generation fence |
|---|---|---|---|---|
| **platform vtable** | one, per mode | boot-installed, `static` | no | no |
| **driver / module vtable** | many, per cdylib | runtime-loaded | yes (`02-Process/05-Machine-Boot.md` §5) | yes |

The machine floor cannot vanish under a running kernel, so the platform seam
needs no revocation path. Only the seams whose provider can disappear at
runtime carry generation fencing.

### 3.2 The contract owns the global handle static and `AllocError`

The boot-installed table is read by `lib/ds` on every effectful op, so the
**global platform-handle `static` lives in `kabi`** (write-once atomic, AB12
shape), never in `arch` — if it lived in `arch`, `lib/ds` would have to name
`arch` to read it, puncturing the firewall (`01-Architecture/02` §10, the
`lib/ds ⊄ arch` rule). `AllocError` — the allocator primitive's error type,
surfaced by every `lib/ds` allocation — likewise lives in `kabi`, so `lib/ds`
surfaces `kabi::AllocError`, not `arch::`.

### 3.3 Bootstrap: the `static` dissolves the chicken-egg

A naive worry: `arch` must allocate to build the handle, but `lib/ds` needs
the handle installed before it can allocate. It dissolves:

- The vtable is a `static` of function pointers — **zero heap to build**;
  install is one atomic pointer store.
- `arch`'s allocator initializes **heap-free** (it *is* the heap source: a
  static arena + direct `mmap`-class syscall), ready before the table is built.
- Boot order: `_start` → `arch` inits the allocator (heap-free) → build the
  `static` vtable → install it → *only now* may any `lib/ds` DS be constructed.
- **Invariant:** no `lib/ds` DS op before `Init::boot` installs the handle — a
  by-construction boot-proof prerequisite (`02-Process/05-Machine-Boot.md`),
  not a runtime guard.

`arch` reaches its allocator **directly** (a provider using its own impl);
`lib/ds` reaches it **through the handle** (a consumer via the contract). One
allocator, two access paths — not a duplicate.

### 3.4 Exception: panic seams stay write-once globals

The panic handler has no `&self`, so the four panic hooks remain write-once
atomic statics, exactly as today (AB12, `06-ABI/02 §5.2`). Everything else
effectful lives in the vtable.

## 4. What the decoupling buys (stated honestly)

The mechanism model — `arch → kabi`, no facade re-export — buys, concretely:

- ✅ the **airlock direction is correct**: the contract never depends on its
  implementor; the Math kernels' *direct* dependency set is World-free (the
  firewall, `01-Architecture/02` §11). Resolved-closure still reaches `arch`
  transitively via `lib/ds`'s backend — the firewall is a *direct-edge* rule,
  stated at the strength it holds.
- ✅ **provider swappability including the allocator**: when a second backend
  exists (RTOS), the same `lib/ds` DS run on it by installing a different
  table — no kernel recompile. The swappability invariant applied to the floor
  itself.
- ❌ not free: every effectful primitive is one indirect call. For clock/park/io
  this is syscall-equivalent. For `alloc` the fast path is on-core, so the
  indirection is real overhead — accepted as the price of allocator-swappability,
  paid once per allocation (not per element), and never on the DS fast paths
  (CAS, in-bounds index, non-growing push).

The honest accounting: the contract buys (1) a single edit-point, (2) a
depgraph-enforceable "kernels name only `kabi::*` + `lib/ds`" firewall, and
(3) runtime swappability of every World primitive including the allocator.
That is the real, bounded value.

## 5. Where it lives

`kabi/platform`, a down-face contract tier (`01-Architecture/02` §1)
mirroring `uapi/` on the up face, under the same add-only, versioned
discipline. The device-access seam is a *separate* contract, `kabi/device`
(`04-Domain-Substrate/06-Device-Domains.md` §8). The DS algorithms are
`lib/ds`, not part of the contract — the contract carries only the effectful
primitives those DS call.

## Open items

1. Does the contract ship as its own crate (`kabi/platform`) from day one, or
   start as a `platform` module split out when the system kernel must
   implement against it independently?
2. Which effectful primitives convert to the handle first. `clock` / `park`
   already have two backends (Linux futex + bare-metal timer/WFI) and lead;
   `alloc` converts in the same DS-split flight — the seam is canonical
   (`alloc(Layout) → ptr`) and AB3 append-only makes any under-spec
   reversible, so it is built straight, not staged behind provider 2.

## Conformance

| Behaviour | Fixture |
|---|---|
| Mechanism, not re-export | `kabi/platform` names no `arch` symbol; the depgraph probe rejects a `kabi → arch` edge. |
| DS reach the backend via the handle | a `lib/ds` allocation routes through the installed vtable, not a direct `arch::alloc` call; `lib/ds` names no `arch` symbol. |
| One binary, two providers | the same kernel binary links an `arch`-hosted table and a system-kernel table with no `cfg` per target; mode is a boot-time table install. |
| Header discipline | the platform vtable begins with `VtableHeader`; a provider that grows a slot is read through `size_of_self` by an older kernel (AB3 reuse). |
| Bootstrap order | the platform vtable is a `static` built without heap; no `lib/ds` DS is constructed before `Init::boot` installs the handle. |
| No direct `arch` edge | no kernel or `lib/ds` crate names `arch::*`; the depgraph probe rejects `*-kernel → arch` and `lib/ds → arch`. |
