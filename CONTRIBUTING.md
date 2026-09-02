# Contributing to Reovim

## Read the contract first

Before changing an architectural boundary, read:

1. [Documentation authority and map](Documentation/README.md);
2. [Concept](Documentation/00-Concept.md);
3. [Vocabulary](Documentation/00-Vocabulary.md);
4. [Architecture and layer model](Documentation/01-Architecture/README.md); and
5. the owning contract and its [Rule Matrix](Documentation/06-Conformance/02-Rule-Matrix.md)
   rows.

Normative Documentation defines the target; it is not evidence that current
code conforms. Plans own implementation order. Historical code is a behavior
donor and does not define v0.16 architecture.

Contract indexes route to focused normative chapters. Keep each active
normative Markdown file at or below 250 lines, link directly to the owning file,
and do not preserve a retired monolith as a second authority.

Before changing source, read and follow the repository-wide
[coding style](CODE_STYLE.md). Human-authored source uses an 80-column maximum
and no more than three Clippy-counted Rust block-nesting levels.

## Change discipline

- Preserve inward dependency direction and one semantic authority per state.
- Keep text, viewport, platform, transport, and adapter implementation types out
  of the Domain-neutral Kernel.
- Use public SDK/Module/Client contracts for first-party behavior.
- Update contract, rule mapping, implementation, public-boundary tests,
  migration evidence, Events, and performance evidence together when
  applicable.
- Classify migrated behavior as adopt, adapt, rewrite, or retire.
- Do not leave duplicate paths or permanent compatibility shims after cutover.
- Add dependencies only with the review required by the Layer Model.

## Verification

Run the repository check suite:

```sh
./scripts/check.sh
```

At minimum, relevant changes require formatting, warning-free builds, strict
lint, focused and workspace tests, documentation links/terminology checks, and
the evidence classes named by applicable Rule Matrix rows. A happy-path test
does not substitute for fault, cancellation, lifecycle, performance, Event, or
exact-artifact evidence.

Do not claim support or qualification from a source-tree test. Qualification
uses the exact packaged candidate in its named clean environment.

## Pull requests

A pull request should:

- link its owning issue/plan and normative rules;
- explain user-visible behavior and architectural ownership;
- identify donor behavior and migration disposition where applicable;
- include commands and retained evidence, including failures;
- state compatibility, dependency, unsafe/native, performance, and release
  effects; and
- remove superseded active paths after the conforming replacement passes.

## Contributor license terms

By contributing, you grant ds1sqe a perpetual, worldwide, non-exclusive,
royalty-free, irrevocable license to use, reproduce, modify, prepare derivative
works from, publicly display or perform, sublicense, and distribute your
contribution and derivative works. You represent that the contribution is your
original work or that you have the authority to grant these rights.
Contributions may be included in AGPL-3.0 and commercially licensed versions
of Reovim.
