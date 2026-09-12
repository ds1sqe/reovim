# Contributing to Reovim

Read [the goals](Documentation/README.md), then
[the resource-based design](Documentation/Design/README.md).
Specifications define the target; implementation requires code and evidence.

Browser checks require the Node and TypeScript versions declared in
apps/web/package.json on PATH. The shared runner verifies them and runs the
browser build/transport tests before Rust cache selection on every invocation.
Local checks do not install browser tools. GitHub provisions the same declared
versions before calling that runner.

## Make a coherent change

- Agree the used interface chain before its inner mechanisms. Compile public
  providers/consumers, then run the actual journey.
- Preserve inward dependencies, one editing authority, source fidelity and
  explicit error/effect outcomes. Keep Client view policy outside the Kernel.
- Classify donor behavior as adopt, adapt, rewrite or retire. Do not copy a
  subsystem just because it already works for text.
- Keep contracts, code, behavior/fault tests and evidence consistent.
- Use public SDK/service/family APIs in examples and first-party modules.
- Review dependency maintenance, license, targets, unsafe and runtime cost.
- Follow [coding style](CODE_STYLE.md): 80 columns, three Rust nesting levels,
  rustfmt, small functions and explicit ownership.

## Verify

```sh
./scripts/check.sh
./scripts/ci.sh
```

The default gate selects affected components and reverse dependents using
Git file identities and successful toolchain-bound local records. Unknown
ownership or invalid coverage runs the full gate. No manually tracked hash
manifest is needed. check.sh --full runs its uncached checks for one installed
toolchain. ci.sh is the checkpoint/CI runner: it runs the same complete window
locally and in GitHub, with no cached-result shortcut.

ci.json owns rust-stable and rust-nightly as exact version pins. The window
tracks the selected latest stable and dated nightly releases. The runner
installs those pins/components/targets, fetches locked dependencies into a
fresh Cargo home, and invokes check.sh --full. This requires network access.
Use ci.sh --toolchain stable or nightly for one lane; --matrix shows the
GitHub matrix. Normal runs never change ci.json. To update the window, change
its pins, run the full matrix, review the results, then commit the manifest.

The runner shares commands, toolchains and configuration with GitHub CI;
it does not claim identical host OS, system libraries or physical hardware.
The current runner requires Linux x86_64 and an existing rustup installation.

The gate checks style, links, product formatting, builds/tests, Clippy,
rustdoc and portable library compilation. Cargo commands use
--manifest-path product/Cargo.toml. Review actual Cargo dependencies and
public types: bare-target compilation alone proves neither core-only
Foundation nor absence of every forbidden architectural edge.

Use semantic tests, not literal checkout hashes, lengths or package counts.
Each changed journey also needs applicable failure/cancellation, lifecycle,
structured events and causal evidence. Measure performance on reproducible
scenarios and retain raw artifacts. Report failures, not only successful runs.

The former exact source/compiler/package proof system is retired. Ordinary
tests do not claim its automated proof or packaged release qualification.
A release claim tests the exact packaged candidate on its claimed environment.

## Submit

A change explains the owning issue/plan, public behavior, responsibility,
donor/dependency decisions, exact verification and limits. Do not claim an
unimplemented contract as supported. Remove superseded paths only after
the replacement is checked; preserve unrelated or uncommitted user work.

## Contributor license terms

By contributing, you grant ds1sqe a perpetual, worldwide, non-exclusive,
royalty-free, irrevocable license to use, reproduce, modify, prepare derivative
works from, publicly display or perform, sublicense, and distribute your
contribution and derivative works. You represent that the contribution is your
original work or that you have the authority to grant these rights.
Contributions may be included in AGPL-3.0 and commercially licensed versions
of Reovim.
