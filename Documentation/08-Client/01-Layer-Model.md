# 8.1 — Client Layer Model

**Scope.** The closed `client/lib/subsys/*` contract tier and the
open `client/{platforms,drivers,modules,capabilities}/*`
implementation tier. Allowed cross-category edges and the role of
the launcher.

**Heritage.** the client-layer-model RFC (folded).

**Locked rules.** `CL1..CL5` here; `CL6..CL10` in 8.3.

---

## 1. Tiers

```
apps/*
  └─ client/platforms/<p>/            # platform runtime host
      └─ client/lib/subsys/*          # closed client contracts
      └─ runtime-loaded client drivers/modules/capabilities
```

## 2. Subsys contract tier

```
client/lib/subsys/
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

> **CL1 — Subsys is closed-contract.** `client/lib/subsys/*` MUST
> NOT depend on any `client/*` open extensions. *Class*: depgraph (CI).

## 3. Implementation tier

```
client/
├── platforms/<p>/         # platform runtime + platform-core
│                          # (e.g. tui, web, native)
├── drivers/<d>/           # impls of subsys contracts
├── modules/<m>/           # client-side policy modules
└── capabilities/<k>/      # capability impls (cell, dom, ...)
```

> **CL2 — Implementation tier is the open extension surface.**
> *Class*: spec.

Naming rules:

- "driver" on the client side means **open extension implementing
  a subsys contract** (same meaning as server).
- "subsys" means **closed-contract tier**.
- Flat: `client/<category>/<crate>/`. No platform-nested
  sub-paths.
- Crate naming: `reovim-client-<tier>-<name>` where `<tier>` is
  `subsys` or `ext`.

## 4. Allowed cross-category edges

Per `01-Architecture/02-Project-Layout-and-DAG.md` §2:

```
apps/reovim                → client/platforms/<p>/
client/platforms/<p>/      → client/lib/subsys/*/
client/drivers/<d>/        → client/lib/subsys/*/
client/drivers/<d>/        → client/platforms/<p>/
client/drivers/<d>/        → client/capabilities/<k>/
client/drivers/<d1>/       → client/drivers/<d2>/      (sub-DAG)
client/capabilities/<k>/   → client/lib/subsys/*/
client/capabilities/<k1>/  → client/capabilities/<k2>/ (sub-DAG)
client/modules/<m>/        → client/lib/subsys/*/
client/modules/<m>/        → client/capabilities/<k>/
client/modules/<m>/        → client/platforms/<p>/
client/modules/<m>/        → client/drivers/<d>/
client/modules/<m1>/       → client/modules/<m2>/      (sub-DAG)
```

## 5. Forbidden cross-category edges

```
client/platforms/*      → client/drivers/*
client/platforms/*      → client/modules/*
client/platforms/*      → client/capabilities/*
client/capabilities/*   → client/drivers/*
client/capabilities/*   → client/modules/*
client/capabilities/*   → client/platforms/*
client/drivers/*        → client/modules/*
client/lib/subsys/*     → client/*
apps/reovim         → client/lib/subsys/*
apps/reovim         → client/{drivers,modules,capabilities}/*
```

> **CL9 — Forbidden client cross-category edges.** *Class*:
> depgraph (CI).

## 6. Platform runtime ownership

> **CL3 — Platform runtimes own client registries.** The EditorCore does not
> own the client-side `ClientRuntime`. In subprocess mode the
> client process owns its own runtime; in embedded mode the
> launcher process holds the EditorCore and the `ClientRuntime` as
> separately-rooted structures.
>
> *Class*: spec.

> **CL5 — The EditorCore routes opaque client traffic; does not decode.**
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
   separate web SSR runner. **Out of target** per README.
2. Whether modules can register capabilities they consume from
   sibling modules — current spec disallows; clarify in 8.3.
3. Heritage banner for `archive/docs/architecture/client/` v6.3 docs
   (Phase G of #753).

## Conformance

| Rule | Fixture |
|---|---|
| CL1 | Probe over `client/lib/subsys/*`; any dep on `client/*` open extensions → fail. |
| CL2 | Crate at `client/<unknown-category>/x` → fail. |
| CL3 | Embedded launch shows separate `ClientRuntime` and `EditorCore` ownership. |
| CL4 | Render driver cdylib without `debug` vtable → debug surface not provided by it. |
| CL5 | Server logs show debug payloads passed through opaque (no framed-protocol field decoding of capability bodies). |
| CL8 | Inventory listing in embedded mode tags each cdylib server/client. |
| CL9 | Probe fixtures per forbidden edge (§5). |
