# 4.1 — Domain

**Scope.** What a Domain is in v4: an identifier plus routing
entries. Removal of `trait Domain` from v3 prior art. The intern
contract for stable Domain names.

**Heritage.** v3 `04-Substrate/01-Domain.md`; v4 README §2 (rule 3).

**Locked rules.** `DT10` (priority bands, carried — restated §6),
`DT17` (dispatch tie-break); references `DT*`, `CR*`.

---

## 1. Domain is an ID

```rust
pub struct DomainId(NonZeroU32);
```

Behaviour for a Domain is **registered** in tables, not implemented
on a trait. The kernel knows three things about a `DomainId`:

- the stable name string it was interned from,
- a `DomainManifest` published by its owning cdylib,
- routing table entries: handlers, projectors, codecs (position,
  cursor), persistence handlers, positional-arg dispatch handlers.

There is no `trait Domain` in v4. The v3 `trait Domain` is
removed.

## 2. DomainRouter

```rust
pub struct DomainRouter {
    pub names_to_ids:    HashMap<Arc<str>, DomainId>,
    pub ids_to_names:    HashMap<DomainId, Arc<str>>,
    pub manifests:       HashMap<DomainId, DomainManifest>,

    pub handlers:        HashMap<(DomainId, HandlerId), Vec<HandlerEntry>>,
    pub projectors:      HashMap<(DomainId, ProjectorId), Vec<ProjectorEntry>>,

    pub position_codecs: HashMap<(DomainId, u16), PositionCodecEntry>,
    pub cursor_codecs:   HashMap<(DomainId, u16), CursorCodecEntry>,
}
```

Codec maps are part of `DomainRouter`, not a separate
`CoordinationRegistry` (v3 had this; v4 removed it per README §5.1).

## 3. Intern contract

```rust
impl DomainRouter {
    /// Interns a stable Domain name. Idempotent for the lifetime
    /// of the process. Returns the same DomainId across calls
    /// with the same name.
    pub fn intern_named(&mut self, name: &str) -> DomainId;

    /// Reverse lookup. Returns the name an ID was interned from.
    pub fn name_of(&self, id: DomainId) -> Arc<str>;
}
```

- `intern_named` allocates a fresh `DomainId` on first call, returns
  the same one on subsequent calls.
- Numeric `DomainId` is allocation-order-dependent across runs;
  persistence stores names (CR9).

## 4. DomainManifest

```rust
pub struct DomainManifest {
    pub id:                DomainId,
    pub name:              Arc<str>,
    pub owner_cdylib_id:   CdylibId,
    pub api_version:       Version,
    pub handler_kinds:     Vec<HandlerId>,
    pub projector_kinds:   Vec<ProjectorId>,
    pub position_codecs:   Vec<u16>,    // inner_id values registered
    pub cursor_codecs:     Vec<u16>,    // inner_id values registered
    pub positional_args:   Option<PositionalArgHandler>,  // AL10
}
```

Manifest is published at participant init; entries are torn down at
unload.

## 5. HandlerId / ProjectorId

```rust
pub enum HandlerId {
    OnRawInput,
    OnAttach,
    OnDetach,
    OnPersistSave,
    OnPersistLoad,
    OnPositionalArg,            // AL10
    Custom(u32),                // module-defined; cleared at unload
}
```

`Custom` handler IDs are scoped per-cdylib; collisions across
cdylibs are not visible (they sit in different rows keyed by
`HandlerId::Custom(n)` *and* the per-row `owner_cdylib_id`).

Projectors mirror this shape:

```rust
pub enum ProjectorId {
    Render,
    StatusLine,
    Custom(u32),
}
```

## 6. Routing rules

- Lookup is `(DomainId, HandlerId) → Vec<HandlerEntry>`.
- Each `HandlerEntry` carries `(band, priority, callable, owner_cdylib_id)`.
- Dispatch order is `priority` (descending; the DT10 band ranges
  make band ordering implicit), then the DT17 tie-break.

> **DT10 (carried) — Priority bands for handler/projector
> registration.** Priority is a `u32`; higher numbers run first.
> The range is partitioned into five convention bands:
>
> | Band | Range | Use |
> |---|---|---|
> | `Pre`   | 900..=999 | Run BEFORE base behaviour; veto-style hooks |
> | `Base`  | 500..=899 | Default band; mode dispatch, core handlers |
> | `Adapt` | 300..=499 | Adapter / shim layer (accessibility, telemetry) |
> | `Decor` | 100..=299 | Decoration / view-only effects (statusline) |
> | `Post`  | 0..=99    | Run AFTER everything; cleanup, observation-only |
>
> The bands are convention; the kernel enforces only the `u32`
> ordering. Manifest authors declare a band name in `[handlers]` /
> `[projectors]`; the macro picks the band's mid-range default
> (Pre=950, Base=700, Adapt=400, Decor=200, Post=50). An explicit
> `priority = N` outside the named band's range is a hard error at
> manifest validation. A reserved `pre_phase: u8` manifest slot
> covers a future event-phase model without a major bump; it is
> not interpreted in v1. The numeric ranges are forever (AB15);
> the names are convenience — adding a band is a manifest-schema
> minor, renaming one is a major.
> *Class*: spec-asserted + manifest validation.

> **DT17 — Dispatch tie-break is total and deterministic.** When
> two entries for the same `(DomainId, HandlerId)` (or
> `ProjectorId`) share `(band, priority)`, order is:
>
> 1. **Across cdylibs**: lockfile entry order. The lockfile is
>    canonically sorted by `(kind, name)` (7.1 §5), so this is
>    lexicographic, reproducible from the same package set, and
>    independent of install history.
> 2. **Within one cdylib**: manifest declaration order — the
>    author's stated intent.
>
> The resulting order is total: no two entries are unordered.
> The tie-break is universal — the same rule applies to handlers,
> projectors, and any future banded dispatch.
> *Class*: runtime + conformance fixture.

Authors who need a *specific* cross-cdylib order should use
`priority`, not name games; the tie-break exists so that equal
priorities are still deterministic, not as an ordering API.


## 7. Forbidden

- A `DomainId` may not be cast from / to integers outside
  `DomainRouter` operations.
- Numeric `DomainId` does not appear in persistence, network, or
  log artefacts as the durable identity (CR9).
- A cdylib may not register handlers under a `DomainId` it does
  not own.

## Open items

1. ~~Tie-break ordering~~ — resolved (§6, DT17): lockfile
   canonical order across cdylibs, manifest declaration order
   within a cdylib.
2. Whether Domain manifests must declare every handler kind they
   register, or whether late registration during init is allowed.
   Default: allowed; manifest is informational.

## Conformance

| Behaviour | Fixture |
|---|---|
| Intern stability | `intern_named("text")` twice → same `DomainId`. |
| Codec scoping | Module registers `(DomainId, inner_id=0x42)` and unloads → row revoked; lookup returns `NotFound`. |
| Persistence remap | Persist with `DomainId(7)`, restart, reload → router maps name to fresh `DomainId(N)`; carriers restored. |
| DT10 | Manifest declares `band = "decor", priority = 950` → manifest validation hard-errors (out of band range). Band-name-only declaration → mid-range default assigned. |
| DT17 | Three modules register same `(band, priority)` handlers; dispatch order matches lockfile `(kind, name)` order; two handlers in one manifest dispatch in declaration order; order identical across repeated boots. |
