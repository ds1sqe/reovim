# 10.1 — Testing Requirements

**Scope.** The development-process testing rules: what every change
to a spec-conformant implementation must ship with — coverage,
E2E-level smoke tests, and golden assertions for visual output and
inner state.

**Heritage.** Linux `Documentation/process/`; the v0.15 coverage
policy and `/e2e` harness (archived).

**Locked rules.** `DEV1..DEV5`.

---

## 1. Relationship to conformance (9.1)

The CF rules (9.1) bind the *spec* to the implementation: every
locked rule has a fixture, every ABI shape has a golden. The DEV
rules bind *changes* to tests: a feature can satisfy every CF row
it touches and still be unshippable under this chapter, because CF
proves rule conformance while DEV proves the feature itself works
end to end.

Both gates apply. Neither substitutes for the other.

## 2. Coverage

> **DEV1 — Tests land in the same change; coverage never
> regresses.** Every change ships its tests in the same change —
> not before (strict TDD is not required), not after (retroactive
> tests are forbidden). CI enforces **100% MC/DC coverage (line +
> condition) workspace-wide. There is no escape hatch and no
> "OK to 99.999%"**: no crate-level exemption, no `coverage(off)`
> annotation. Code whose branches cannot be exercised is
> restructured until they can be (the North Star: dev-conv is
> never a valid justification). If a toolchain defect misattributes
> coverage on a code shape, the code shape changes — the gate does
> not. A change that regresses coverage does not merge.
> *Class*: CI.

If the test cannot be written in the same change, the change is not
understood well enough to merge yet.

**Physical measurement limits (DEV1 clarification).** "100%" means
100% of regions whose counters can physically persist. Two shapes are
outside any instrumentation's reach, restructure or not:

1. **Terminal syscalls** — the body of a never-returning exit syscall
   (`exit_group`): the coverage profile is flushed *before* the final
   exit, so that line's own counter increment can never reach disk.
2. **The profiler's self-measurement horizon** — counter increments
   that occur while the counter section is being serialized count
   themselves after the snapshot.

Such lines are not exempted: each one is individually classified and
justified in the issue's coverage ledger under `Documentation/debt/`
(`coverage-<issue>-<subject>.md`), reviewed before merge, and carries
no annotation (`coverage(off)` remains forbidden). A line
claimed under this clause that a reviewer can show is coverable — by a
mid-run invocation, a fixture exec, or a restructure — fails the gate.

## 3. E2E smoke

> **DEV2 — Every feature passes an E2E-level smoke test.** A
> feature change MUST include at least one smoke test that
> exercises the feature through the real composed system — the
> launcher boots the real server and client (embedded or
> subprocess, 1.3), input is driven through the debug drive surface
> (`cli drive`, DS7), and the result is asserted through the debug
> read surface (7.2 §5). Unit and integration fixtures are
> necessary but never sufficient: a feature no E2E path can reach
> is not done.
> *Class*: CI.

The smoke test is deliberately shallow — one happy path through the
real binaries. Depth belongs to unit tests and CF fixtures; the
smoke test's job is proving the layers actually compose.

## 4. Golden assertions

Smoke tests assert against committed expected artefacts —
**goldens** — not against ad-hoc predicates. Which golden depends
on what the feature touches:

| Feature touches | Golden artefact | Captured via |
|---|---|---|
| rendered output (cells, chrome, layout) | expected screen capture (`FrameBlob`) | `hostapi_debug_capture_frame` (7.2 §5; client side 8.4) |
| buffer bytes | expected byte dump | `hostapi_debug_buffer_bytes_read` (7.2 §5) |
| subsystem / tree state | expected `DomainTreeSnapshot` | DS10 buffer description |
| emitted events | expected DS12 event sequence | `hostapi_debug_event_subscribe` (7.2 §5) |

