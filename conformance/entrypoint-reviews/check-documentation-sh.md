# Entrypoint Review: `scripts/check-documentation.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:a90292c883cbd8402fbb6042b42623ad8cb33d8e57417a1b67675b007616b022`
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
- `BASH_SOURCE` resolves this checked-out script so `ROOT` selects the reviewed
  repository inputs.

## Decision

Admit these exact bytes with an empty direct-local-invocation set for
documentation conformance. The script binds both the focused
Interaction/Update/View/Protocol chapters and the focused Product-v2 proof
chapters to their inventories, indexes, authority clauses, and Rule Matrix
routes. It also binds the focused Host API chapters and initial startup Event
family to their navigation indexes. It includes `CODE_STYLE.md` in public
Markdown link, whitespace, fence, and local-fragment validation. Any byte or
invocation change reopens this review.
