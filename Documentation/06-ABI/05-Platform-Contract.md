# 6.5 — Platform Contract

**Scope.** The down-face seam the system bridge reaches its machine substrate
through — `kabi/platform`. The mechanism/implementation split (the contract
*declares*, provider code *implements*), the `#[repr(C)]` platform vtable
("platform syscall table") of effectful primitives, where the provider-facing
value-type layout lives, the boot-time `static` install and its lifecycle
relative to driver and module vtables, and the bounded value the decoupling
buys. The dependency-graph edges this contract implies (`system/lib/kernel →
kabi/platform`, upper product code → `uapi/*`, provider code → `kabi/*`,
never `→ arch` from above the provider) are normative in
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
of `06-ABI/02-Versioning-and-Vtables.md` (AB3, AB15). The behavioral-
conformance rule **AB16** is declared `spec-asserted` in
`09-Conformance/01-Rule-Matrix.md`; its fixtures land in the 05d sub-plans.

---

## 1. The contract is the airlock, and it points one way

Upper product code — including the editor and client kernels
(`02-Process/01-Kernel-Types.md` §0) — depends on the up-face `uapi/*`
contracts, not on `kabi/platform` and never on `arch`. `system/lib/kernel` is
the bridge that may name both `uapi/*` and `kabi/*`. Provider code below the
bridge implements `kabi/platform` and must not import `uapi/*` directly unless
a concrete inescapable exception is recorded in the DAG chapter.

This split is the line that keeps the editing mechanism byte-identical across
modes (`01-Architecture/06-OS-Modes.md` §0). It does not exist for
cleanliness; it exists to *guarantee* that invariant. Anything that makes
upper product code behave differently by mode is, by definition, a leak across
this line and a bug.

**Mechanism, not facade.** `kabi/platform` *declares* what a platform must
provide; `platform-*`, freestanding provider code, or other hardware-specific
providers implement it. Dependency flows provider → `kabi`, **never**
`kabi → provider`, and never provider → `uapi` by default. A contract that
imported its own implementor would run backward through the airlock. A lower
provider that imports the product's `uapi` vocabulary bypasses the system
bridge. The substrate splits by **what a thing is**:

| Part | What it is | Where it lives | Reached how |
|---|---|---|---|
| **Effectful primitives** | services that read the world or schedule — the allocator, `park`/`unpark`, clock, spawn, thread id, device/block I/O | declared in `kabi`, implemented by providers below the bridge | a `#[repr(C)]` vtable, installed at boot (§3) |
| **Value-type layout + algorithm** | the data structures — `Seq`/`Map`/`Bytes`/`Shared`, the `Mutex`/`RwLock`/`Condvar` state machines | `lib/ds` (Math; `01-Architecture/02` §10) | direct, monomorphic; their *backend* (alloc, park) is reached through the handle |

This is the Math/World airlock applied to the type system: a value's *layout
and algorithm* are identical on every provider (Math, `lib/ds`); the
*allocator and park/unpark behind it* are provider-specific (World,
provider/floor code), installed at boot. There is no re-export of `arch` or
`uapi` types from `kabi` for provider convenience. Provider-facing slot types
belong in `kabi`; product-facing values belong in `uapi`; conversion between
the two belongs in `system/lib/kernel`.

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

This deliberately rejects a `dyn` trait *seam* and a generic — though a
contract trait still *authors* the table (§3.7), it is never the runtime
seam:

- **One vtable doctrine.** Drivers and modules already cross their seam via
  `REOVIM_*_VTABLE` (`06-ABI/02-Versioning-and-Vtables.md`). The platform
  seam being the same mechanism unifies the system: platform vtable
  (system bridge↔machine provider), driver vtable (kernel↔device), module
  vtable (kernel↔policy).
- **`no_std`-clean** under `DAG6` (1.2 §10) — no `Debug` bound, no
  `Box`/alloc, no fat pointer.
- **Composes at runtime by construction** — Over-OS installs a hosted provider
  table; RTOS installs a freestanding provider table below the system bridge;
  *same editor/client kernel source*. This is the mechanical guarantee of the
  §0 mode invariant (mode-as-swap; `01-Architecture/06-OS-Modes.md` §6).
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

### 3.4 The fault floor is a separate seam: `kabi/panic`

The panic handler has no `&self`, so the panic hooks cannot be vtable slots:
they remain **write-once atomic statics**, exactly as today (AB12,
`06-ABI/02 §5.2`). These are not an exception bolted onto the platform
vtable — they are a distinct down-face seam, **`kabi/panic`**, the
always-present fault floor. It carries no install gate: a panic can fire
before `Init::boot` finishes installing the platform table, so the fault
hooks must exist unconditionally, unlike the install-gated `kabi/platform`
vtable. Everything else effectful lives in the platform vtable; fault
disposition lives in `kabi/panic`.

### 3.5 Slot signatures are provider-facing `kabi` types

