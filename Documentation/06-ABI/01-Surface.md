# 6.1 — ABI Surface

**Scope.** What constitutes the ABI surface: the boundary every
runtime-loaded cdylib crosses, what kinds of types are allowed at
that boundary, what the kernel pins and what it lets the cdylib
choose.

**Heritage.** v3 `06-ABI/01-Surface.md`; v4 README §2 (rules 6, 5); #789 static registry loader.

**Locked rules.** Carried from v3; references `AB*`.

---

## 1. The boundary

Two contracts live at the ABI boundary:

- **Loader contract** — the vtable contract a module must satisfy to
  be loaded: vtable symbol(s) or registry entry, config symbols
  (CFG1), manifest path. The vtable contract (header read,
  `size_of_self` guard, version gates — 6.2 §2) applies identically
  to both conforming loader realizations:

  1. **Dynamic loader** — `dlopen`-based. The kernel locates the
     cdylib at the library-root path, calls `dlopen`, reads the
     exported vtable symbol, and applies the 6.2 §2 load rules.
     See 6.2 §2.1.

  2. **Static registry** — vtables embedded in a known linker section
     and walked at boot. The kernel iterates the section to discover
     and register each module's vtable before any module reaches
     `Active` state. The vtable contract is identical; no `dlopen`
     call is made. The in-tree `arch_test!` distributed-slice pattern
     (arch testrt) is prior art for this mechanism; it is not a
     normative dependency. Load and unload semantics for statically
     registered modules are defined in LF17 (2.2 §3).

- **Call contract** — the rules every function pointer obeys:
  parameter types, ownership, error convention, panic isolation.
  See AB12.

## 2. Allowed types at the boundary

| Allowed | Forbidden |
|---|---|
| `#[repr(C)]` structs | `#[repr(Rust)]` structs |
| `#[repr(C)]` unions with sibling discriminator | bare unions |
| `#[repr(transparent)]` newtypes over allowed primitives | |
| Primitive scalars (`u8`..=`u64`, `i8`..=`i64`, `f32`, `f64`, `bool`); `usize`/`isize` for lengths and sizes (= `size_t`/`ptrdiff_t`) | wider than 64-bit primitives; C aliases (`c_int`, `c_long`, `c_char`) — use explicitly sized types (AB14 retired the last `c_int` surface) |
| `*const c_void` / `*mut c_void` opaque handles | Rust reference types (`&T`, `&mut T`) |
| `*const u8` / `*mut u8` byte slices with explicit length | `&[T]`, `&str` |
| Function pointers `extern "C" fn(...)` | `fn(...)` (Rust ABI) |
| Stable interned identifiers (`DomainId(NonZeroU32)`, `CdylibId`, …) over `#[repr(transparent)]` | bare integers without a typed wrapper |
| Bytes for serialised payloads (UTF-8, canonical TOML, in-repo byte codec) | language-specific encodings |
| Standard fixed-size byte arrays (`[u8; N]`) | dynamic-size arrays |

## 3. Forbidden categories

- **Rust trait objects** (`Box<dyn T>`, `&dyn T`, `dyn T`) never
  cross the boundary.
- **Rust generics** never appear in `extern "C"` signatures.
- **Shared-reference types** (the `arch/`-provided `Arc`
  equivalent, 1.2 §10) as such never cross; opaque
  reference-counted handles use HostApi-managed counts.
- **`core::any::TypeId`** never crosses.
- **`enum` without `#[repr(...)]`** never crosses; tagged unions
  use `#[repr(C, u8)]` or a sibling discriminator.

## 4. Kernel-pinned vs cdylib-chosen

| Pinned by kernel | Chosen by cdylib |
|---|---|
| `VtableHeader` shape (AB3) | vtable body (after header) |
| `AbiVersion` major rejection rule | declared `AbiVersion` value |
| `ConfigSlice` layout (CFG6) | config field set |
| Carrier header layout (CR2) | content bytes |
| Error convention (`ErrorCode`) | error semantics |
| Panic isolation (AB12) | what gets panic-contained |

## 5. ABI generation strategy

The single source of `#[repr(C)]` layouts is
`06-ABI/03-Type-Catalog.md`. Generated artefacts:

- `uapi/<crate>/src/*.rs` — Rust mirror.
- Generated C header (TBD generator).
- Golden offset/size tests — bind both (CF4).

## 6. Cdylib-to-cdylib direct calls

Forbidden. All cross-cdylib communication goes through:
- ServiceRegistry (3.3) for typed services,
- DomainRouter (4.1) for handler/projector dispatch,
- StreamRuntime (4.4) for stream substrate.

This keeps every dynamic call routed through the kernel's
ownership and lifecycle machinery.

## Open items

1. Whether `f32` is allowed at the boundary (currently in §2 row);
   IEEE 754 cross-platform invariants are fine, but future ABI may
   reduce to `f64` only for simplicity.
2. ~~C header generator choice~~ — resolved #782: hand-rolled
   header generator in in-repo `tools/`; no third-party generator
   (`DAG5`).
3. Whether the boundary supports caller-owned-mutable buffers
   (e.g. an `ErrorBuf*` filled by the cdylib). Default: yes
   (mentioned in 6.2).

## Conformance

| Behaviour | Fixture |
|---|---|
| Forbidden types | CI grep for `Box<dyn` / `Arc<` / `&str` / `&[` in `extern "C"` signatures across `uapi/` crates. |
| Allowed types | Sample vtable per category; golden offset/size test. |
| Static registry (LF17) | Boot fixture: section-walk discovers a statically registered vtable; vtable contract checks (header, `size_of_self`, version gates) run identically to the dynamic-loader path. Review-class (no fixture until static-registry implementation lands). |
