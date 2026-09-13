# Minimal server

Build from the repository root:

```sh
cargo build --manifest-path product/Cargo.toml \
  -p reovim-server -p reovim-cli --features std
product/target/debug/reovim-server 127.0.0.1:7878
```

Open the [Resource shell](../cli/README.md) in another terminal:

```sh
product/target/debug/reovim-cli 127.0.0.1:7878
```

Follow the shell's explicit loading example to install the separate `read`
and `write` command artifacts. Command implementations are not linked into
this server; an independently built Resource module can add another command
without a CLI or Kernel rebuild.

Without configuration, the service starts with private client roots and no
concrete modules or Session. On Linux, process arguments select native artifacts
by their passive contract declarations:

```sh
reovim-server 127.0.0.1:7878 \
  --module counter /opt/reovim/modules/counter.so counter \
  --start counter 1 \
  --client-module environment /opt/reovim/modules/env.so '' \
  --grant '' all --client-start env 1
```

`--module LABEL LIBRARY MOUNT` declares a server-owned catalog entry.
`--client-module` declares a fresh instance template for every retained client.
`--start CONTRACT SCHEMA` and `--client-start` select exact required identities.
Dependencies come from the artifact and use the existing catalog graph; missing,
incompatible, ambiguous and cyclic selections fail startup. Discovery retains
the exact executable, reads metadata and does not call the module constructor.
Dynamic linker constructors and the ABI entry point still execute native code.

`--grant PATH read|write|all` applies to the preceding declaration. Paths refer
to existing Resources in that context; an empty path grants its root subtree.
No grants are implicit. Dependency selection does not grant data access.
Shared aliases need their own named grants, such as `--grant modules read`;
granting the private root alone does not grant the alias targets.
Server and client dependency catalogs have separate selection scopes.
Server publications appear under the client's `modules` alias; private client
publications and `catalog/LABEL` appear in its own root. Ordinary activation of
an unstarted catalog remains available. Each context admits at most 64 configured
declarations and 64 required roots, subject to Runtime capacity.
The service reserves limits of 1024 Resources and 256 caller/import scopes;
other Runtime limits retain their defaults. Sixteen client contexts with private
env templates are exercised together; richer configurations can exhaust a
different bound first and return pressure.

For a shared Session, select a native Model and pass
`--session MODEL_PATH INPUT_CONTRACT SCHEMA HEX_DATA`. The server contains no
heading schema or constructor choice. `--export-root DIRECTORY` separately
grants filesystem output; see [dump](../../modules/dump/README.md).

`--buffer MODEL_PATH INPUT_CONTRACT SCHEMA HEX_DATA` creates one server-owned
Buffer through the selected Model's public creation contract. It is mounted as
`buffer` in each independent Client scope. Clients receive read/write access
and share its History, but the server remains the close owner. Each Client
attaches/detaches its own Text interpretation; shared content implies no focus
or view sharing. Buffer and Session configuration remain separate choices.

`--snapshot MOUNT SOURCE SCRATCH_DIRECTORY` grants an immutable file snapshot
at `modules/MOUNT`. The Linux Host atomically clones the source into an
anonymous file on the scratch filesystem. Unsupported or cross-filesystem
cloning fails boot explicitly. Source and scratch are explicit process grants;
this does not expose arbitrary filesystem paths to modules. See the
[source model](../../modules/source/README.md) for lazy working-state opening.

`--files MOUNT DIRECTORY SCRATCH_DIRECTORY` grants fresh capture after boot.
Write a UTF-8 relative filename to `modules/MOUNT` with `host.files.capture` v1;
the result owns one READ snapshot Resource. Every path component stays beneath
the pinned directory grant; absolute paths, `..`, symlinks and non-regular files
reject. Capture runs on a bounded Host worker and publishes only on success.
Pass the snapshot to `source.open`, then close it after its working states and
retained reads no longer need the backing. Explicit source-access close fences
future reads; it is not the same operation as shared Buffer close. Replaying
one retained request returns its original snapshot; a new request captures a
new cut. Unsupported atomic cloning returns Unsupported without a snapshot.

`host.files.select` v1 on the same directory selects a shared file source and
prepares an immutable data/base view. Pass its READ reference to the selected
Model with `kernel.buffer.open` v1 to install a backed Buffer. Repeated selection
of the same pinned file, including hard links, reuses its subject within that
directory provider. Replacing a pathname selects a different file. Each Buffer
retains its own base and working state; a later selection cannot change them.

Selection response data names the Resource child. `host.files.release` v1 takes
that UTF-8 name with no references and withdraws the idle selection. It rejects
captured or in-flight use; closing one Buffer does not close the shared source.
The directory owns selections until explicit release or module unload, within
Runtime resource limits. This read-only profile supplies exact original content
as base evidence, not conditional save support. The loaded `command.open`
composes selection and backed opening, releasing its capture without closing
the shared source. Its Client journals the descriptor and retained recovery work.

An installed user supplies only a compatible module package and configuration;
adding it requires neither a Reovim checkout nor a server/CLI rebuild. The ABI is
SemVer 0.1.0 with reserved u32 zero until official release; operation and module
contract schemas are separate u32 identities. Installation performs no network
fetch. The native and output adapters currently support Linux.

`kernel/modules/load` also loads standalone native Resource or State Model
providers after startup; see [native-counter](../../modules/native-counter/README.md).
Artifacts declaring dependencies must use the catalog path.
`host.module.load-with` accepts one Resource reference as the module's granted
import subtree; [native-forward](../../modules/native-forward/README.md) calls
another module through that connection.

The [command transport](../../host/service/README.md) retains separate client
roots selected by connection tokens. Each has its own diagnostic access and
native loader; configured server services and an optional selected Session
are explicitly shared. Each client keeps its own env instance and namespace;
Session access does not share CWD.
A module loaded into a client's root cannot publish through shared aliases.
Selecting that full root as its imports includes the client's authorized shared
aliases; selecting a narrower Resource grants only that subject's subtree.
State lasts until explicit client teardown or process exit. Teardown refuses to
discard live Buffer/Session state or unadopted receipts. Delivery receipts retain
client startup as well as ordinary requests. `--connect` returns only after its
required contracts are ready and acknowledges that startup receipt; a failed
startup reports its retained client token and receipt for recovery/cleanup.

Socket workers use bounded channels and retain no Kernel execution. Waiting
responses and incomplete frames do not block other clients. A disconnected
observer leaves its request and delivery receipt alive until explicit adoption.
The Runtime owner sleeps until work wakes it; lifecycle cleanup keeps bounded
turns with a 1 ms park when it still needs advancement.

`host.module.load` and `host.module.load-with` return the installed instance
reference once installation has happened. Result data is `ready`,
`readiness-error<TAB>CODE`, or
`readiness-unknown<TAB>CODE<TAB>RECOVERY`. A readiness observation failure does
not discard the known instance or authorize another load. Its ordinary
`kernel.module.ready`/status operations remain available. Rejection before
installation remains an error.

Run the process scenarios after building:

```sh
python3 scripts/e2e.py
```

The [scenario guide](../../scripts/e2e_cases/README.md) covers selection,
retained logs and adding independent journeys. Each test owns a fresh server.
