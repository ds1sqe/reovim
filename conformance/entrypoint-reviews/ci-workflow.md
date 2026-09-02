# Entrypoint Review: `.github/workflows/ci.yml`

## Scope

- Kind: workflow
- Exact content digest:
  `sha256:033269df23b9ff12496f77f1730b663a17cd3932044a094af1352ce7119e10e5`
- Review mechanism: exact content and retained review

## Direct local invocations

- `scripts/check.sh` by direct path execution

The pinned GitHub Actions are external CI tooling and have a separate retained
admission. Shell utility and Cargo commands are external-tool invocations.

## Assumptions

- GitHub supplies the documented Ubuntu 24.04 hosted runner and `RUNNER_TEMP`.
- Checkout and toolchain actions execute the exact reviewed pins with the
  workflow permissions and inputs present in these exact bytes.
- Each `run` step uses the hosted runner's admitted shell semantics and begins
  in the checked-out repository.
- The five-minute job timeout bounds dependency fetch, the selected fast
  contributor check, and cleanup together.
- The mutable hosted image has no exact execution-tool admission or retained
  namespace/prepared-root measurement. This workflow makes no exact-evidence
  claim until both exist.

## Decision

Admit these exact workflow bytes and the one declared direct local invocation
edge for the hosted stable/Rust 1.92 contributor matrix. Exact evidence remains
a separate clean-revision journey in an admitted environment. Any byte or edge
change reopens review.
