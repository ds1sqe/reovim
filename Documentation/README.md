# Reovim editing system

Build a reliable, extensible editing system. Preserve data, make failure and
recovery explicit, then measure performance. Text and Markdown are domains;
the Kernel supports other state models through the same editing mechanisms.

The first product is Markdown source editing with a derived preview, a
TypeScript GUI and a TUI supporting Kitty graphics. Both Clients share one
editing authority and keep independent views. SSR is the GUI's initial
presentation direction; its framework and deployment remain open.

## Design

Start with the [core](Design/00-core.md): everything is a Resource, a
Module serves it through one fixed vtable, and the Kernel has no domain
word. The [client chapter](Design/02-client.md) is the wire; the
[walkthroughs](Design/03-walkthroughs.md) test both. These are target
contracts; source and tests establish implementation status.

The Kernel enforces the tree, rights, declarations, replies and events.
Everything else is a Driver's or a Plugin's choice. Clients own input,
views, rendering and user-facing output; raw keys never reach the Kernel.

[Plugin research](Design/plugins/future.md) covers Client module schemas,
user configuration and additional interaction methods. Its open
interfaces do not change the release requirements below.

## Release scope

v0.16 establishes the editing-system foundation. v0.17 builds Neovim feature
parity on it; plugin/runtime compatibility is not implied. The first Markdown
milestone keeps the v0.16 requirement to ship TUI, CLI and Web, each with usable
Core/Contract/Module/Runtime and production output. Android/native support
remains a future direction.

| Module SDK | Server | TUI | CLI | Web |
| --- | --- | --- | --- | --- |
| Rust | Required | Required | Required | Excluded |
| Python | Required | Optional | Optional | Excluded |
| TypeScript | Excluded | Excluded | Excluded | Required |

These are release requirements, not current availability. Server SDKs concern
Kernel modules; Client columns concern their own module systems. Rust defines
the public semantics first, followed by a tested service reference before
Python/TS extension bindings. A TS GUI alone does not provide its extension
SDK. No embedded engine is selected.

The complete v0.16 cross-client context-sharing path joins TUI, CLI and Web
in one shared scope, with gap/reconnect recovery and slow-member isolation.
The GUI/TUI milestone covers only part of that scope. Public v0.16 SDK/service
APIs remain Experimental until an explicitly admitted full SDK stability
release.

## Reliability

Edits and Undo use one Buffer/History authority. The same path must handle a
small non-text, non-byte-addressed model. The foundation also includes byte/hex
and Text access to one partially resident Resource, preserving invalid source
units through one Buffer/History. Read-only access, creation, volatile-source
gaps and add-only effects retain their own explicit behavior.

Supported behavior needs bounded resources, typed failure/cancellation,
lifecycle, diagnostics and a public path. Behavior tests check actual outcomes;
compilation and retired conformance tooling do not establish runtime support.
[Debugging](debugging.md) owns diagnostics and causal/lock inspection.
[Performance](performance.md) owns reproducible workloads, IPC and
instrumentation cost. Unmeasured data is not measured zero.

Release claims require the exact packaged candidate on each claimed
environment. Failures and unmeasured limits remain visible; numerical budgets
need retained, reproducible baselines.

## Existing code and documentation

Donor behavior is adopted, adapted, rewritten or retired individually. Its
implementation record names the useful behavior, unwanted coupling, new
owner, verification and cutover. Existing code supplies behavior to learn
from; it does not define the target architecture or justify a parallel editor.

Keep each specification focused and at most 250 lines. Explain contracts and
brief rationale together; keep execution plans and machine-specific results
outside the design. Update relevant code/tests/evidence with changed behavior.
[Heritage](heritage/README.md) and [past performance reports](../perf/README.md)
remain historical references.
