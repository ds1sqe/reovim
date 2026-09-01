# Quick Authoring, Admission, and Compatibility

[Extension contract index](README.md)

## Quick Extension Profile

The **Quick Extension Profile** gives a local single-file or small-package
extension the same public contracts with less repetitive setup:

1. the author selects an installed SDK/adapter binding and exact Module target;
2. before source execution, the author or tool supplies role/Family,
   Contribution Point, Activation Predicate, and every non-default grant or
   dependency;
3. Reovim resolves a deterministic minimal manifest with visible deny-by-
   default grants and conservative bounds;
4. generated types, completion metadata, documentation, and one
   capability-scoped context expose only the accepted public surface;
5. one command runs the extension through the deterministic test host and
   reports source-correlated schema, authority, runtime, pressure, and stale-
   handle failures; and
6. promotion to an installed or published package materializes an explicit
   manifest without changing the semantic API.

The synthesized manifest is inspectable, hashable, and retained with evidence.
Manifest synthesis executes or inspects no source behavior and never infers
authority. Published, shared, and installed packages require an explicit
reviewed manifest. Runtime registration remains a typed subset; synthesis is
not a bypass.

Quick Profile replacement is restart-based and generation-fenced. State-
preserving hot reload requires a separately versioned state-migration feature.
Interactive evaluation or a REPL may be offered only by a later concrete
adapter, using an ephemeral Session, bounded output, and visible authority. It
has no implicit current-Buffer mutation. Persistence or automatic startup
requires a declared extension definition.

The Quick Profile does not create a separate configuration/automation role.
Adding such a role requires its own authority, Session, Contribution Points,
manifest and lifecycle, relationship to the official matrix, and exact
evidence.

## Author experience and evidence

Each row designated `RequiredV0.16` or offered as `Supported` ships an idiomatic
package, installation recipe, quickstart, task guides, complete API/error/
lifecycle reference, public runnable examples, testing fixtures,
deterministic conformance runner, debug/causal-trace guide, compatibility and
migration guide, and minimal package template. Examples run from the released
package set without repository-private paths.

At least one independent sample extension for each required or Supported row
proves its documented public path. Clean-environment journeys cover:

- discovery, installation, and exact SDK/adapter/runtime verification;
- creation of a one-file command or contribution;
- inspection of the resolved manifest, API, grants, limits, and versions;
- deterministic testing and diagnosis of schema, type, authority, and runtime
  errors;
- lazy activation and one real public editing or Client/View contribution;
- capability denial, pressure, stale/gap recovery, cancellation, and actionable
  diagnostics;
- process crash/restart where applicable, replacement without stale callbacks,
  packaging, disable/removal, and bounded shutdown.

Every author journey is keyed to one exact language, SDK, binding, placement,
engine when applicable, role, and Family row. A Server command cannot qualify a
TUI or CLI Client contribution; one Client Family cannot qualify another; the
`TestOnly` model cannot establish production usability; and one language cannot
qualify another.

Each concrete candidate owns controls for product-with-adapter-absent,
adapter-packaged-but-inactive, adapter/host-ready-without-extension plus
engine-ready when applicable, and exact-extension-active. Real Scenarios
measure setup steps/time and boilerplate; cold/warm startup; first activation;
warm calls; bridge, marshal, and transport cost; callback, queue, lock, and
input-to-present or command-to-output latency; fairness/backpressure; CPU,
heap/RSS/GC/JIT/code/native/unaccounted memory; cancellation stages; stale
completion; crash/restart; replacement; and shutdown.

Synthetic loops, allocation bombs, callback storms, forced errors, and native
hangs may prove their exact injected transition or calibrate a probe. They do
not alone prove real workload performance, containment, support, or
qualification. Required causal-path claims include every applicable adapter
dispatch, engine entry, guest callback, SDK request, terminal callback, and
fenced installation with exact generations and cancellation stamps. A required
trace gap invalidates that attribution claim.

