# 8.1 — Client Layer Model

**Scope.** The closed `clients/lib/subsys/*` contract tier and the
open `ext/client/{platforms,driver,module,capabilities}/*`
implementation tier. Allowed cross-category edges and the role of
the launcher.

**Heritage.** the client-layer-model RFC (folded); v3
`.claude/rules/architecture.md` Client Layer Model v7; v4 README §13.

**Locked rules.** `CL1..CL5` here; `CL6..CL10` in 8.3.

---

## 1. Tiers

```
apps/*
  └─ ext/client/platforms/<p>/        # platform runtime host
      └─ clients/lib/subsys/*         # closed client contracts
      └─ runtime-loaded client drivers/modules/capabilities
```

## 2. Subsys contract tier

```
clients/lib/subsys/
├── module          ClientModule trait, ModuleContext, lifecycle
├── chrome          ChromeSurface trait, projection cache
├── render          RenderTarget trait, render-pipeline contracts
├── codec           codec registries, DomainProjection, from_wire()
├── capability      generic Capability trait, slot system
├── platform        module-side platform-requirement contract
├── protocol        notification-stream contracts; framed-protocol-facing traits
├── debug           client-side debug capability vtable
└── driver-loader   safe wrapper for runtime-loaded drivers
```

> **CL1 — Subsys is closed-contract.** `clients/lib/subsys/*` MUST
> NOT depend on any `ext/client/*`. *Class*: depgraph (CI).

## 3. Implementation tier

```
ext/client/
├── platforms/<p>/         # platform runtime + platform-core
│                          # (e.g. tui, web, native)
├── driver/<d>/            # impls of subsys contracts
├── module/<m>/            # client-side policy modules
└── capabilities/<k>/      # capability impls (cell, dom, ...)
```

> **CL2 — Implementation tier is the open extension surface.**
> *Class*: spec.

Naming rules (per `.claude/rules/architecture.md` §Naming):

- "driver" on the client side means **open extension implementing
  a subsys contract** (same meaning as server).
- "subsys" means **closed-contract tier**.
- Flat: `ext/client/<category>/<crate>/`. No platform-nested
  sub-paths.
- Crate naming: `reovim-client-<tier>-<name>` where `<tier>` is
  `subsys` or `ext`.

## 4. Allowed cross-category edges

Per `01-Architecture/02-Project-Layout-and-DAG.md` §2:

```
apps/reovim                → ext/client/platforms/<p>/
ext/client/platforms/<p>/  → clients/lib/subsys/*/
ext/client/driver/<d>/     → clients/lib/subsys/*/
ext/client/driver/<d>/     → ext/client/platforms/<p>/
ext/client/driver/<d>/     → ext/client/capabilities/<k>/
ext/client/driver/<d1>/    → ext/client/driver/<d2>/   (sub-DAG)
ext/client/capabilities/<k>/ → clients/lib/subsys/*/
ext/client/capabilities/<k1>/ → ext/client/capabilities/<k2>/ (sub-DAG)
ext/client/module/<m>/     → clients/lib/subsys/*/
ext/client/module/<m>/     → ext/client/capabilities/<k>/
ext/client/module/<m>/     → ext/client/platforms/<p>/
ext/client/module/<m>/     → ext/client/driver/<d>/
ext/client/module/<m1>/    → ext/client/module/<m2>/   (sub-DAG)
```

## 5. Forbidden cross-category edges

```
ext/client/platforms/*  → ext/client/driver/*
ext/client/platforms/*  → ext/client/module/*
ext/client/platforms/*  → ext/client/capabilities/*
ext/client/capabilities/* → ext/client/driver/*
ext/client/capabilities/* → ext/client/module/*
ext/client/capabilities/* → ext/client/platforms/*
ext/client/driver/*     → ext/client/module/*
clients/lib/subsys/*    → ext/client/*
apps/reovim         → clients/lib/subsys/*
apps/reovim         → ext/client/{driver,module,capabilities}/*
```

> **CL9 — Forbidden client cross-category edges.** *Class*:
> depgraph (CI).

## 6. Platform runtime ownership

> **CL3 — Platform runtimes own client registries.** The Kernel does not
> own the client-side `ClientRuntime`. In subprocess mode the
> client process owns its own runtime; in embedded mode the
> launcher process holds the Kernel and the `ClientRuntime` as
> separately-rooted structures.
>
> *Class*: spec.

> **CL5 — The Kernel routes opaque client traffic; does not decode.**
> Server-side has no compile-time knowledge of client capability
> semantics. *Class*: spec.

## 7. Client debug

> **CL4 — Client debug is a sibling capability vtable, not render
> slots.** A driver providing rendering does not double as the debug
> surface. Debug is its own capability cdylib with its own vtable.
> *Class*: spec.

(See 8.4 for the debug-capability contract.)

## 8. Inventory

> **CL8 — Inventory distinguishes server cdylibs from client
> cdylibs.** Each entry names which process / runtime owns it, so a
> single inventory listing across embedded mode is unambiguous.
> *Class*: runtime.

## Open items

1. Whether the WASM browser runtime is a client platform peer or a
   separate web SSR runner. **Out of v4 target** per README.
2. Whether modules can register capabilities they consume from
   sibling modules — current spec disallows; clarify in 8.3.
3. Heritage banner for `archive/docs/architecture/client/` v6.3 docs
   (Phase G of #753).

## Conformance

| Rule | Fixture |
|---|---|
| CL1 | Probe over `clients/lib/subsys/*`; any dep on `ext/client/*` → fail. |
| CL2 | Crate at `ext/client/<unknown-category>/x` → fail. |
| CL3 | Embedded launch shows separate `ClientRuntime` and `Kernel` ownership. |
| CL4 | Render driver cdylib without `debug` vtable → debug surface not provided by it. |
| CL5 | Server logs show debug payloads passed through opaque (no framed-protocol field decoding of capability bodies). |
| CL8 | Inventory listing in embedded mode tags each cdylib server/client. |
| CL9 | Probe fixtures per forbidden edge (§5). |
