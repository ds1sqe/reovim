# Reovim repository rules

## Direction and authority

Reovim is a reliable, high-performance, extensible editing system. Text is one
domain. v0.16 is re-foundation; v0.17 builds Neovim parity on its contracts.

Read Documentation/README.md, Documentation/architecture.md and the owning
interface before changing a boundary. Documentation owns goals/contracts;
plans own execution order. Target prose is not implementation evidence.
Keep each active specification at most 250 lines, explain each rule once,
and use direct file links.

Use tmp/current.md for Mission, Progress and Focus. Progress contains Current,
Next and Completed. After a successful commit, move only fully covered
Completed tasks to the active plan directory's completed.md with commit/date,
result and verification. Leave uncommitted tasks in current.md.
Do not resume archived plans. Keep tmp/README.md synchronized with disk.
Keep context light and focused on the active task. Runtime notes and inventory
stay concise; link durable decisions and history instead of repeating them.
Retain perf/ and Documentation/heritage/ as reference material; do not delete
them during documentation or tooling cleanup.

## Architecture

- Dependencies point inward: Runtime -> Core -> Interface -> Foundation.
  Assembly alone selects concrete components; no hidden globals or cycles.
- Kernel owns neutral mechanisms and authoritative Buffer/History state, not
  concrete domains, resource/Host adapters, transports, clients or renderers.
- No text strings/coordinates, file paths, terminal cells, viewport or drawing
  types shape the neutral core. Address, unit, revision and capability are
  explicit; byte offsets and whole-resource copies are not universal.
- Resource adapters connect data; Domains supply meaning; Representations
  interpret/lower; Projections synchronize; Client views present.
- One Buffer is transactional state over one Resource. Clients/extensions
  submit public semantic edits, never mutate storage or private state.
- Complete the used interface chain and compile public providers/consumers
  before inner mechanisms. Then execute that journey, not another type catalog.
- Foundation is core-only; portable Kernel/applicable Rust Client logic is
  no_std + alloc. Hosted adapters/assembly may use std.
- Host contracts apply to Kernel, Client and Resource consumers through
  separate grants. Keep concrete environment types inside adapters.
- A new Host capability needs an immediate consumer, typed bounds/errors,
  standard/test adapters, shared tests, cancellation/shutdown and measurements.
- No I/O, waiting, callbacks or fallible work after the atomic commit cut
  under Buffer authority. Revalidate revisions/generations before installation.

## Public APIs and dependencies

Rust SDK is the trusted in-process surface. Python/TS extensions use versioned
language-neutral service contracts; no embedded interpreter in the Kernel.
Every client owns its Core, Contract, Runtime and Module roles. Rendering is
explicit capability, never inferred solely from a client family name.

Internal owner-tree APIs may change coherently. Public v0.16 APIs remain
Experimental; stable compatibility starts at an admitted full SDK release.
Public examples and first-party extensions use public APIs, not private types.
Require grants, bounds, pressure, cancellation, lifecycle, compatibility tests
and honest containment limits.

Review third-party needs, maintenance, license, transitive/unsafe cost, targets,
runtime behavior and overhead. Abstract dependencies for owned semantics,
deterministic substitution or an unsafe/volatile boundary, not just naming.

## Verification and tooling

Use scripts/check.sh during work and scripts/ci.sh before a checkpoint.
ci.json owns the exact stable/nightly version window. Local and GitHub CI
use the same runner, which installs those pins and invokes check.sh --full.
Window changes require the full matrix before committing; normal test runs
must not mutate the manifest. Do not restore the old Rust 1.92 test lane.
The retained gate uses ordinary Cargo and style/link checks.
The old exact conformance registry, source classifier and compiler/package
proof requirements are retired, not silently satisfied by these checks.
Review dependency direction, public types and portable closures explicitly.

Tests compare semantic values, keys, sets, relations and outcomes. Never use
literal content hashes, serialized lengths or collection counts as proxies
for behavior or repository shape. Published protocol/hash vectors and named
capacity/safety/accounting bounds may use exact values.

Git supplies committed per-file blob identities. Ignored local coverage
records successful inputs. Do not hand-maintain a tracked hash manifest or
edit generated digest/timestamp fields. Select changed file owners and reverse
dependents; unknown ownership/configuration or invalid cache runs all checks.
Reuse file-local results only for file-local checks; Rust results require the
complete component input closure and toolchain. Cached passes do not replace
uncached checkpoint/release verification.

Before tool coding, record its immediate consumer/block, smallest path,
invocation/failures, effort budget and stop condition in the active plan.
At most one preparatory tool-only slice may precede real consumption.
Stop at the cleared block or budget/review point; renaming does not reset it.
Retain no unused framework. Target about one minute for the warm ordinary
check and measure it; do not claim that target without data.

## Style, performance and migration

Follow CODE_STYLE.md: 80 columns and at most three Rust block-nesting levels.
Let rustfmt own whitespace. Use small functions, shallow flow, explicit
ownership and comments about why.

Measure real scenarios from input admission through presentation, primarily
p99. Include memory, large/sparse resources, IPC/transports, extensions, locks,
logging and tracing overhead. Retain raw data and workload/environment.
Use the single structured diagnostic stream with bounded 1 MiB ring, canonical
output and visible loss. Keep causal graph/snapshots separately bounded.
Development deadlocks fail loudly with bounded independent capture; elapsed
time alone does not prove deadlock. See Documentation/debugging.md.

Migrate behavior individually as adopt/adapt/rewrite/retire. Preserve useful
fixtures, remove invalid coupling, and prove a real composed public path.
Include relevant fault/cancellation, lifecycle, event and performance evidence.
Remove superseded paths after verification; no permanent unowned shims.
Do not discard unrelated or uncommitted user work.
