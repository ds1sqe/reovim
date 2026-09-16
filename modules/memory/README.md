# Memory module

An ordinary module publishes a `create` factory under its configured mount.
Kernel and server depend on neither its API nor its byte storage implementation.
The module owns its immutable byte storage and depends on Interface. Its cdylib
example uses SDK `export_module!` for the common native entry and lifecycle.

The discovered factory declares exec with a Bytes input bounded to 4096 bytes.
It stages a private copy and returns one primary Resource plus a Unit result.
Creation requires EXEC and a granted READ creation ceiling. The resulting
Resource supports positional reads, with EOF at the end and no write interface.
Contents are unrestricted bytes. New factory calls own independent storage.

```sh
cargo build --manifest-path product/Cargo.toml \
  -p reovim-module-memory --example memory
cargo test --manifest-path product/Cargo.toml \
  -p reovim-server --test native_module
```

The server integration test builds the module after Runtime starts and removes
the artifact file before factory invocation. It exercises discovery, binding,
creation, exact opaque data, positional EOF, the declared size bound, denied
creation without grants, denied WRITE access and final Resource/Runtime close.
Hosted process control, module registry/recovery and individual unload remain
separate execution-plan requirements.
