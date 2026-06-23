# 5.2 — Raw Input

**Scope.** The kernel's hot-path input surface: `RawInput { kind,
payload }`. What kinds exist, what their payloads look like,
keyboard/mouse/text/paste/web typed records, why no kernel
keyparsing, and the JSON-Schema boundary for CLI/debug.

**Locked rules.** Carried key-free invariant; concrete payload
records resolved and locked in the type catalog (6.3 §7). For how
the client side normalizes platform sources into this vocabulary see
8.2 §6.2.

---

## 1. The key-free invariant

The kernel does not parse keymaps, modes, motions, registers, or
leader sequences. Input crosses into the kernel as opaque framed
bytes; semantic interpretation is module/Domain policy.

## 2. Surface

```rust
#[repr(C)]
pub struct RawInput {
    pub kind:    RawInputKind,
    pub payload: ByteSlice,            // payload bytes; kind decides shape
}

#[repr(u8)]
pub enum RawInputKind {
    Key     = 1,
    Mouse   = 2,
    Text    = 3,    // bracketed paste / typed text run
    Paste   = 4,    // explicit clipboard paste
    Web     = 5,    // reserved; locked with the WASM-peer decision
    Trigger = 6,    // raw-input trigger ID (PM5 lazy-load triggers)
    Ime     = 7,    // composition events (preedit / commit)
    // 128..=255 reserved for module-defined custom kinds
}
```

(catalog: 6.3 §7 — the catalog is the authoritative layout
source; this chapter explains semantics.)

## 3. Hot-path payload format

Two boundaries, two formats, one source of truth:

- The **hot path** (handler dispatch) carries typed `#[repr(C)]`
  records — zero-copy, no JSON, no string parsing. The concrete
  per-kind layouts are locked in `06-ABI/03-Type-Catalog.md` §7:
  `KeyEvent`, `MouseEvent`, `TriggerEvent`, `ImeEvent`, and raw
  UTF-8 payloads for `Text` / `Paste`.
- The **CLI / debug translation layer** encodes/decodes the typed
  records to/from JSON Schema at its own boundary for replay
  tooling and introspection; the kernel never sees JSON on the
  hot path. JSON schemas are golden-tested (CF4).

Semantics worth restating here:

- **Keycodes** are USB HID page-0x07 usage IDs with a Reovim
  extension range for software-only keys (6.3 §7.1.1). Input
  drivers own platform translation.
- **Mouse coordinates** are window-local grid cells primary, with
  optional pixel refinement (6.3 §7.2) — every platform can
  produce cells; pixel-capable platforms add precision.
- **IME composition is first-class** (`Ime` kind, 6.3 §7.5):
  begin / update / commit / cancel phases with preedit text and
  caret. Keymap modules pass `Ime` events to the focused Domain
  untouched.
- **`Trigger`** crosses as an opaque ID: the kernel does not know
  whether `trigger_id = 7` is "the user pressed `<leader>q`" or
  "the file watcher fired"; it routes to the registering cdylib.
- **`Custom(128..=255)`** kinds are per-cdylib (6.3 §7.6): the
  emitter is the only consumer; manifests declare the payload
  schema for debug decoding.


## 4. Dispatch

`RawInput` is delivered through `OnRawInput` handler dispatch, walking
the focus chain leaf-to-root (4.2 §7).

## 5. Bounded resources

| Cap | Field |
|---|---|
| Max input rate per client | `max-input-per-sec-per-client` |
| Max payload bytes | `max-raw-input-bytes` |

Excess returns `ErrorCode::ResourceExhausted` and DS12 emits.

## 6. Forbidden

- The kernel does not parse keymaps, modes, leader sequences, or
  motions.
- The kernel does not maintain a "pending key sequence" buffer.
  Sequences are module state.
- The kernel does not interpret `RawInputKind::Trigger` payload
  beyond routing.

## Open items

1. ~~Concrete payload layouts~~ — resolved (6.3 §7).
2. ~~Mouse coordinate system~~ — resolved: window-local
   grid cells primary, optional pixel refinement.
3. ~~IME / composition~~ — resolved: own `Ime` kind, not
   `KeyEvent` state.
4. Web adapter contract (out of target; kind value 5 reserved).
5. ~~Custom-kind allocation~~ — resolved: per-cdylib,
   emitter-is-consumer, manifest-declared schema.

## Conformance

| Behaviour | Fixture |
|---|---|
| Payload layouts | Golden offset/size tests per kind per target triple (6.3 §7; CF4). |
| Key-free invariant | Editor-core source contains no keymap/mode/motion parsing; CI grep + review gate. |
| IME pass-through | `Ime` events reach the focused Domain handler unmodified through a keymap module. |
| JSON boundary | CLI replay round-trips each kind through its JSON schema with byte-identical `#[repr(C)]` records. |
