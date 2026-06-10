# 4.5 — Coordination

**Scope.** Position and cursor carriers, the codec contract, ingress
validation, persistence, client unknown-codec fallback, and resource
ceilings.

**Heritage.** the coordination RFC (folded with review
amendments); v4 README §11.

**Locked rules.** `CR1..CR19`.

---

## 1. Carrier model

```rust
#[repr(C)]
pub struct PositionHeader { pub bytes: [u8; 8] }

#[repr(C)]
pub struct CursorHeader   { pub bytes: [u8; 8] }

pub struct PositionCarrier {
    pub header:  PositionHeader,
    pub content: Vec<u8>,
}

pub struct CursorCarrier {
    pub header:  CursorHeader,
    pub content: Vec<u8>,
}
```

`PositionHeader` and `CursorHeader` are distinct types even though
they share bytes — prevents accidental cross-use.

## 2. Header layout

```
offset  size  field
0       4     domain_id (u32 little-endian, NonZeroU32 for non-empty)
4       2     inner_id  (u16 little-endian)
6       2     flags     (u16 little-endian)
                bit 0x8000: reserved for kernel annotation
                bits 0..14: codec-defined
```

> **CR1 — Carrier model: `header(8) + content(N)`.** *Class*: ABI.
> **CR2 — Header layout is little-endian `u32 / u16 / u16`;
> `flags 0x8000` reserved for kernel annotation.** *Class*: ABI.

## 3. No magic sentinel

> **CR3 — No magic sentinel; absence is API-level.** Empty cursor
> set is a zero-length list, not a "16-byte all-zero blob".
> Optional fields use `None` / NULL / absent proto field.
> *Class*: spec-asserted.

> **CR4 — `domain_id == 0` is invalid for non-empty carriers.**
> Validation rejects at ingress. *Class*: kernel-enforced.

## 4. Codec contract

A `(domain_id, inner_id)` codec implements:

```c
typedef struct {
    int  (*validate)(const u8* content, usize len);     // 0 = ok
    int  (*equal)   (const u8* a, usize alen,
                     const u8* b, usize blen);           // 1 = equal, 0 = not
    int  (*display) (const u8* content, usize len,
                     ByteBuf** out_utf8);               // for UI placeholders
    int  (*persist_canonicalise)(const u8* content, usize len,
                                  ByteBuf** out);
} CoordCodecVtable;
```

Each codec MUST document, in its module's docs:

- content versioning scheme,
- fixed/variable length,
- endian rules,
- padding/alignment rules,
- validation invariants,
- maximum content length,
- persistence migration behaviour.

> **CR7 — Codec contract.** Vtable + per-codec content-encoding doc
> required at registration. *Class*: spec-asserted.

Spec examples use **explicit little-endian serialisation**, not
`transmute` of `repr(C)` structs, for persistent/wire content.

## 5. Equality

> **CR5 — Carrier `PartialEq` is byte equality.** Codec
> `equal_or_null` is opt-in for named semantic algorithms (cursor
> aggregation/dedup); never used by `PartialEq`.
> *Class*: ABI.

This avoids surprising behaviour where two equal Rust values appear
unequal because no codec is registered.

## 6. Ingress validation

> **CR6 — Ingress validation runs at every boundary.** Carriers are
> validated when they enter kernel state from:
> - handler output,
> - gRPC / client input,
> - persistence restore,
> - module/service APIs,
> - test/fuzz fixtures.
>
> Invalid carriers are rejected with a named error and DS12 event.
> *Class*: runtime + tests.

## 7. Carrier status

```rust
pub enum CarrierStatus {
    ValidKnown,    // codec registered; validation passed
    ValidOpaque,   // codec unavailable; bytes structurally well-framed
    Invalid,       // malformed header/content or rejected
}
```

