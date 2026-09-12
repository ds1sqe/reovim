# Browser service reference

This small loopback HTTP adapter forwards the existing public service protocol.
It supplies no editing authority and does not replay operations automatically.
The TypeScript client keeps Resource IDs and receipts as exact strings and
uses bigint for u64 values. Callers persist receipts before send, adopt returned
owners/results, then release delivery storage separately.

With Node and TypeScript installed:

```sh
bash apps/web/check.sh
node apps/web/bridge.mjs 127.0.0.1:7878 8080
```

Open the printed browser URL. The connection witness acquires a coherent
Session snapshot/watch, journals adoption across reload, and explicitly releases
both handles. Start the Kernel service with a configured shared Session first.

The adapter pins a loopback backend, checks Host/Origin and a custom RPC header,
limits frames and concurrent forwarding, and preserves transport uncertainty.
It accepts no arbitrary proxy target or filesystem path. Local processes remain
within the existing trust model. Closing HTTP does not cancel admitted work.

apps/web/package.json declares exact Node and TypeScript toolchain versions.
The shared check.sh/ci.sh runner verifies them and executes this build and its
transport tests in every lane. Local checks require those tools on PATH and
install nothing. GitHub provisions the declared tools before the same runner.
Real-browser and dedicated native-process journeys remain additional checks;
this build gate does not qualify packaged browser behavior. Richer SDK helpers,
the editing UI, extension SDK and packaged qualification remain required.
This connection witness is not the planned Markdown GUI.

The browser persists an opening secret before startup. A lost handshake is
recovered through that secret without creating another Client. It journals
the returned token/result before releasing startup delivery storage. Missing
evidence and failed startup remain retained errors for explicit reconciliation.

The `/edit` contract trial creates a source Buffer and client-owned Text access,
reads bounded scalar/opaque windows, replaces explicit scalar ranges and uses
the same History for Undo. Configure its providers before starting the bridge:

```sh
cargo build --manifest-path product/Cargo.toml -p reovim-server \
  --features reovim-server/std
cargo build --manifest-path product/Cargo.toml \
  -p reovim-module-source --example source \
  -p reovim-module-text --example text
product/target/debug/reovim-server 127.0.0.1:7878 \
  --client-module source product/target/debug/examples/libsource.so source \
  --client-start source.model 1 \
  --client-module text product/target/debug/examples/libtext.so text \
  --grant "" all --client-start text 1
```

After starting the bridge, run `node apps/web/editing-e2e.mjs BROWSER_URL`
to exercise editing, lost-reply recovery, stale input, Undo and cleanup against
those real providers. It requires a dedicated test server. The browser trial
journals ownership and never retries edits with a new receipt. Recovery settles
the pending operation; use Observe afterward to acquire current Text. Its
initial hex field can create invalid source bytes without lossy decoding.

Check changes reads the current Text watch without waiting. Events and Gap
renew observation and release the old snapshot/watch; Pending and End remain
distinct. Recovery preserves the original delivery before refresh and does not
consume another event. The old snapshot remains a qualified historical view
until renewal; notification alone does not change its content or revision.

The trial can also join a server-owned Buffer with independent Client Text
views. For that profile configure the source Model at server scope:

```sh
product/target/debug/reovim-server 127.0.0.1:7878 \
  --module source product/target/debug/examples/libsource.so source \
  --start source.model 1 --buffer source source.initial 1 2320736861726564 \
  --client-module text product/target/debug/examples/libtext.so text \
  --grant "" all --client-start text 1
```

Choose Join shared source in independent browser tabs. Detach releases only
that Client's Text owners; it cannot discard the shared Buffer. Run
`node apps/web/shared-e2e.mjs BROWSER_URL` against a fresh dedicated server with
the above initial bytes to verify independent edits/views, stale input, shared
Undo, denied shared close and independent detach/rejoin.

This trial does not implement shared Session focus,
continuous following, Markdown preview or SSR. The Text TS
codec lives with its module; the common service client has no Text semantics.

Undo and Redo buttons, Ctrl/Meta+Z, Ctrl/Meta+Shift+Z and Ctrl+Y use native
History. History navigation waits for pending input and temporarily locks the
textarea until its accepted state is observed. Browser-native History input
events use the same path; IME key events remain with composition.

The textarea input trial maps browser UTF-16 selection and normalized newlines
back to the observed Text window. It preserves untouched CRLF and opaque bytes;
the object marker represents one opaque byte. Composition remains a local draft
until completion. Undo uses shared History rather than a second document store.
The input queue holds at most 32 edits over a view of at most 1024 Text units.
Typing continues while one edit is in flight. Each accepted revision and its
semantic window must match the predicted basis before the next queued edit is
sent. Peer changes, mismatched views, lost replies and pressure retain the
remaining draft for reconciliation. Composition stays local until completion.
Queued operations survive reload as locked draft data; they are not replayed
automatically. Recover and resume verifies the queue head's original Client/
receipt acceptance and current semantic state, then sends only remaining edits.
Missing proof, peer changes or unqueued draft text require manual reconciliation.
Platform IME and larger-window input remain further work.
Input and composition drafts are saved in the same session journal before
transmission and survive reload as locked local text. Refresh does not discard
them or submit them again. Recover admitted work through its existing receipt,
then explicitly discard the local draft when appropriate. Discarding a draft
does not cancel or forget a pending request. Session storage is not persistence
across browser/session loss. This is input-contract evidence, not the finished GUI.
