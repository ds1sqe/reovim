# Private environment module

Each native instance owns its values; Kernel has no CWD field or env schema.
CWD is one initial key. Arbitrary keys use the same `resource.bytes` read/write
profile and remain private to the selected client, even with a shared Session.

```sh
cargo build --manifest-path product/Cargo.toml -p reovim-module-env --example env
```

Load the artifact with an empty export mount: absolute library path followed
by a NUL byte. Its declared sibling roots are `env` and `module/env`; shared
aliases are not publication destinations. Grant the module access to the client
root so its handlers can resolve their own exports. See [CLI setup](../../apps/cli/README.md).

| Resource | Method/contract | Encoding/result |
| --- | --- | --- |
| env | read env.names v1 | Empty input; sorted NUL-separated key names |
| module/env/create | write env.create v1 | UTF-8 name, NUL, initial bytes; returns the created key reference |
| env/KEY | read resource.bytes v1 | Empty input; current value bytes |
| env/KEY | write resource.bytes v1 | Complete replacement bytes, including empty |
| module/env/remove | write env.remove v1 | UTF-8 key name; withdraws that key |

Names are nonempty path segments, excluding dot/dot-dot. `create` and `info`
are valid keys because controls live elsewhere. Creating an existing key rejects.
Removal rejects captured/in-flight use; recreation creates a different identity
and old IDs cannot reach the replacement. Names and initialized values publish
together through Interface and the C adapter. No Runtime import is used here.

`cd` and `pwd` are separate loaded modules using `env/CWD`. Empty CWD means the
authorized root. Values remain bytes; malformed CWD can be repaired through an
absolute invocation without first interpreting the old CWD.

`module/env/watch` accepts read `env.observe` v1 with empty input. It returns
one read-only subscription reference and a coherent snapshot: little-endian u64
revision, then sorted key/value pairs to end of data. Each field is a
little-endian u32 byte length followed by its bytes; keys are UTF-8. Snapshots
are bounded to 16 KiB; a larger snapshot rejects before publishing a reader.

The subscription accepts read `env.next` v1 with empty input. It waits for a
change and returns a tag byte followed by the little-endian u64 revision.
Tag 0 invalidates the previous snapshot; tag 1 reports a gap. After a gap,
subsequent reads reject with Stale: create a fresh observation and close the old
reader. Every successful create, replacement (including CWD), or removal
advances the revision once. Rejected mutations emit nothing.

Each reader keeps one delivery position and allows one pending read. There are
at most 32 live readers per instance; a slow reader never delays a mutation.
Use Connection.close on the returned access to fence new reads and settle a
pending read with Closed. Close is idempotent while the identity is retained.
It requires that reader's client owner, independently of source write grants.
Closing one reader leaves other readers and the environment alive. Live readers
keep the native module loaded; unload returns Busy until they are closed and
captured calls/results are released. Lost observe replies use the service's
existing request recovery, preserving the original reader and snapshot.
