# Inventory, Includes, and Evidence Publication

[Source Classification and Exact Checker](../04-Source-Classification-and-Exact-Checker.md)

## Repository inventory and include closure

Phase-B v1 scope is the exact repository-root singleton `roots: ["."]`.
Every in-scope repository path is classified exactly once, including committed
policies, topology, review registries and records, schemas, checker and helper
sources, tests, and scripts. Its exclusion set is closed to exact root `.git`
as VCS metadata, root `target` as transient build output, and root `tmp` as
working notes. An exclusion is never treatment or a waiver for a controlled or
consumed input. Unmatched paths, lexical or symlink root escape, and equal-
precedence multiple matches are conformance errors. Stronger claims that an
execution mechanism discovered every consumed input require the mechanism-
specific evidence named by the applicable contract.

Each admitted script/workflow entrypoint binds its exact content digest in both
the committed topology and the one-to-one entrypoint-review registry. The
registry's explicit direct repository-local invocation set and retained review
record are the Phase-B authority; exact byte drift invalidates that review.
The checker validates registry/schema identity, coverage, declared-edge
resolution, review-record digest and classification, and a closed set of
defense-in-depth lexical warnings. It does not parse or interpret shell or YAML
and does not infer a complete invocation closure. Unsupported or dynamic
repository invocation is forbidden by review; stronger semantic proof requires
a dedicated parser- or interpreter-produced evidence source.

For all-tooling v1, a relative string-literal `include_str!` or
`include_bytes!` input is resolved relative to its Rust source, must remain
inside the repository, and must be present in the classified inventory.
`include!`, an absolute or non-literal include path, a missing input, or an
excluded/out-of-root input fails closed. Product include and macro expansion
remain part of the later compiler-evidence gate. The included input is
`RepositoryTooling`: a `Tool` source requires `Tool`; a `Test` or `Example`
source permits shared `Tool` or the same non-shipping use; and `BuildOutput` is
never a direct source input. Source Use remains a participation label, not a
second dependency partition.

## Evidence destination and publication

Generate mode rejects an evidence destination inside the repository root before
creating or writing it. Generated evidence is out-of-tree, never an exclusion,
and never a checker policy input.

Evidence publication uses the content-addressed no-clobber final link as its
commit point. Success requires complete owned-temporary cleanup and established
report-file, leaf-directory, parent-directory, and evidence-root durability. A
post-commit fault returns `Applied` with the final path plus explicit cleanup
and durability state; a pre-handoff fault returns `NotApplied` with any intended
path and residual cleanup state. A handed-off link whose reconciliation probe
is itself indeterminate returns `OutcomeUnknown` with the intended path and
cleanup/durability state. No cause erases or invents Effect Certainty.

Phase-B exact publication is Linux-only and holds directory and temporary-file
handles through commit. Creation, link, unlink, and synchronization operate
through held `/proc/self/fd` directory anchors; nominal directory identities
are revalidated before commit and after synchronization, and the final entry
must name the synced temporary inode. Directory substitution cannot redirect
publication: substitution observed before the link is `NotApplied`; an identity
failure after the link is `Applied` with cleanup/durability state preserved.
Cleanup removes only entries still naming held checker-owned inodes and verifies
their post-removal link counts; replacements are never deleted and displaced
owned entries remain visibly `Incomplete`. A hard-link error is reconciled with
three results against the held temporary inode: confirmed link is `Applied`,
proved absence/difference under the admitted authority model is `NotApplied`,
and probe failure is `OutcomeUnknown`. An already-applied or indeterminate final
link cannot be reported `NotApplied`.

Publication authority is a canonical, initially empty, owner-only `0700`
evidence root outside the repository whose parent either excludes other writers
or applies owner-protecting sticky semantics. The checker creates and syncs one
fixed `create_new` authority marker, holds its inode through the operation, and
retains it permanently; a failed attempt consumes the one-shot root. After
acquisition, only that marker may exist before bundle creation. Compliant
publishers honor the marker. Host/root intervention, compromised-host mutation,
and hostile concurrent mutation by the same filesystem authority are outside
this Phase-B proof and cannot inherit its claim.
