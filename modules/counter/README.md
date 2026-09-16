# Counter module

A non-byte Resource using ordinary module activation. The implementation depends
only on Interface. Its inert constructor retains the scoped Registrar; Runtime
advance stages `value` under the mount selected by composition.

| Declared operation | Input | Result |
| --- | --- | --- |
| exec `add` | u64 | New counter value |
| ioctl 1 `read` | Unit | Current u64 |

Values use the canonical little-endian encoding. Invalid input and overflow
preserve the counter. Each activation starts at zero. The Resource offers no
byte I/O; callers discover the declaration, bind it and select opened access.

The `counter` cdylib example uses SDK `export_module!` with the same Rust
implementation. The macro emits `rv_module_entry`; SDK owns the native driver
callbacks and imported Caller/Registrar lifetime cleanup.

```sh
cargo build --manifest-path product/Cargo.toml \
  -p reovim-module-counter --example counter
cargo test --manifest-path product/Cargo.toml \
  -p reovim-server --test native_module
```

The server integration test builds the artifact after Runtime starts, loads it
through the production Host loader and removes the file before invocation.
It discovers/binds the API, adds a value, reads it back and closes Runtime.
This establishes native composition; the server/base Client process journey
and individual module unload remain execution-plan work.
