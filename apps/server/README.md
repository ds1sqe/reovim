# Server composition

`assemble(epoch, limits)` starts an empty Kernel Runtime. Composition owns this
execution context, admits module implementations and supplies granted Callers
to consumers. The server links no memory, text, heading or keyed provider and
exposes no domain-specific demo Resource fields.

The `native_module` integration test uses this assembly and the production Host
loader. After startup it compiles a C module, loads it, removes the artifact,
then discovers and binds both exported APIs. Calls return their distinct values;
shutdown verifies that module data outlives embedded providers and late cleanup.
A second journey builds the Rust counter artifact after Runtime starts, loads
it through the same entry, discovers its declaration, adds 37 and reads 37.
A memory factory journey creates separate Resources, preserves unrestricted
bytes and positional EOF behavior, and checks creation and read-only grants.
Neither implementation is linked into the server test binary. These Rust
artifacts enter the Kernel registry as Interface definitions; exact revision
selection supplies the implementation and admitted imports for activation.

`loading::Loader` offers a declared admission Resource. Its callback only queues;
composition drives artifact policy and registration, then grants the package to
an existing recipient Caller. Native path bytes remain unrestricted. Repeated
paths reuse the captured package even after artifact deletion; activation is a
separate ordinary package call. Cancellation, bounded queues and loader shutdown
retain original receipt outcomes. The hosted service drives it beside Runtime;
boot now captures Definitions through the same Host policy.

`clients::Clients` retains Host-issued credentials and exact Caller bindings.
It dispatches canonical service frames and preserves recovery during retirement;
retired credentials never retarget a replacement. Credential retention is bounded
separately from Runtime's active Caller slots. It selects no providers and stores
no duplicate receipts. Process composition issues one random credential.

With `std`, `service::serve` accepts an already bound listener, Runtime, Clients,
loaders and a stop flag. It dispatches canonical frames, drives the supplied
owners and joins listener/workers when stopped. Runtime custody remains with
composition after transport stops. `serve_closing` keeps recovery service alive
after termination until Runtime cleanup finishes; Busy/Incomplete do not exit.

```sh
cargo test --manifest-path product/Cargo.toml \
  -p reovim-server --features std --lib --test clients --test native_module \
  --test process
```

`boot::Modules` captures configured artifacts through a supplied Host admission
policy. Labels select those exact Definitions; `register` receives the actual
registry, mount, immutable imports, approved dependency names and recipient.
A failed recipient grant preserves the admitted package identity. Discovery and
registration remain passive; startup invokes ordinary package activation.
`boot::Startup` drives required package activation through ordinary Caller
receipts. It retains the original request across submission/observation failure,
returns Ready identity after closing temporary accesses, and leaves UNKNOWN
custody available for reconciliation. Composition must drive it beside Runtime.
Configuration accepts `--module LABEL LIBRARY` and `--start LABEL`. Client/state
constructor flags are retired; their owning module/Client migrations remain open.

The Linux executable consumes required startup labels before announcing
readiness. It issues an OS-random epoch and one credential, grants the initial
exports, and supplies `/load` for later admission through a trusted directory:

```sh
cargo run --manifest-path product/Cargo.toml -p reovim-server --features std -- \
  127.0.0.1:7878 --trusted-module-root /srv/reovim/modules \
  --module device /srv/reovim/modules/device.so --start device
```

The readiness line is `ready ADDRESS CREDENTIAL`; its credential authorizes the
launcher's Client. Native code under the selected root must obey the ABI; root
selection is not a native-code sandbox. SIGINT/SIGTERM fences new work while
original receipt recovery remains available. Release held receipts to complete
shutdown. A process test reads a configured native export and verifies this
recovery after SIGTERM. Another builds an artifact after process readiness,
admits it through `/load`, deletes it and activates the captured package, then
lists `/loaded`, acquires the returned identities, discovers their declarations
and reads both new backend APIs through the same process.

Current process policy grants all operation flags on initial exports, zero
factory creation flags and no dependency import approvals. Late admission grants
package access. An explicit Global publication grant at the loader mount admits
its future exports; unrelated subjects with similar paths gain no authority.
The actual base Client process and native namespace materialization remain open.
The old service-source inclusion test is removed: hosted unit and actual process
tests now compile the real library and executable together.

Continue those consumers on Caller, declared operations, Info/Interfaces and the
common module loader/Runtime activation. Registry selection, joined activation,
request recovery, retained clients, Session scope and individual unload remain
required. File and state providers enter through modules with granted Host
capabilities; concrete constructors do not return to generic server assembly.
