# Future — Beyond-0.16 Design

**Status:** Non-normative. Forward-looking design.

The numbered chapters (`01-`–`10-`) are the **0.16 release goal**: the
contracts 0.16 is built to satisfy, whether or not they are implemented
yet. This `future/` tier is the opposite axis — design that is **not a
0.16 goal**. It is recorded so the long-horizon direction is not lost,
but nothing here gates a 0.16 merge, and nothing here is wire-truth a
stranger must reimplement.

A `future/` document **graduates** into a numbered chapter when its
subject becomes a release goal. Until then it is rationale and
direction, not contract.

## Relationship to the other non-normative tiers

| Tier | Holds | Time axis |
|---|---|---|
| `heritage/` | proposals and phase records that *shaped* the architecture | past |
| `debt/` | known gaps and analysis against the *current* tree | present |
| `future/` | design direction past the 0.16 goal | future |

## Contents

| File | Subject |
|---|---|
| [`design-spine.md`](design-spine.md) | The design through-line: via-negativa version sequence beyond 0.16, kernel-as-mathematical-mechanism, the survival / corruption model (radiation, SEU, Mars-safe), the verification horizon, distributed = git, governance ported from Linux, project-level discipline. |
| [`multi-os-ports.md`](multi-os-ports.md) | Hosted ports to Windows / macOS / Solaris, and the DAG6 "lowest stable boundary per target" amendment they would force. Bare-metal (0.16) has no such tension; these ports are a separate, later track. |
| [`tty-pty.md`](tty-pty.md) | Future TTY/PTY direction: reserve `tty` for a real system-kernel terminal-device subsystem and use `root_shell` / `console_io` for the current #800 root-daemon shell proof. |

## Why the radiation / Mars material is here, not in a numbered chapter

The three-layer corruption model (Math / Body / World), CRC and
memory-scrubbing self-heal, single-event upsets, and the "Mars-safe"
north star are design *limits* — the harshest constraint the
architecture is steered to face, not a 0.16 deliverable. They belong on
the future axis: they justify present decisions without being present
goals. See [`design-spine.md` §Survival](design-spine.md#survival--the-corruption-model).
