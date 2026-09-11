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

Assembly supplies [counter](../../modules/counter/README.md) and
[editing](../../modules/editing/README.md) modules.
Activation constructs each instance at runtime; unload removes its service. The module
imports Kernel Interface only. These initial factory declarations are compiled into the server. On Linux, `kernel/modules/load` loads native Resource or State Model
providers after startup; see
[native-counter](../../modules/native-counter/README.md).
`host.module.load-with` accepts one Resource reference as the module's granted
import subtree; [native-forward](../../modules/native-forward/README.md) calls
another module through that connection.

The [command transport](../../host/service/README.md) retains separate client
roots selected by connection tokens. Each has its own diagnostic access and
native loader; the initial editing catalog/services and one selected Session
are explicitly shared. Each client keeps its own env instance and namespace;
Session access does not share CWD.
A module loaded into a client's root cannot publish through shared aliases.
Selecting that full root as its imports includes the client's authorized shared
aliases; selecting a narrower Resource grants only that subject's subtree.
State lasts until process exit. Client-scoped env, explicit client teardown
remain unfinished. Delivery receipts preserve original outcomes across sockets.

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
