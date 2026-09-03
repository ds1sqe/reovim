# Entrypoint Review: `.github/workflows/ci.yml`

## Scope

- Kind: workflow
- Exact content digest:
  `sha256:b3f71d42639c90dbf5cb063e19ece01518ed51f8ce1f20b223d845cd36c772d6`
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
- The contributor invocation passes `--full`; CI never consumes local cache
  hits as verification evidence.
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
