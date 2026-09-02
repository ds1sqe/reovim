# Entrypoint Review: `.github/workflows/ci.yml`

## Scope

- Kind: workflow
- Exact content digest:
  `sha256:b92cb695e19e195514f6f2485d101a2b77b46cc148ede5d86e3a952441f9961c`
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
- The five-minute job timeout bounds dependency fetch, the selected fast
  candidate check, and exact-revision evidence together.

## Decision

Admit these exact workflow bytes and the two declared direct local invocation
edges for the hosted Product-empty v2 matrix. Any byte or edge change reopens
review.