> **CR8 — Carrier status: `ValidKnown` / `ValidOpaque` / `Invalid`.**
>
> `ValidOpaque` rules:
> - storage bounded by `max-opaque-carrier-bytes-per-session`;
> - byte equality only;
> - persistence writes back unchanged with stable Domain name mapping;
> - server-client protocol broadcasts with unknown-codec marker;
> - clients MUST NOT issue semantic operations against it;
> - revalidates if matching codec later registers.
>
> *Class*: kernel-enforced.

## 8. Persistence

> **CR9 — Persistence stores stable Domain names, not numeric
> `DomainId`.** Persisted carrier rows include `(domain_name,
> inner_id, flags, content_b64)` plus codec-version metadata.
> Restore remaps name → fresh `DomainId` via `DomainRouter.intern_named`.
> *Class*: spec-asserted.

Cursor and viewport persistence stores base64 carrier blobs, not
`cursor_byte` / `viewport_top` integers.

## 9. Client unknown-codec fallback

> **CR10 — Client unknown-codec fallback.** A client lacking a local
> codec for a `(DomainId, inner_id)` MUST:
> - not interpret the bytes,
> - hide the cursor or display a generic marker,
> - optionally request server-rendered display text,
> - log/report unsupported domain/codec in debug surfaces.
>
> Clients MUST NOT depend on server-rendered display for editing
> semantics.
> *Class*: spec-asserted.

The server-client protocol carries a `DomainTable` after attach and
on codec deltas (see 7.3).

## 10. Bounded resources (CR11..CR19)

| Rule | Limit | Field |
|---|---|---|
| CR11 | Max `PositionCarrier.content` bytes | `max-position-carrier-bytes` |
| CR12 | Max `CursorCarrier.content` bytes | `max-cursor-carrier-bytes` |
| CR13 | Max cursors per window | `max-cursors-per-window` |
| CR14 | Max opaque carrier bytes per session | `max-opaque-carrier-bytes-per-session` |
| CR15 | Max deferred-restore carriers | `max-deferred-restore-carriers` |
| CR16 | Max codecs per Domain | `max-codecs-per-domain` |
| CR17 | Max DomainTable entries per attach | `max-domain-table-entries` |
| CR18 | Max display-text size returned by codec | `max-codec-display-bytes` |
| CR19 | Max validation-error rate per source | `max-invalid-carrier-per-min` |

Codec-declared per-content limits MUST be ≤ kernel limits.
Breaches return `ErrorCode::ResourceExhausted` and emit
rate-limited DS12 events keyed by source.

## 11. Lifecycle

When a codec's owning cdylib unloads:

1. carriers in `ValidKnown` for that codec transition to
   `ValidOpaque`;
2. broadcasts to clients carry the unknown-codec marker;
3. semantic operations (compare, aggregate) on those carriers
   return `ErrorCode::CodecGone`.

If the same `(DomainId, inner_id)` codec re-registers later,
ingress revalidates the affected carriers; on success, status
returns to `ValidKnown`.

## Open items

1. Whether `flags` reserves bits for content-version (vs requiring
   codec-internal versioning). Draft default: codec-internal.
2. Whether `persist_canonicalise` is required or optional. Draft:
   optional; absent → carrier bytes persisted as-is.
3. Server-rendered display text policy — how often, what format,
   cache strategy. Draft: ASCII-art per cursor, no caching.

## Conformance

| Rule | Fixture |
|---|---|
| CR1, CR2 | Header byte-layout golden test. |
| CR3 | Empty cursor list emits zero-length proto field, never a sentinel blob. |
| CR4 | `domain_id == 0` carrier rejected at every ingress. |
| CR5 | `PartialEq` returns true for byte-equal carriers without any codec registered. |
| CR6 | Fuzz fixture sends invalid carriers via gRPC; kernel rejects + DS12 emits. |
| CR7 | Codec docs include all required fields; CI gate. |
| CR8 | Codec unloads → carriers go ValidOpaque; codec re-loads → revalidate path. |
| CR9 | Persist with `DomainId` differing across runs; cursors restore. |
| CR10 | Client without codec receives carriers; verifies fallback marker; verifies server-rendered display request. |
| CR11..CR19 | Per-cap fixtures. |
