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
toolchain/CI integration, handshake-loss recovery, richer SDK ownership helpers,
the editing UI, extension SDK and packaged qualification remain required.
This connection witness is not the planned Markdown GUI.
