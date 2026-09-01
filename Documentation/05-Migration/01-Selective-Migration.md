# Selective Migration

## Policy

Historical Reovim code is a behavior donor, not the v0.16 design authority.
It is inspected through Git or a named sibling worktree and is not copied into
the active source tree as an archive. Existing text-neutral names do not prove
a text-neutral boundary, and successful OS-era behavior does not justify an
OS-shaped product or Host API.

Every candidate behavior is classified independently:

- **adopt** — behavior and boundary already conform; retain the smallest
  useful implementation and its proof;
- **adapt** — behavior is valuable but moves behind a ratified contract;
- **rewrite** — retain fixtures and knowledge because the implementation shape
  would preserve invalid ownership or coupling; or
- **retire** — remove duplicate, obsolete, out-of-scope, or unjustified
  behavior.

Classification follows user value, contract fit, and evidence—not file age,
code volume, or the convenience of a bulk cherry-pick.

A donor behavior is valid migration input only when it is accepted by a current
public contract, a retained supported user journey, or an explicit owner
decision recorded in the slice. A passing historical test alone does not make
the behavior valid or in scope.

## Migration record

Each slice keeps an auditable record outside normative Documentation with:

- user-visible behavior and release-scope justification;
- exact donor source and behavioral fixtures;
- known defects and current dependency/ownership shape;
- text, presentation, Host, transport, global-state, and unbounded-work
  coupling;
- target layer and public contracts;
- adopt/adapt/rewrite/retire decision and rationale;
- behavior, failure, cancellation, integration, performance, memory, and Event
  Stream evidence requirements; and
- cutover conditions plus removal conditions for superseded code or a bridge.

The record links retained artifacts rather than pasting mutable results into
the specification.

## Stable slice procedure

1. Characterize the donor through its real public surface and preserve only
   valid observable behavior as fixtures.
2. Map each responsibility to exactly one applicable logical role in the
   [Layer Model roles](../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/01-Logical-Roles-and-Source-Ownership.md),
   including qualified Resource, Host, External Protocol Binding, Protocol,
   Extension Adapter/Engine, Client Family/Runtime, Presentation, Kernel, and
   Assembly boundaries. Do not collapse two roles because donor code combines
   them. Amend the contract before code if a semantic need is missing.
3. Build the end-to-end vertical path whose component scope is justified by the
   slice's acceptance scenario. The slice record maps every included component
   to a required proof step and explains any retained bridge; subjective
   “smallest” claims and default horizontal subsystem migration are not a scope
   argument.
4. Compare donor and target behavior, explain intentional differences, and run
   applicable conformance, performance, memory, and Event Stream evidence.
5. Cut assembly over only after the replacement is proved; then remove the
   duplicate path, stale documentation, dead feature gates, and unapproved
   compatibility shims.

Each implementation phase, contract-changing implementation phase, and cutover
phase includes a composed integration smoke, not only isolated unit tests.
Analytical characterization and responsibility-mapping phases retain review and
traceability evidence instead of inventing a pre-implementation smoke.

## Slice acceptance

A migrated slice is accepted only when ownership and dependencies conform,
public-boundary behavior and failure cases pass, standard and deterministic
adapters cover every consumed Host capability, relevant p99 and memory evidence
is retained, required events and loss behavior are verified, public extension
impact is documented, and superseded active paths are removed.

Resource behavior uses the family-owned deterministic fault fixture/model
oracle; Client presentation behavior uses its deterministic sink/golden
harness; Protocol behavior runs through in-memory and production adapters.

Applicable slices additionally prove Buffer-incarnation and Projection-
generation fencing, definitely-not-applied versus outcome-unknown mutation
results, transactional readiness/rollback, bounded retry and concurrency,
authority-before-lookup, Client Representation Gap/resynchronization, and reverse-order
shutdown.

v0.16 accepts the editing-system foundation and only the behavior needed to
prove and use it. Neovim feature parity is a v0.17 responsibility and must use
the v0.16 contracts rather than bypass them. This release boundary is a stable
product rule, not an execution schedule.
