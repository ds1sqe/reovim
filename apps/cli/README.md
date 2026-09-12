# Resource shell

The CLI resolves a Resource and submits one complete write Message. Commands
are runtime-loaded Kernel modules. Each built-in command has its own crate;
an extension can expose its own command alongside its other Resource exports.

Bare command names search `env/PATH`, independently of `env/CWD`. PATH is an
ordinary module-owned value containing colon-separated absolute Resource
directories; when absent it defaults to `/bin`. An empty value disables bare
command lookup. Empty components are ignored; relative components reject.
At most 32 directories and 4096 PATH bytes are accepted.

Place commands under `/bin` in composition. `ls` lists CWD, `ls /` lists the
root, and `tree` displays the selected hierarchy with its root label. After
`cd /env`, `ls` still selects `/bin/ls` and lists `/env`. Explicit `/bin/ls`,
`./name`, relative paths containing `/`, and caller-local `@ID` remain direct
Resource calls. Arguments and borrowed-reference paths use CWD.

Build the [server](../server/README.md) and these two command artifacts:

```sh
cargo build --manifest-path product/Cargo.toml \
  -p reovim-module-read -p reovim-module-write --examples
```

On Linux, load them into the running server using explicit encoded requests:

```sh
cli=product/target/debug/reovim-cli
export REOVIM_CLIENT=$($cli 127.0.0.1:7878 --connect)
for name in read write; do
  module_payload=$(printf '%s\0%s' \
    "$PWD/product/target/debug/examples/lib$name.so" "bin/$name" | \
    od -An -v -tx1 | tr -d ' \n')
  $cli 127.0.0.1:7878 --refs / --contract host.module.load-with 1 \
    --hex "$module_payload" /kernel/modules/load
done
```

This local example grants each module the root subtree. Assembly can supply a
smaller import subtree instead. Loading returns the module-instance reference.

```sh
$cli 127.0.0.1:7878 write /modules/catalog/counter \
  --contract kernel.module.activate 1
$cli 127.0.0.1:7878 write /modules/counter/value \
  --contract external.counter 1 --hex 0700000000000000
$cli 127.0.0.1:7878 read /modules/counter/value \
  --contract external.counter 1
```

Without an explicit invocation contract, the CLI reads the Resource's ordinary
`info` child for its default write contract, argument encoding and output mode.
Options before the Resource path belong to the CLI; arguments after it belong
to the module. Omitted arguments supply empty data. The `read` and `write`
modules accept `PATH --contract NAME VERSION [--hex DATA]` as their arguments.

Use CLI `--refs PATH,@ID` to supply checked Resource reference slots. A loaded
read/write module selects a passed subject with `@0`, `@1`, and so on; these
are slot indices, not raw IDs in the module's private namespace. Reply Messages
preserve contract, encoded data and returned caller-local references.

Run the CLI with only an address for an interactive session. Input splits on
whitespace. Tab completes command names from PATH or Resource paths from CWD;
completion reads namespace metadata and never invokes a command. Ambiguous
completion follows the terminal editor's list/common-prefix behavior. Paths
containing whitespace/control characters remain available through explicit
encoded calls but are not offered as completion candidates.
Up/Down navigate input history; Left/Right, Home/End, Backspace and Delete edit
the current line. Ctrl-C clears pending input; Ctrl-D exits an empty prompt.
History stays in memory for this CLI process, with up to 100 entries of at most
16 KiB each. Piped input retains the existing plain line loop. See the
[terminal dependency review](DEPENDENCIES.md).

Relative Resource paths use the conventional `env/CWD` value when available.
Each new client has its own namespace and loader. `--connect` prints a token
for a retained client; reuse it with `--client TOKEN` before the Resource path
or `REOVIM_CLIENT` for several CLI processes. Without a selected token, an
interactive session or one-shot invocation opens a new client. Tokens remain
valid only in this live server. The native env module owns per-client values,
CWD and independent observation. Failed one-shot requests exit nonzero;
disconnected effects are never replayed.

Each exchange reserves a delivery identity before sending its operation. A
known response is adopted and its receipt released. An interrupted or unknown
response reports its client and request identity; it is never replayed under a
new identity. On that same client, use `--requests`, `--request-status ID`,
`--request-info ID` and `--recover ID` to inspect retained delivery evidence.
After adoption, `--forget ID` releases its receipt, not returned Resource access.
These connection/delivery options precede ordinary Resource invocation.

The [env module](../../modules/env/README.md) declares sibling `env` and
`module/env` paths and loads with an empty export mount. Absolute Resource paths
and caller-local IDs bypass CWD; relative reference arguments still use it.
An ID can also name a child, such as `@ID/info`, through ordinary scoped lookup.
The separately loaded `cd` and `pwd` modules use the env instance's CWD value.

Use `--client TOKEN --disconnect` for explicit retained-client cleanup. It
closes that client's watches first, waking pending reads with Closed. Outstanding
receipts keep teardown Busy: recover/adopt them and use `--forget ID`, then
retry. Live private Buffers and Sessions must be closed through their declared
policies first; disconnect never silently discards edited state. Busy keeps the
connection usable for recovery and those cleanup calls.

Once work is adopted, disconnect releases the client's snapshots, unloads its
private modules, removes its private namespace and revokes its Connections.
Shared Session/alias targets and other clients remain alive. Module cleanup may
need another retry while its driver finishes. The service remembers the last
16 completed disconnect tokens for idempotent retry; ordinary calls with a
closed token reject. Socket loss and exiting a CLI alone are not disconnect.
