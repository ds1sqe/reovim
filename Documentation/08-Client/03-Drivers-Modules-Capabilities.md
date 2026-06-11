# 8.3 — Client Drivers, Modules, Capabilities

**Scope.** Concrete kinds for client-side cdylibs, multi-vtable
manifest support, lifecycle for sibling vtables, and the
RawInputKind enumeration as part of the type catalog.

**Heritage.** the client-layer-model RFC (folded).

**Locked rules.** `CL6`, `CL7`, `CL10`.

---

## 1. Client driver kinds

| Kind | Purpose | Vtable symbol |
|---|---|---|
| `client_render` | Rasterise projection to platform output | `REOVIM_CLIENT_RENDER_DRIVER_VTABLE` |
| `client_input` | Translate platform input → `RawInput` | `REOVIM_CLIENT_INPUT_DRIVER_VTABLE` |
| `client_display` | Manage display surfaces (scroll, resize) | `REOVIM_CLIENT_DISPLAY_DRIVER_VTABLE` |
| `client_chrome` | Statusline, gutter, frame chrome | `REOVIM_CLIENT_CHROME_DRIVER_VTABLE` |

Each driver kind has its own `Version` (api) trajectory inside the
shared `AbiVersion`.

## 2. Client module kinds

Modules are policy-bearing. Examples:

- `vim` — vim-mode keymaps and motions.
- `landing` — startup/landing screen.
- `theme` — theme application policy.
- `mru` — most-recently-used list.

Manifest kind `module-client`. Vtable symbol
`REOVIM_CLIENT_MODULE_VTABLE`.

## 3. Client capability kinds

Capabilities are reusable platform affordances:

| Kind | Purpose |
|---|---|
| `cell` | Cell-grid surface (TUI) |
| `cell-view` | Sub-cell rasteriser (Braille, HalfBlock) |
| `dom` | DOM-based surface (web) |
| `svg` | SVG-based surface (web) |
| `debug` | Client-side debug capability (8.4) |

Vtable symbols: `REOVIM_CAPABILITY_<KIND>_VTABLE`.

## 4. Multi-vtable cdylibs

> **CL6 — Multi-vtable cdylibs require explicit manifest
> `[[vtable]]` entries.** The loader does not scan symbols; it
> reads only what the manifest names.
>
> *Class*: runtime + spec.

```toml
# manifest/driver/render-tui.toml
[package]
name = "render-tui"
kind = "driver-client"
sub_kinds = ["render", "chrome"]    # informational

[[vtable]]
kind   = "client_render"
symbol = "REOVIM_CLIENT_RENDER_DRIVER_VTABLE"

[[vtable]]
kind   = "client_chrome"
symbol = "REOVIM_CLIENT_CHROME_DRIVER_VTABLE"
```

> **CL7 — One `CdylibId` owns all sibling vtables.**
> - Compatibility checks per vtable + the shared header.
> - Partial init failure rolls back ALL sibling registrations.
> - Unload drains all sibling RefGuards/leases before shutdown.
> - Manifest order does not imply dependency order; deps must be
>   declared explicitly.
>
> *Class*: runtime.

## 5. RawInputKind on the client side

> **CL10 — Client `RawInputKind` enumeration is part of the type
> catalog.** Client input drivers translate platform events into
> `RawInput { kind, payload }` per 6.3 §7. Custom kinds (`128..=255`)
> require schema declaration and a stable assigned ID across the
> ecosystem.
>
> *Class*: ABI / spec.

## 6. Lifecycle

Same model as server-side cdylibs (2.2):

```
dlopen → Loaded → init → Active → Draining → Shutting → Unloaded
```

Plus per-vtable row state machines mirroring 2.2 §5:
`PublicActive → DrainingHidden → Destroying → Revoked`. CL7 makes
"unload" atomic across sibling vtables.

## 7. RefGuards across sibling vtables

When a `CdylibId` owns N vtables, the inventory holds N row
groups but **one** owner generation. Acquiring a RefGuard for any
one vtable bumps the same owner refcount; unload waits for the
sum.

## 8. Driver-ABI-v1 migration

Existing single-vtable driver-ABI-v1 cdylibs continue to work via
the compatibility loader (6.2 §6). Their manifests carry exactly
one `[[vtable]]` entry. The migration to multi-vtable manifests
is opt-in per cdylib.

## Open items

1. Sub-kind enumeration policy — when a new `client_render` sub-kind
   appears, does it require an `AbiVersion` minor or major bump?
   Default: minor (additive, with `size_of_self` guard).
2. Capability kind allocation — registered globally or per-platform?
   Default: globally, with the `cell`/`cell-view`/`dom`/`svg` set
   reserved.
3. Whether modules can register capability vtables (vs only drivers).
   Default: yes — modules MAY ship a capability if it's policy-bound.

## Conformance

| Rule | Fixture |
|---|---|
| CL6 | Cdylib exporting an unmanifested vtable symbol → not loaded; manifested vtable → loaded. |
| CL7 | Multi-vtable cdylib with init failure on second vtable → first is rolled back; cdylib transitions to FailedInit. |
| CL10 | Custom RawInputKind 200 declared by capability → server dispatches; custom 200 from another cdylib without declaration → rejected. |
