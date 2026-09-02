# Entrypoint Review: `scripts/check-documentation.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:f4d93d63c0a90b65d300b255b704c89208a7c75ea23205cc459a4ed5fde09d06`
- Review mechanism: exact content and retained review

## Direct local invocations

None. The script reads repository documentation, CI workflow, and CI action
review inputs, but it does not execute another repository entrypoint.

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
routes. It also binds the unique Repository Tooling admission/stop authority,
applicability, and Rule Matrix route. Any byte or invocation change reopens
this review.
