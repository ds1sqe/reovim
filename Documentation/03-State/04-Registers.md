# 3.4 — Registers

**Scope.** Register storage shape, scope rules, and access pattern.

**Heritage.** Flagged as "not yet chapter-consolidated" — this
chapter is a minimum-viable shape with v0.16 defaults resolved in
§Open items.

**Locked rules.** None at this revision; reshape candidates below.

---

## 1. Register identity

A register is identified by `(scope, name)`:

- `scope`: `client` | `session` | `system`.
- `name`: short string interned to `RegisterId(u32)`.

Register storage is a `CursorCarrier` payload — registers are not
text-only. The carrier's domain dictates content.

The kernel does not name vim registers. Names such as `"a`, `"+`,
`"_`, `"*`, numbered history, and yank rings are module policy. The
kernel interns opaque register names into `RegisterId(u32)` and stores
carrier bytes under that ID.

## 2. Storage layout

```rust
pub struct RegisterStore {
    // per-client (lives in ClientView)
    client_regs:   HashMap<RegisterId, CursorCarrier>,

    // per-session (lives in Session)
    session_regs:  HashMap<RegisterId, CursorCarrier>,

    // global (lives in Kernel)
    system_regs:   HashMap<RegisterId, CursorCarrier>,
}
```

Read order on lookup: `client → session → system` (innermost wins).

## 3. Access

HostApi:

```c
ErrorCode hostapi_register_get(RegisterScope scope, RegisterId id,
                         CursorCarrier* out);
ErrorCode hostapi_register_set(RegisterScope scope, RegisterId id,
                         const CursorCarrier* value);
ErrorCode hostapi_register_clear(RegisterScope scope, RegisterId id);
```

Cross-Domain paste is a focused-Domain operation. `hostapi_register_get`
returns the carrier bytes and status; the focused Domain's
`OnRawInput`/paste handler decides whether it can consume the carrier,
request a codec/service translation, or reject with a diagnostic. The
kernel never translates register content between Domains.

## 4. Bounded resources

| Cap | Field |
|---|---|
| Per-client register count | `kernel.host.[limits].max-client-registers` |
| Per-session register count | `max-session-registers` |
| Per-system register count | `max-system-registers` |
| Per-register carrier bytes | reuses `max-cursor-carrier-bytes` (4.5 §6) |

## 5. Persistence

System and session registers persist (per 2.4 §1). Client registers
are session-volatile.

## Open items (resolved for v0.16)

1. ~~Vim register vocabulary~~ — resolved (§1): module policy;
   kernel sees opaque `RegisterId`.
2. ~~Yank-ring / numbered-register history~~ — resolved (§1): module
   state, not kernel register mechanism.
3. ~~Cross-Domain register paste~~ — resolved (§3): focused Domain
   decides how or whether to consume the carrier.
4. ~~`RegisterScope::Buffer`~~ — omitted from v0.16; no current use
   case. Adding it requires a future spec change.

## Conformance

| Behaviour | Fixture |
|---|---|
| Lookup order | Same `RegisterId` set at client/session/system scopes → get returns client value; clearing client falls back to session. |
| Carrier storage | Store a hex carrier and retrieve byte-identical header/content; invalid carrier set is rejected by CR6. |
| Module policy | Vim module maps `"a` to an opaque `RegisterId`; kernel never special-cases the name. |
| Cross-Domain paste | Text Domain receives a hex carrier from a register and rejects or translates through its own handler; kernel performs no conversion. |
| Scope vocabulary | `client`, `session`, `system` accepted; `buffer` rejected in v0.16. |
