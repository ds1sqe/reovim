# 6.4 — ConfigSlice ABI

**Scope.** The exact wire format for the merged + validated config
that the kernel hands to each participant. Lifetime, ownership,
encoding rules, version surface, compound-value strategy.

**Heritage.** the q3-config-system RFC v2 §9 (folded with
amendments); v4 README §12.5; review item "Specify the exact
ConfigSlice ABI".

**Locked rules.** `CFG6` body lives here; CFG1..CFG5 / CFG7..CFG9
referenced.

---

## 1. Goals

- One ABI shape used by every participant kind (kernel, module,
  driver).
- C-callable; no Rust-only types.
- Validated by kernel before construction; participant performs
  typed extraction.
- Kernel-owned, init-call-lifetime; participant copies what it
  retains.
- Independent ABI version (`REOVIM_CONFIG_ABI_VERSION`) from
  participant schema version.

## 2. Top-level slice

```rust
#[repr(C)]
pub struct ConfigSlice {
    pub abi_version:    u32,             // mirrors REOVIM_CONFIG_ABI_VERSION
    pub namespace:      StrSlice,        // participant's namespace string
    pub schema_version: u32,             // mirrors REOVIM_CONFIG_SCHEMA_VERSION
    pub entries:        *const ConfigKvp,
    pub len:            usize,
    pub total_bytes:    usize,           // for CFG6 / §11 cap accounting
    pub reserved:      u32,
}
```

## 3. Key/value pair

```rust
#[repr(C)]
pub struct ConfigKvp {
    pub dotted_key: StrSlice,        // e.g. "ui.replay-max-wait-ms"
    pub kind:       ConfigValueKind,
    pub value:      ConfigValue,
    pub source:     ConfigSource,
    pub flags:      u32,             // §6
}
```

`dotted_key` uses `[a-z0-9-]+` segments separated by `.`. Schema
declares the same shape (1.5 §8.2). Order in `entries` is
**schema-declared field order** — generated accessors index by
position.

## 4. Kinds

```rust
#[repr(u8)]
pub enum ConfigValueKind {
    Unknown        = 0,
    Bool           = 1,
    U32            = 2,
    I32            = 3,
    U64            = 4,
    I64            = 5,
    F64            = 6,
    String         = 7,    // UTF-8 validated by kernel
    Path           = 8,    // OS-encoded bytes; opaque on this side
    Enum           = 9,    // string-form variant; discriminant in `flags` low byte
    CanonicalToml  = 10,   // list/table compound, validated TOML bytes
}
```

## 5. Value

```rust
#[repr(C)]
pub union ConfigValue {
    pub bool_value: u8,             // 0 / 1
    pub u32_value:  u32,
    pub i32_value:  i32,
    pub u64_value:  u64,
    pub i64_value:  i64,
    pub f64_value:  f64,
    pub bytes:      ByteSlice,      // string / path / enum / canonical TOML
}
```

The union discriminator is the **sibling `kind` field** on
`ConfigKvp`. Reading `value` other than per `kind` is undefined.

UTF-8 validation:
- `String`, `Enum`, `CanonicalToml` → kernel validates UTF-8
  before constructing the slice; failure → `Utf8Invalid`.
- `Path` → opaque OS bytes; participant decodes per platform.

## 6. Flags