Every effectful slot that carries a POSIX-shaped value takes and returns a
**`kabi/platform` provider-facing type**, never a product-facing POSIX type
and never an untyped integer when a slot-specific newtype exists: `Fd`,
`OpenFlags`, `Mode`, `Errno`, and the rest of the provider-facing set. A bare
`i32` flag or `errno` must not travel through a slot unless the ABI scalar
itself is the documented `#[repr(transparent)]` carrier.

There is no product-facing POSIX vocabulary crate. Product semantics live in
domain uapi leaves (`uapi::fs`, `uapi::net`, `uapi::terminal`, etc.).
`kabi/platform` owns the provider-visible slot vocabulary. The bridge between
domain uapi semantics and the down-face slot vocabulary lives in
`system/lib/kernel` in RTOS-itself mode, or in the hosted bridge/provider role
in Over-OS mode. Providers translate NATIVE machine effects to/from `kabi`
slot values; the bridge translates `kabi` to/from domain `uapi`. A provider
that imports `uapi/posix` directly bypasses that bridge and is forbidden unless
a documented inescapable exception exists.

### 3.6 Append-only vtable evolution

The "versioned, appended not broken" discipline above (AB3) is realized
concretely. A new primitive is a **nullable slot appended at the tail**,
paired with a per-slot **`HAS_*` presence const**. A consumer null-checks
the slot (or reads `HAS_*`) before calling; a provider that does not fill it
yields `Errno::ENOSYS`, never undefined behavior. Slots are never reordered,
retyped, or removed — only appended. An older kernel reads a newer provider's
table through `size_of_self` (§3) and simply never reaches the tail slots it
does not know. Every under-specified primitive is therefore reversible by
addition, which is the structural reason the contract is built straight
rather than staged behind a second provider.

### 3.7 Authoring: the `#[vtable]`-style ops macro

The `static` table is generated from a **contract trait** by an in-tree
`#[vtable]`-style macro — the Rust-for-Linux mechanism, reimplemented
in-tree with no external proc-macro dependency (DAG5). The macro lowers the
trait to the frozen `#[repr(C)]` table of `extern "C"` pointers plus the
per-slot `HAS_*` consts (§3.6) and the install glue. The trait is an
**authoring DSL only**: it never appears as a `dyn Trait` value and never
crosses the seam. "No trait" in the one-vtable doctrine (§3) means "no `dyn`
seam," not "no trait anywhere" — the trait is how a human writes the table;
the `#[repr(C)]` struct is what ships and what installs.

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

The honest accounting: the contract buys (1) a single down-face edit-point,
(2) a depgraph-enforceable "upper code names `uapi::*`, lower providers name
`kabi::*`, and only the system bridge names both" firewall, and (3) runtime
swappability of every World primitive including the allocator. That is the
real, bounded value.

## 5. Where it lives

`kabi/platform`, a down-face contract tier (`01-Architecture/02` §1)
mirroring `uapi/` on the up face, under the same add-only, versioned
discipline. The device-access seam is a *separate* contract, `kabi/device`
(`04-Domain-Substrate/06-Device-Domains.md` §8). The DS algorithms are
`lib/ds`, not part of the contract — the contract carries only the effectful
primitives those DS call.

### 5.1 The conformance suite defines "same behavior"

A provider is correct **iff it passes `platform-conformance`** — a
behavioral fixture suite that operationally *defines* what "same behavior
across providers" means. There is **no reference provider**: `kabi/platform`
owns the provider-facing slot shape and the suite owns the down-face behavior,
so the Linux provider is not privileged as the oracle. Product-visible domain
uapi semantics are checked at the bridge layer, not by letting providers import
`uapi` directly. A zero-arch mock (`platform-linux-mock`) passes the *same*
suite — the proof that "provider" is a contract role, not a synonym for the
Linux backend. The suite is
**append-only**: adding a fixture tightens the contract and can never
invalidate a previously conforming provider, so it ships from day one with
no premature-abstraction risk. The suite is normative now; its fixtures are
implemented in the 05d sub-plans. The gating rule is **AB16**
(`09-Conformance/01-Rule-Matrix.md`).

## Open items

1. Provider-facing slot vocabulary must stay owned by `kabi/platform` so lower
   providers do not import `uapi/posix`; any unavoidable exception must be
   recorded in `01-Architecture/02-Project-Layout-and-DAG.md`.
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
| Provider-facing slot types | every POSIX-shaped slot takes/returns a `kabi/platform` provider-facing type; product semantics stay in domain uapi leaves (§3.5). |
| Append-only evolution | a new primitive is a nullable tail slot guarded by a `HAS_*` const; an unfilled slot yields `Errno::ENOSYS`; slots are never reordered or removed (AB3, §3.6). |
| Behavioral conformance | a provider — including the zero-arch `platform-linux-mock` — is valid iff it passes the `platform-conformance` suite; no provider is the reference oracle (AB16, §5.1). |
| Fault floor always present | the `kabi/panic` hooks are write-once statics installable before `Init::boot` completes; they are not platform-vtable slots (§3.4). |
