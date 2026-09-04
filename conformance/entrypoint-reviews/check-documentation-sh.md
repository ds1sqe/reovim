# Entrypoint Review: `scripts/check-documentation.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:bef319d36f5d1877f43be33588ea12ff32d37c0a5e1093b2abb83f95988a6685`
- Review mechanism: exact content and retained review

## Direct local invocations

None. The script reads `CODE_STYLE.md`, repository documentation, the CI
workflow, and CI action review inputs, but it does not execute another
repository entrypoint.

## Assumptions

- `/usr/bin/env` resolves a Bash implementation supporting the constructs used
  by these exact bytes.
- The GNU/Linux command-line tools resolved from `PATH` provide the semantics
  exercised by the retained mutation checks.
- Ambient Ripgrep configuration is removed before structural searches.
- `BASH_SOURCE` resolves this checked-out script so `ROOT` selects the reviewed
  repository inputs.

## Decision

Admit these exact bytes with an empty direct-local-invocation set for
documentation conformance. The script binds both the focused
Interaction/Update/View/Protocol chapters and the focused Product-v2 proof
chapters to their inventories, indexes, authority clauses, and Rule Matrix
routes. Navigation-only shape is checked through exact child and section
semantics rather than current-file content hashes. It also binds the focused
Host API chapters and initial startup Event family to their navigation
indexes. It includes `CODE_STYLE.md` in public Markdown link, whitespace,
fence, and local-fragment validation. The Host inventory now binds the
bootstrap-health index and its four focused ownership/schema, proposal,
retention/snapshot, and import/readiness children beneath the six-child Host
index. The retention child is itself a navigation-only index for descriptor/
lease, counter/sequence/retention, and owner-snapshot chapters. Any byte or
invocation change reopens this review.

The Architecture inventory also binds the navigation-only Ports, Source Tree,
and Portability index and its layer-interface, source-placement, and portable-
Host children. Direct Rule Matrix routes cover their interface, placement, and
`no_std` authorities plus Client Host applicability, vertical migration order,
and SDK stability state. These additions use the existing bounded document
inventory, link, topology, authority, and taxonomy passes; they add no command,
dependency, generated output, or broader execution mechanism.
