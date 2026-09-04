# Repository Entrypoint Reviews

This directory retains the focused human review behind each exact admission in
[`../entrypoint-reviews.json`](../entrypoint-reviews.json). The registry, not
this index, is the machine-readable authority for entrypoint identity, direct
local invocation edges, assumptions, disposition, and record digest.

Each script or workflow admission is bound to its exact content digest. A Cargo
target may appear only as a terminal direct-invocation reference and is
resolved against the workspace topology and its separate Cargo/source
evidence. A changed entrypoint, direct edge, assumption, mechanism, or review
record requires renewed review.

After reviewing the changed entrypoint and updating its retained review prose,
run the bounded refresh command from the repository root:

```sh
cargo run --locked --release -p reovim-conformance -- \
  refresh-entrypoint-review --root /absolute/path/to/reovim \
  --path scripts/check-documentation.sh
```

Select the changed registered path. The command derives the two registries and
focused review, then updates their linked digests and timestamp. Do not edit
those generated JSON fields manually. Review the resulting diff before running
the exact conformance checks.

Current records:

- [`check-sh.md`](check-sh.md)
- [`check-cache-sh.md`](check-cache-sh.md)
- [`check-style-sh.md`](check-style-sh.md)
- [`check-documentation-sh.md`](check-documentation-sh.md)
- [`check-conformance-sh.md`](check-conformance-sh.md)
- [`ci-workflow.md`](ci-workflow.md)

These reviews constrain the admitted current bytes. They do not claim to parse
or prove arbitrary Bash or YAML semantics.
