## Summary

<!-- Describe the user-visible outcome and the responsible boundary. -->

## Authority and scope

- Owning issue/plan: #
- Owning specification:
- Public contract(s):
- Release/support effect:

<!-- Review contract changes with their behavior and failure evidence. -->

## Changes

-

## Migration and dependencies

- Donor disposition (`adopt`, `adapt`, `rewrite`, `retire`, or not applicable):
- Superseded path and removal evidence:
- Dependency/unsafe/native/compatibility review:

## Verification and evidence

<!-- Include exact commands, results, failures, and retained artifact links. -->

- Public-boundary behavior:
- Fault/cancellation/lifecycle:
- Performance/memory/I/O:
- Event/causal evidence:
- Exact-package/environment qualification, or explicitly not claimed:

## Checklist

- [ ] The owning issue and plan are linked.
- [ ] Owning contracts and responsible boundaries are identified.
- [ ] First-party behavior uses only its supported public contracts.
- [ ] Bounds, pressure, cancellation, failure, and shutdown are covered.
- [ ] Documentation, tests, and migration evidence changed together as needed.
- [ ] Performance claims link the real scenario and retained raw results.
- [ ] Support or qualification is not inferred from a source-tree test.
- [ ] Superseded active paths are removed after the conforming path is proved.
- [ ] `./scripts/ci.sh` passed, or failures are reported and scoped.
- [ ] `CHANGELOG.md` is updated when user-visible behavior changed.
