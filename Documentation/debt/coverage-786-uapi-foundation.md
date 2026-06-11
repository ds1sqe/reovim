# Coverage Debt — #786 `uapi/` Foundation

Classified coverage residue for the frozen ABI catalog
(`uapi/abi`), the sans-IO framed-protocol codec (`uapi/protocol`),
and the declare macros under the 100% MC/DC gate. Classification
authority: the physical measurement limits clause in
`Documentation/10-Development/01-Testing.md` §2 (DEV1), applied here
to the input-magnitude class.

## Status

Measured over the `uapi-selftest` no_std binary (339 cases) plus the
fixture fleet, merged profraw, `llvm-cov`:

- `uapi/abi`: 100% (layout goldens + frame codec).
- `uapi/protocol`: `state.rs` and `view.rs` 100%; total 98.86%
  regions.
- No `coverage(off)` annotations; EXCLUDE list empty.

## Classified residue: input-magnitude class

Every remaining line-level miss is an Err arm whose trigger requires
an input exceeding the wire format's own 32-bit bounds,
unconstructible in a test process on the only supported target
(x86_64):

- `codec.rs` — `put_bytes` len > `u32::MAX` arm: needs a > 4 GiB
  slice.
- `frame.rs` — `write_frame` body > `u32::MAX` arm: same magnitude.
- `messages.rs` — the `u32_len(list.len())?` arms in
  `encode_body`/`encoded_size` of every list-carrying message: a
  list with more than 2^32 elements, plus probe-edge second regions
  co-located on the same lines.

The wire format's u32 count/length fields ARE the guard; these arms
exist precisely to refuse what the format cannot carry. Constructing
the triggering values would require allocating beyond the format's
own representable range.

## Restructures applied during closure (DEV1)

- `state.rs`: dead early Closed-return removed (the match owns it).
- `codec.rs` reserve/take and `frame.rs` total/end:
  `checked_add` → plain add with bound-invariant comments (slice
  bounds + u32-decoded lengths cannot wrap `usize` on x86_64); the
  guards' Err arms were unreachable.
- Full truncation sweep (every k < `encoded_size`, both directions)
  on every message including non-empty-list values — drives all
  reachable per-field `?` edges.
- The list-iterator `size_hint` impls carry exact-contract asserts
  (before/after one step) — `view.rs` is 100%.

## Doc-test counts (L12)

`reovim-uapi-abi` 47; `reovim-uapi-protocol` 93; macro crates 8
(plus `ignore`-fenced bin-context examples). Every `pub` item
carries an example.

## Falsifiability

A line claimed on this ledger that anyone can show is coverable — by
a constructible input, a fixture exec, or a restructure — falls off
the ledger and must be covered.
