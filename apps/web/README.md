# Browser service reference

**Keep source open** releases the current Text view while retaining its Buffer
and History. Create or open another source, then use **Other open sources** and
**Switch source** to return. Switching reacquires current Text and preserves the
previous window start; it does not close a Buffer or send an edit. The journal
keeps inactive identities through reload and recovers lost view-cleanup replies.
Retained drafts and composition prevent switching. Closing affects only the
current source under the existing discard/detach policy.

With a live context, **Keep source open**, **Switch source** and **Focus current
source** update its known/active state before changing Text views. Creating a
private context includes currently owned Buffers and the displayed source.
The coordinator records the exact context receipt; a matching active value is
not acceptance evidence. Recover an uncertain switch before editing. Accepted
context changes survive failed view acquisition, and both Buffers remain owned.
Closing the current source first removes it from the context's known set and
clears active only if it names that source. Close/detach then follows the
existing policy. A lost close reply is recovered without another context update.
A rejected close leaves the Buffer owned, ends that close decision, and does
not roll back the accepted context removal. Observe and make a new decision;
unseen concurrent edits are never silently discarded by refreshing the revision.
With a live context, creating, opening or joining adopts Buffer ownership first,
then updates membership/focus, then acquires Text. The opening intent survives
reload and recovery never repeats preparation. A rejected context update leaves
the adopted Buffer owned without a new view; refresh/reselect explicitly.
Follow and retained-context seek select the observed active Buffer without
writing context or undoing Buffer contents. Historical context shows current
contents read-only in this trial. The context provider grants READ references;
an unfamiliar peer Buffer uses `text.attach.read`, never writable attachment.
Existing writable views keep their independently held grants. Missing or closed
selections retain previous Buffer ownership and report a view failure.
Run `node apps/web/switching-e2e.mjs BROWSER_URL` on the private source/Text
fixture, or add `files` with `one.md` containing `first\n` and `two.md` containing
`second\n` plus file/open providers. The journey checks dirty state, independent
History, lost cleanup/reload, failed opening and eventual client cleanup.
`context-switching-e2e.mjs BROWSER_URL` additionally requires the per-client
context provider and checks focus/view/cleanup loss, reload and stale focus.
`context-opening-e2e.mjs BROWSER_URL create|file|join` checks all three opening
stages through loss/reload and stale focus. File mode needs `draft.md` containing
`opened\n`; join mode needs a shared Buffer containing `shared`.
`context-selection-e2e.mjs BROWSER_URL` uses an initially empty shared context
to test seek/follow loss, current Buffer contents and read-only peer selection.

The editing trial's **Open file** control calls the separately loaded
[open command](../../modules/open/README.md). Composition supplies per-client
`open` and `file_source` (the Source opening Model), plus a granted directory
at `modules/files`. It also needs the existing per-client Text provider.
The filename is relative to that explicit Host grant. Opening returns a backed
Buffer descriptor; its source identity grants no incidental source operations.
The current backing is read-only. Conditional saving remains unimplemented.

Opening persists its receipt and adopts the owned Buffer before Text observation
or context attachment. Recover a lost response before another opening; changing
the disk file does not change the retained result. **Focus current source** is a
separate exact-current context update. A stale or lost focus update leaves the
opened Buffer owned by this Client. Refresh/recover context before retrying it.
`node apps/web/file-context-e2e.mjs BROWSER_URL FIXTURE_PATH` exercises this chain
on a dedicated server; it deliberately replaces the test fixture after opening.
An observed nested Unknown uses `command.open.resume`: the journal preserves
the original receipt until its granted work reference is adopted, then retains
the resume receipt. Reload can recover either step without another opening.
After Buffer adoption it retires outer work newest first and retries command
cleanup before reporting recovery complete. Cleanup failure retains the Buffer.
It reads retained outcomes before resuming. A confirmed failed stage is cleaned
up and reported without installing a Buffer or retrying the filename; a later
new opening is allowed. Unknown or unavailable evidence remains owned.
`nested-file-e2e.mjs BROWSER_URL FIXTURE_PATH` uses the native `open_fault`
artifact to test both stopped stages and interrupted lookup/receipt retirement.
`failed-file-e2e.mjs BROWSER_URL` selects its `fail.md` initialization fault and
checks failure cleanup followed by a successful new opening.
`backed-file-e2e.mjs BROWSER_URL` uses a `draft.md` containing `# peers\n` to
check two Clients' shared backing, independent state/History and close ownership.

The editing trial can dynamically load the first-party uppercase contribution,
run it on the textarea's highlighted selection, and unload it. It uses only the
[GUI contribution contract](../../clients/gui/contract/README.md); opaque units
refuse conversion and native Text/History owns acceptance. This is an initial
public author/lifecycle path, not the complete packaged extension SDK.
Run `node apps/web/contribution-e2e.mjs BROWSER_URL` against a dedicated private
source/Text trial server to verify that contribution through the public path.

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

For private retained focus, append this client module to the server command:

```sh
--client-module context product/target/debug/examples/libcontext.so context \
  --grant "" all --client-start editing.context.model 1
```

Build its example with `cargo build --manifest-path product/Cargo.toml
-p reovim-module-context --example context`. Create private context starts with
no known/active Buffer; Focus current source adds the current Buffer with READ
references. Clear focus keeps known references. Context updates/close never
edit or close those Buffers. Snapshots and delivery receipts retire separately,
including after interrupted replies. Run `node apps/web/context-e2e.mjs
BROWSER_URL` against a dedicated shared-source/context server for the two-Client
journey. Each context uses owned native participation. Follow live context
acquires a coherent snapshot/watch and checks delivery.
Events or Gap renew context and release obsolete snapshots/watches separately.
Lost delivery recovers the original receipt before renewal; it cannot consume
another event as recovery. Context observation does not edit or close Buffers.

The native participation journey uses that same dedicated server configuration:

```sh
node apps/web/participation-e2e.mjs BROWSER_URL
```

It exercises the loaded context Model's reference-bearing updates, historical
seek, coherent follow, lost-result recovery after an intervening update, stale
ABA rejection and detach cleanup. Buffer revision stays unchanged.

For shared context, add a server-owned context Model and initial Session to
the shared-source configuration, alongside any private Client context Model:

```sh
--module context product/target/debug/examples/libcontext.so context \
  --grant "" all --start editing.context.model 1 \
  --session context editing.context.initial 1 ffffffffffffffff
```

Join shared context creates independent participation in the `session` alias.
Retain context checkpoint owns one additional snapshot; replacing it releases
the old checkpoint. View retained context selects that observation without
rewinding text. Focus changes require Follow live context while viewing history.
Close or detach context releases participation, watches and snapshots, and
closes the Session only when this controller created it privately.

Attach, checkpoint, seek and teardown recovery preserve the original receipt
and ownership across reload. Restoration sends no work by itself. Run
`node apps/web/context-participation-e2e.mjs BROWSER_URL` on the dedicated
shared-context/source server for two-Client views, interrupted lifecycle steps,
shared content/Undo and cleanup. Automatic open/focus policy and the finished
GUI remain further product work.

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