| Bit | Meaning |
|---|---|
| `0x01` | `secret` (was redacted from any user-facing log; participant sees real value) |
| `0x02` | sourced from project layer (informational; mirrors `source = Project`) |
| `0x04` | sourced from force layer (informational; mirrors `source = Force`) |
| `0x08` | required field (participant's schema declared `required = true`) |
| `0x10` | reserved |
| `0xff_ff_ff_00` | low byte for `Enum` discriminant; rest reserved |

## 7. Source

```rust
#[repr(u8)]
pub enum ConfigSource {
    Default = 0,
    System  = 1,
    User    = 2,
    Project = 3,
    Env     = 4,
    Cli     = 5,
    Force   = 6,
}
```

`source` records which layer (1.5 §3) supplied the field's final
value. Used by introspection (7.4) and by participant diagnostics.

## 8. Compound values

Lists and tables encode as **`CanonicalToml` bytes**. This is
locked for `REOVIM_CONFIG_ABI_VERSION = 1`.

Rationale (50-year lens): a flat, recursion-free ABI has no depth
limits, no nested-pointer ownership rules, and no accessor-codegen
complexity to keep stable forever. The participant already has a
TOML parser online during schema load; the re-parse happens once,
at init, off the hot path. Generated accessors materialise the
parse result at slice ingest and own the materialised copy.

Canonical form: the kernel emits the value re-serialised from its
validated merge result — sorted keys, normalised whitespace,
UTF-8 — so byte equality of two `CanonicalToml` values implies
value equality.

If real-world participants ever find the re-parse cost
prohibitive, a recursive form arrives as
`REOVIM_CONFIG_ABI_VERSION = 2` **alongside** version 1 — never
replacing it (AB15).


## 9. Locked properties

> **CFG6 — `ConfigSlice` is normalised KVP, kernel-owned,
> init-lifetime unless copied.**
>
> - `entries` is in schema-declared order.
> - String/path/enum/canonical-toml bytes are valid for the
>   lifetime of the participant's init call.
> - Strings/enums/canonical-toml are UTF-8-validated by the kernel.
> - Paths are opaque OS bytes.
> - Total-byte cap from 1.5 §11 enforced before slice construction.
> - Kernel owns; participant must not free, must not mutate.
> - `source` byte records origin layer.
> - `flags` bit 0x01 marks `secret` fields (redacted in user-facing
>   output; participant sees true value).
>
> *Class*: ABI / runtime.

## 10. Generated accessors

`declare_config!` (in `uapi/{module,driver}-macros/`) generates a
typed accessor per schema:

```rust
// Generated from config-schema.toml
pub struct VimConfig<'a> {
    slice: &'a ConfigSlice,
}

impl<'a> VimConfig<'a> {
    pub fn leader(&self) -> &'a str {
        // entries[0].kind == String, value.bytes is UTF-8
        unsafe { /* ... */ }
    }
    pub fn replay-max-wait-ms(&self) -> u32 {
        // entries[1].kind == U32, value.u32_value
        unsafe { /* ... */ }
    }
    pub fn watcher_exclude(&self) -> impl Iterator<Item = &'a str> + 'a {
        // entries[2].kind == CanonicalToml, parse list<string>
        // ...
    }
}
```

Generated accessors are not part of the ABI; they are convenience
sugar over the kvp table.

## 11. Lifetime

The `ConfigSlice` and all bytes it references are valid **only**
during the participant's init call. To retain a value:

```rust
// participant init:
fn init(slice: &ConfigSlice) -> Result<Init, ErrorCode> {
    let cfg = VimConfig::new(slice);
    let leader: String = cfg.leader().to_string();   // copy
    Ok(Init { leader, ... })
}
```

After init returns, the kernel may free `entries` and the bytes.
A participant that reads stale slice memory reads UB.

## 12. Reload (out of v4 target)

If a future spec gains lifecycle="reloadable" honour, a new HostApi
function will deliver a fresh `ConfigSlice` to the participant.
The slice from the prior delivery becomes invalid at that call.
Participants must defensively-copy in either model.

## Open items

1. ~~Compound encoding strategy~~ — resolved (§8):
   `CanonicalToml` locked for config-ABI version 1.
2. Whether `Path` should be UTF-8 on platforms where it is (Linux:
   not always UTF-8; macOS: usually NFD UTF-8; Windows: UTF-16 →
   transcoded). Default: opaque bytes; participant decodes.
3. Whether `flags` bit 0x10 (reserved) should be assigned now (e.g.
   `ephemeral` for fields that don't persist). Default: leave reserved.
4. Whether the kernel guarantees stable pointer for the slice
   across the init call. Default: yes (slice is on a kernel-side
   arena freed only after init returns).

## Conformance

| Aspect | Fixture |
|---|---|
| Layout | Per-target-triple golden offset/size of `ConfigSlice`, `ConfigKvp`, `ConfigValueKind`, `ConfigSource`, `ConfigValue` union. |
| Order | Slice constructed from a 5-field schema; fixture verifies entries match schema declaration order. |
| Source tagging | Force-set fixture: a field set by `--force-set` has `source = Force` and `flags & 0x04`. |
| Secret redaction | Field with `secret = true` shows real value in slice; `dump --effective` redacts. |
| UTF-8 | Schema with intentionally-invalid byte payload at user layer → `Utf8Invalid`. |
| Cap | Schema produces total > §11 cap → `ConfigSliceTooLarge`. |
| Compound | `list<string>` field decoded by generated accessor matches the project file's source list. |