> **DEV3 — Visual changes assert against an expected screen
> capture.** A feature that affects rendered output MUST commit an
> expected frame capture and assert the live capture against it
> byte-wise. "Looks right" is not an assertion; the golden frame
> is.
> *Class*: CI.

> **DEV4 — Inner-data changes assert against an expected state
> snapshot.** A feature that affects inner data — buffer bytes,
> editor subsystem state, the domain tree — MUST commit the
> expected snapshot (byte dump, `DomainTreeSnapshot`, or event
> sequence per the table above) and assert the live read against
> it. Asserting only the rendered screen is insufficient: the
> screen can be right while the bytes are wrong.
> *Class*: CI.

A feature that touches both (most editing features) ships both
goldens — the frame proves what the user sees; the snapshot proves
what the editor holds.

These rules apply to ext cdylibs (modules, drivers, capabilities)
exactly as to core: a plugin feature is a feature.

### 4.1 Reproducibility

> **DEV5 — Goldens are captured in a reproducible environment.**
> A golden test pins everything the capture can observe:
>
> - a committed fixture environment — fixture filesystem tree,
>   fixture buffers — under the test's own directory, never the
>   developer's real filesystem, `$HOME`, or the repo checkout
>   itself,
> - fixed window and cell dimensions,
> - fixed config (the layer stack resolves to the test's committed
>   config only, 1.5),
> - deterministic ordering (directory enumeration sorted or
>   fixture-controlled; lockfile-ordered load per DT17),
> - no environment leakage: wall-clock times, hostnames, usernames,
>   and absolute paths MUST NOT appear in the captured artefact —
>   render relative forms or mask the field.
>
> A golden that does not reproduce byte-identically on a second
> run, on a different machine, is a broken test — not tolerable
> flakiness.
> *Class*: CI.

Worked example — an explorer (filetree) module: the smoke test
boots the real launcher with the fixture tree as its root, opens
the explorer window at fixed dimensions, and captures the frame.
The golden shows the rendered filetree — entry names, nesting,
expand markers — from the committed fixture, so it renders
byte-identically anywhere. A second assertion covers the inner data
per DEV4 (e.g. the expanded-state slots via the DS10 handler-query
path), so a frame that happens to look right cannot hide a wrong
tree state underneath.

## 5. Blessing goldens

A golden changes only when the *intended* behaviour changes, and
the re-blessing is part of the change that altered the behaviour —
reviewed as a behaviour change, never regenerated to silence a red
test. CF4's post-v1.0 permanence applies to ABI goldens; DEV
goldens (frames, snapshots) follow normal review, since features
may legitimately evolve their output pre- and post-1.0.

Goldens are versioned in-repo next to the test that asserts them.
Platform-dependent frames (cell sizes, font metrics) are captured
at the cell layer, not the pixel layer, so one golden serves all
platforms.

## Open items

1. Golden storage format for frames — raw `FrameBlob` bytes vs a
   stable text rendering (diff-friendly). Draft: text rendering,
   one cell per character, attributes as suffix runs.
2. Whether DEV2's smoke tests run per-PR or per-merge. Draft:
   per-PR; the composed boot must stay fast enough to afford it.

## Conformance

| Rule | Fixture |
|---|---|
| DEV1 | CI gate: coverage report below 100% MC/DC (line or condition) → merge blocked, any crate; any `coverage(off)` annotation present → CI fails. |
| DEV2 | CI presence check: a change tagged `feat` without an E2E smoke test referencing the real launcher boot → merge blocked. |
| DEV3 | Fixture feature altering rendered output without a frame golden → CI fails; with a stale golden → byte-diff failure names the golden path. |
| DEV4 | Fixture edit altering buffer bytes asserted only via frame golden → CI fails (snapshot golden required); snapshot mismatch names offset and expected/actual bytes. |
| DEV5 | Same golden test run twice on two hosts → byte-identical captures; a test rendering an absolute path into the frame → CI fails the leakage check. |
