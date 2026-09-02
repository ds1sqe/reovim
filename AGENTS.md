# Reovim Repository Rules

These rules apply to the entire repository. More specific workflow documents
may add constraints but must not redefine the architecture below without also
updating the authoritative documentation.

## Product and release direction

- Reovim is a high-performance, extensible editing system.
- Text editing is a domain, not the shape of the core.
- v0.16 is the editing-system re-foundation.
- v0.17 is Neovim feature parity built on the v0.16 contracts.
- Do not describe a planned contract as implemented without code, tests, and
  conformance evidence.

## Documentation authority

- Read `Documentation/README.md`, `Documentation/00-Concept.md`, and the
  relevant contract before changing an architectural boundary.
- Follow the contract indexes to the focused owning chapters. Keep every active
  normative Markdown file at or below 250 lines and use direct file links
  instead of local heading fragments or duplicate redirect stubs.
- `Documentation/` owns normative terminology, contracts, and conformance
  rules. `docs/` may contain user guides and implementation explanations but
  must not contradict or redefine the normative documents.
- Update the contract, conformance rule, implementation, tests, and migration
  evidence together when a boundary changes.
- Keep temporary investigation and working notes in `tmp/`. Promote durable
  decisions into `Documentation/` rather than relying on temporary notes.

## Architectural invariants

- Preserve the dependency direction in
  `Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/03-Source-Dependency-Graph-and-Edges.md`.
- Keep foundation and editing-state contracts independent of concrete resource
  adapters, Host API adapters, domains, representations, views, transports,
  and extension implementations.
- Do not place strings, lines, columns, grapheme positions, text cursors,
  terminal cells, or another domain's types in the domain-neutral core.
- Do not assume all resources use byte offsets. Keep address, extent, unit,
  revision, capability, and error types explicit.
- Resource adapters connect data. Domains provide meaning. Representations
  expose interpreted state. Projections synchronize it. Views present it.
- A buffer is transactional working state over one resource; it is not a string
  or a mandatory full copy.
- Views and extensions submit edits through representation/domain and
  transaction contracts. They never mutate concrete storage directly.
- Select concrete components only in assembly. Avoid hidden globals and cyclic
  callbacks.

## Host API

- Route environment services through small semantic Host API capabilities.
- Keep platform-specific handles, constants, branching, and dependencies
  inside standard adapters.
- Add a capability only with a concrete consumer, typed errors and limits, a
  standard adapter, a deterministic test adapter, shared contract tests,
  cancellation/shutdown behavior, measurements, and structured events.
- Keep resource adapters and Host API adapters distinct even when a resource
  adapter consumes Host API capabilities.

## Extensions and public contracts

- Treat the Rust SDK as the trusted, latency-sensitive in-process surface.
- Keep the cross-language contract versioned and language-neutral.
- Python and TypeScript SDK extensions run as out-of-process services; do not
  make an embedded interpreter a core dependency.
- Public examples use only public SDK or service-contract APIs. Private core
  types are not an extension contract.
- Require capability negotiation, bounds, backpressure, cancellation,
  lifecycle handling, compatibility tests, and failure containment.
- Third-party extensions and adapters are allowed only after deliberate trust,
  dependency, compatibility, and performance review.

## Dependencies

- The Rust standard library is allowed.
- Add third-party dependencies selectively. Record the concrete need and review
  maintenance, license, transitive cost, unsafe surface, runtime behavior,
  target support, and measured overhead relevant to the use.
- Add an abstraction around a dependency when Reovim owns different semantics,
  needs deterministic substitution, or must contain a volatile or unsafe
  boundary—not merely to hide the dependency name.

## Repository tooling

- Follow `TOOL-001` before adding, continuing, or expanding conformance,
  benchmark, migration, generation, or developer tooling.
- Record the named immediate consumer, exact block, smallest bounded path,
  end-to-end invocation, typed failures, and stop condition before coding.
- Set a plan-owned effort budget and forced review or abort point before tool
  coding. Reaching either stops tool-only work; splitting or renaming it does
  not reset the budget.
- Permit at most one preparatory tool-only slice before the real consumer
  invokes and consumes the result. Fixtures and self-tests do not accept a
  tool by themselves.
- Stop when the named block clears. Do not retain unused compiled experiments
  or use dead-code allowances to hide them. Do not add broader profiles for
  anticipated future use.

## Performance and observability

- Treat input-to-present p99 as the primary reaction metric. Include command,
  transaction, projection, layout, rendering, and presentation handoff inside
  its boundary.
- Measure large/sparse resource latency and memory, extension cost, and logging
  cost. Establish a reproducible baseline before ratifying numeric budgets.
- Every performance claim names the workload and links retained artifacts.
- Emit editor diagnostics through the one structured event stream defined by
  the Editor Event Stream contract. Keep queues and the 1 MiB ring bounded,
  make loss visible, and preserve canonical rendering across ring, file,
  stderr, replay, and `reovim log`.
- Measure instrumentation overhead and test sink failure, pressure, redaction,
  correlation, and shutdown behavior.

## Migration and verification

- Follow `Documentation/05-Migration/01-Selective-Migration.md`. Prior code is
  a behavior donor, not the v0.16 design authority.
- Classify migration slices as adopt, adapt, rewrite, or retire. Do not copy a
  subsystem wholesale because it already works for text.
- Preserve useful behavioral fixtures, but rewrite tests that enshrine invalid
  ownership or coupling.
- A migrated slice needs public-boundary behavior tests, integration smoke,
  fault/cancellation coverage, applicable conformance checks, performance
  evidence, and structured-event verification.
- Remove duplicate paths and stale documentation after the conforming path is
  verified. Do not leave permanent compatibility shims without an explicitly
  tracked owner and removal milestone.
- Place feature parity that is not required to prove the foundation in v0.17.
