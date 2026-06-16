# `system/` — the future World system-kernel root

The settled tree carries three kernel roots: `editor/` (the Math editor
kernel), `client/` (the Math client kernel), and `system/` (the future World
system kernel — KERNEL 1). The first two host real crates today; `system/` is
the top-level placeholder for the third.

## Placeholder (no crate yet — rule of three)

`system/` has **no crate and no workspace member** this flight. The only kernel
consumer today is the editor; no concrete system-kernel consumer exists yet
(rule of three: the seam shape is best learned from the first real consumer,
not guessed). `system/` becomes a real crate root when that consumer arrives;
until then it is this documented placeholder.

The depgraph `system/*` → Foundation category row already exists (SP01) but
stays inert until a real `system/` crate lands.
