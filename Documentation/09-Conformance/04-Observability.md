# 9.4 — Observability

**Scope.** DS12 event taxonomy, required event families, correlation
IDs, schema rules.

**Heritage.** v4 README §15.2.

**Locked rules.** `OBS1..OBS4`.

---

## 1. DS12 shape

```rust
pub struct DS12Event {
    pub ts:     SystemTime,
    pub level:  LogLevel,                 // 6.3 §2.4: Trace / Debug / Info / Warn / Error
    pub event:  &'static str,             // dotted family.subject (e.g. "cdylib.load.ok")
    pub fields: HashMap<&'static str, FieldValue>,
}
```

`event` is a stable string from a fixed family/subject vocabulary.
`fields` carries event-specific structured data.

## 2. Required event families

> **OBS1 — DS12 event families.** Every implementation MUST emit
> events from these families when the corresponding operation
> occurs:

| Family.subject | When |
|---|---|
| `boot.stage.start` | each L21 stage starts |
| `boot.stage.ok` | stage success |
| `boot.stage.fail` | stage failure |
| `config.layer.loaded` | per layer per participant |
| `config.layer.rejected` | layer rejected (CFG validation fail) |
| `config.force_override` | per overridden field at boot (CFG5) |
| `pkg.sync.start` | `pkg sync` started |
| `pkg.sync.ok` | `pkg sync` success |
| `pkg.sync.fail` | `pkg sync` failure |
| `pkg.verify.fail` | verification mismatch |
| `cdylib.load.start` | `dlopen` begin |
| `cdylib.load.ok` | reached `Active` |
| `cdylib.load.fail` | rejected at any pre-Active step |
| `cdylib.init.start` | init body invoked |
| `cdylib.init.ok` | init returned success |
| `cdylib.init.fail` | init returned error |
| `cdylib.init.panic` | init panicked (AB12) |
| `cdylib.unload.start` | unload begun |
| `cdylib.unload.draining` | drain phase |
| `cdylib.unload.timeout` | drain timeout (FAIL3) |
| `cdylib.unload.ok` | reached `Unloaded` |
| `cdylib.unload.fail` | tombstoned-failed-unload |
| `auth.reject` | transport auth rejected |
| `debug.drive.start` | drive op begun (DS13) |
| `debug.drive.ok` | drive op success |
| `debug.drive.fail` | drive op failure |
| `dispatch.handler.panic` | handler panicked (AB12) |
| `lazy.replay.queued` | input queued on Pending leaf |
| `lazy.replay.timeout` | replay queue timed out |
| `lazy.replay.cancelled` | replay queue cancelled |
| `lazy.replay.replayed` | replay applied to live attachment |
| `persistence.restore.skip` | deferred-restore entry expired |
| `persistence.restore.drop` | restore-time validation failed |
| `persistence.restore.ok` | session restored |
| `persistence.save.ok` | persist handler completed (Debug level) |
| `persistence.save.fail` | persist handler failed or timed out during detach (LF12) |
| `detach.handler.fail` | `OnDetach` handler failed or timed out (LF12) |
| `stream.backpressure` | scheme entered Blocked |
| `stream.stale` | underlying source ended |
| `stream.draining` | scheme drain begin |
| `shutdown.start` | kernel shutdown begin |
| `shutdown.drain` | drain phase |
| `shutdown.ok` | clean shutdown |
| `shutdown.fail` | shutdown error |
| `shutdown.forced` | immediate shutdown engaged (LF16) |
| `focus.transition` | focus chain mutation committed (DT14, Q-4) |

> *Class*: runtime.

Two additional families are owned by 9.5: `log.message` (free-form
diagnostics, LOG4 — voluntary, not operation-bound) and
`log.sink.fail` (LOG7).

### 2.1 `focus.transition` event (OBS4)

> **OBS4 — `focus.transition` schema.** Required fields:
> - `session_id` (string),
> - `client_id` (u64; matches `ClientId(usize)` in the catalog),
> - `buffer_id` (u64),
> - `window_id` (u64),
> - `seq` (u64; monotonic per session),
> - `before` (array of focus-entry snapshots: `{kind, id, domain}`),
> - `after` (array of focus-entry snapshots).
>
> Cardinality: bounded by input rate; rate-limited per
> `(session_id, client_id, window_id)`.
> Compatibility: additive on minor.
>
> *Class*: runtime + CI golden tests.

## 3. Correlation IDs

> **OBS2 — External operations carry correlation IDs.** Every
> operation initiated externally (CLI, framed-protocol request, package sync,
> debug-drive, session attach) carries a correlation ID propagated
> through every DS12 event in that operation.
>
> Exceptions: explicitly listed early-bootstrap events that fire
> before the correlation allocator exists (`boot.stage.*` for
> stages 0..1).
>
> *Class*: runtime.

## 4. Schema rules

> **OBS3 — DS12 schemas locked.** Each event family/subject has a
> normative JSON schema declaring:
> - required fields,
> - redaction class per field (CFG7),
> - cardinality bounds (rate-limit per source per minute),
> - compatibility rules (additive on minor; new field is optional;
>   removed field requires major).
>
> *Class*: runtime + CI golden tests (CF4).

## 5. Required common fields

Where applicable, events SHOULD carry these common fields:

| Field | Type | Notes |
|---|---|---|
| `correlation_id` | uuid-like string | per OBS2 |
| `cdylib_id` | u32 | per-cdylib events |
| `package_name` | string | pkg.* events |
| `package_version` | string | pkg.* events |
| `checksum` | hex string | pkg.verify.* events |
| `session_id` | string | session-scoped events |
| `client_id` | u64 | client-scoped events |
| `request_id` | string | RPC events |
| `error_code` | i32 | failure events |
| `source` | string | layer/source label (e.g. "user", "force") |
| `redaction_class` | string | "public" / "secret" / "internal" |
| `rollback` | "unnecessary" / "ok" / "partial" / "failed" | per FAIL2 |

## 6. Rate limiting

CR19-style rate limits cap event volume per source per minute.
Excess events are coalesced into a single
`<family>.<subject>.coalesced` event with a count.

Default: 1000 events/min per source per family.

## 7. Subscriber model

DS12 events fan out to subscribers (DS4 subscriber capacity).
Subscribers register a filter (family glob + level threshold) and
receive a stream.

Subscriber capacity per kernel instance: `kernel.host.[limits].observe-subscriber-capacity`
(default 64).

## Open items

1. JSON Schema source-of-truth — separate file per family vs one
   schema document.
2. ~~Whether DS12 streams are durable (file-backed) or volatile~~ —
   resolved: volatile; durability is the 9.5 file sink (LOG7), a
   plain subscriber. The log ring (LOG6) bounds in-memory retention.
3. Default rate limits per family — current defaults are guesses.

## Conformance

| Rule | Fixture |
|---|---|
| OBS1 | Each operation in §2 fires its named event when invoked. |
| OBS2 | Framed-protocol request trace: every event in the trace carries the same correlation ID. |
| OBS3 | JSON schema fixture: emit + parse round-trip per family. Field-add → minor; field-remove → major. |
