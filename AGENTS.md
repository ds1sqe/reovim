# Reovim agent rules

Build a reliable, high-performance, extensible editing system. Text is one
domain. Prefer fewer concepts, explicit ownership and light, readable code.
Design quality takes priority over delivery speed; there is no imposed deadline.

## Authority and work

Read [goals](Documentation/README.md) and
[Design](Documentation/Design/README.md) before changing a boundary.
Documentation owns goals/contracts; plans own execution order. Target prose
is not implementation evidence. Keep each specification at most 250 lines,
define terms before use and link to each rule's owner rather than repeating it.

Research and agree the used interfaces before inner mechanisms. Make small,
coherent changes; compile public providers/consumers and exercise the journey.
Extract useful donor behavior, not a parallel legacy implementation.
Preserve unrelated work, perf/ and Documentation/heritage/.

## Boundaries

- Resource is the common subject model, not mandatory bytes or storage.
  Buffer, History and retained Session context use that same identity model.
- Runtime owns live contexts and execution. Interface declares typed operation
  tables; consumers depend on Interface, not concrete Runtime/Core types.
  Composition selects implementations and binds them to actual contexts.
- Kernel dependencies point inward: Runtime -> Core -> Interface -> Foundation.
  Each layer needs a distinct purpose; do not duplicate public type catalogs.
- Core owns atomic Buffer/History installation. No I/O, waiting, callbacks or
  fallible work after the commit cut under that authority. Revalidate first.
- Clients own semantic response and presentation policy. Kernel knows neither
  text coordinates nor concrete viewports. User input handling, view layout,
  rendering and user-facing output are outside Kernel responsibility. Host
  grants apply separately to Kernel, Client and Resource consumers; native
  details stay in adapters.
- Foundation is core-only; portable Kernel/applicable Rust Client paths stay
  no_std + alloc. Hosted adapters/assembly may use std.
- Use public contracts for modules/examples. Internal APIs may change.
  Public v0.16 SDK/service APIs remain Experimental. Review dependencies and
  third parties deliberately. Grants are not an in-process memory sandbox.

## Verification and style

Follow [contributing](CONTRIBUTING.md) and [coding style](CODE_STYLE.md):
80 columns, at most three Rust block-nesting levels, small functions and
explicit ownership. Let rustfmt own Rust whitespace.

Use scripts/check.sh during work and scripts/ci.sh before checkpoints.
ci.json owns the exact stable/nightly window; normal runs never change it.
Local and GitHub CI use the same full runner. Compilation alone proves neither
architecture, runtime support nor release qualification.

Test semantic values, relations and outcomes, not content hashes, serialized
lengths or collection counts as proxies. Named protocol/safety bounds may use
exact values. Git owns file identities; never hand-edit hash manifests or
generated fields. Cache complete component inputs/toolchains; uncertainty runs
all checks. Required checkpoint/release evidence remains uncached.

Follow [debugging](Documentation/debugging.md) and
[performance](Documentation/performance.md): one bounded diagnostic stream,
actual causal evidence, truthful failure, real workloads and retained raw data.
Measure instrumentation and lock cost; elapsed time alone is not deadlock proof.

## Tooling

Before tool coding, name the immediate consumer/block, smallest path,
invocation/failures, effort budget and stop condition. At most one preparatory
tool-only slice precedes real use. Stop when the block clears or review is due;
renaming does not reset it. Keep no unused framework. Measure the roughly
one-minute warm-check target instead of assuming it.
