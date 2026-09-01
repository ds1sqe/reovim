# Entrypoint Review: `.github/workflows/ci.yml`

## Scope

- Kind: workflow
- Exact content digest:
  `sha256:54820cd83daee64e4a65593554cedc7714444adab4ef24fa0ee37aebab71c45f`
- Review mechanism: exact content and retained review

## Direct local invocations

- `scripts/check.sh` by direct path execution
- `scripts/check-conformance.sh` by direct path execution

The pinned GitHub Actions are external CI tooling and have a separate retained
admission. Shell utility and Cargo commands are external-tool invocations.

## Assumptions

- GitHub supplies the documented Ubuntu 24.04 hosted-runner, `RUNNER_TEMP`, and
  immutable triggering revision in `GITHUB_SHA`.
- Checkout and toolchain actions execute the exact reviewed pins with the
  workflow permissions and inputs present in these exact bytes.
- Each `run` step uses the hosted runner's admitted shell semantics and begins
  in the checked-out repository.

## Decision

Admit these exact workflow bytes and the two declared direct local invocation
edges for the hosted Phase-B matrix. Any byte or edge change reopens review.