Measurements follow
[Performance](../04-Performance/03-Path-Overhead-and-Regression-Evidence.md).
Bounded actionable
module/adapter lifecycle and failure summaries enter the
[Editor Event Stream](../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md);
dense traces, causal graphs,
and raw engine/IPC measurements remain evidence rather than a second log.

## Third-party admission

Third-party extensions and adapters are allowed but never implicitly trusted.
Each has a retained admission record naming the approver, exact registry row,
requested grants and data exposure, maintenance owner, license and dependencies,
unsafe/native code, target support, process/placement/isolation, compatibility,
lifecycle, performance workloads, and evidence.

In-process Rust or engine-hosted native code uses the stricter in-process trust
class because process isolation cannot contain its faults. A third-party adapter
passes the same public-path, conformance, authority, bounds, failure, and
qualification gates as a first-party adapter. Carefully selected support does
not create a general arbitrary-language support promise.

## Neovim compatibility is separate

Neovim feature parity does not imply plugin, runtime, or `vim.*` compatibility.
v0.16 selects no Neovim Compatibility Adapter. v0.17 may evaluate one only
after another owner `BindingScope` decision and independent admission.

A truthful future profile is a separately versioned Extension Package above a
selected Engine Adapter when needed and the public Reovim SDK. It pins an exact
Neovim release/API level and exact engine/runtime/ABI/build profile and
enumerates every supported `vim.api`, `vim.fn`, `vim.cmd`, Lua-library,
command, option, event, callback, text-coordinate, loader/package/startup,
native-module, and UI behavior, plus deviations, performance limits, and a
representative test corpus. Embedding Lua or supporting any engine proves none
of this.

The compatibility facade is Text Domain/editor policy, not Kernel authority.
Server/text and Client/UI behavior use separate Module Instances and Contract
Sessions through public Text Domain and Client Family contracts. Neovim Buffer,
current-window globals, byte-column rules, grid UI, or reentrant callback
semantics must not enter the neutral Kernel.

A Reovim wrapper manifest is an intentional admission deviation, not native
Neovim loader fidelity. A compatibility claim separately tests or marks
unsupported the eager manifest-free `plugin/**/*.lua` loader, ordered and
cached `require`, start/optional/`after` packages, per-runtime-directory
Vimscript-before-Lua and other startup ordering, and the distinct generated
remote-plugin manifest.

An in-process `vim.*` facade is not a Protocol Adapter. Actual Neovim
MessagePack-RPC requires an External Protocol Binding plus a Protocol Adapter.
Remote-plugin RPC and remote-UI compatibility are separate profiles; support
for one never implies the other. Engine `Supported` or `Qualified` status never
implies compatibility-profile status.

## Conformance and status

Extension conformance requires:

- public-only Server and Client examples and dependency/public-type checks;
- registry, manifest, Contribution Point, activation, introspection, and
  stable/experimental/private API validation;
- Contract Session, authority, compatibility, cross-language golden, and
  in-memory/production-transport parity;
- Effect Certainty, reconciliation, stale-generation, duplicate, gap/resync,
  cancellation, pressure, process-failure, replacement, and bounded shutdown
  tests;
- atomic attach/readiness/rollback across Server, Family Module, pair, and CCM
  owners;
- exact-package author journeys and row-specific usability evidence; and
- latency, memory, IPC, callback, lock, instrumentation, and failure evidence
  linked to the exact workload and registry row.

The evidence rules in
[Mission-Critical Quality](../07-Mission-Critical-Quality/README.md) and
[Conformance Requirements and Evidence](../06-Conformance/01-Requirements-and-Evidence.md)
apply without substitution. Every newly selected official binding row enters
as `PlannedMissing`; the Adapter Kit's synthetic model is always `TestOnly`;
and v0.16 selects no production Engine Adapter or Neovim compatibility profile.
Live registry and evidence status belongs outside normative Documentation.
