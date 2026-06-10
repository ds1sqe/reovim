# 3.4 — Registers

**Scope.** Register storage shape, scope rules, and access pattern.

**Heritage.** v3 `03-State/04-Registers.md`. v4 README §0.4 flagged
this as "not yet chapter-consolidated" — this chapter is a
minimum-viable shape; details TBD before lock.

**Locked rules.** None at this revision; reshape candidates below.

---

## 1. Register identity

A register is identified by `(scope, name)`:

- `scope`: `client` | `session` | `system`.
- `name`: short string interned to `RegisterId(u32)`.

Register storage is a `CursorCarrier` payload — registers are not
text-only. The carrier's domain dictates content.

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

## Open items (must resolve before lock)

1. Vim register vocabulary (e.g. `"a..z`, `"+`, `"_`, `"*`) — kept
   as policy in the vim module, or kernel-named?
   **Default**: vim module owns names; kernel knows only
   `RegisterId(u32)` opaque keys.
2. Yank-ring / numbered-register history — kept in module state
   (3.2) or as a kernel-side ring with bounded depth?
3. Cross-Domain register paste — when `register["x"]` holds a hex
   carrier and the active Domain is text, what happens? Likely
   Domain-specific handler decides; spec must document.
4. Whether `RegisterScope::Buffer` is needed (no current use case;
   omitted for now).

## Conformance

This chapter is a sketch. Conformance fixtures land when §Open #1
and #2 are resolved.
