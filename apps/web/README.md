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

The local browser check is separate from the pinned Rust CI window. This
initial profile was tested with Node 26.8.0 and TypeScript 6.0.3. Browser build
toolchain/CI integration, richer SDK ownership helpers,
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

This trial owns a private source; it does not implement shared Session focus,
continuous following, Markdown preview, keyboard/IME editing or SSR. The Text TS
codec lives with its module; the common service client has no Text semantics.
