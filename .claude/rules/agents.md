# Claude Agents & Skills

Custom agents and skills in `.claude/` provide specialized workflows for this project.

## Available Agents

| Agent | Model | Purpose |
|-------|-------|---------|
| `oracle` | Opus | Planning, architecture, complex decisions |
| `voyager` | Sonnet | Codebase exploration, dependency tracing |
| `mission-control` | Haiku | Plan compliance, documentation review |
| `telemetry` | Haiku | Test coverage & quality review |
| `flight-director` | Sonnet | Unix philosophy & code quality review |

**Agent Modes (review agents):**
- **countdown**: T-minus checks before launch (validate plan)
- **orbit**: In-flight monitoring (Claude uses, asks user when blocked)
- **reentry**: Final Approach review (Go Poll for landing)
- **ground-ops**: General ground support (design help, strategy advice)

**A+ Skip Rule:** If an agent gives A+ in round N, skip that agent in round N+1.

**Deferral Policy:** Deferrals are OK if tracking issue exists (e.g., #248). No grade penalty for properly documented deferrals.

## Available Skills

| Skill | Command | Purpose |
|-------|---------|---------|
| `countdown` | `/countdown [issue] [plan]` | Pre-implementation validation with oracle (Opus) + Go Poll |
| `final-approach` | `/final-approach [issue] [plan]` | Landing sequence with Go Poll, landing doc, and commit script |
| `stop` | `/stop [issue]` | Clean session handoff with flight log update and commit script |
| `rebase` | `/rebase [target]` | Rebase current branch onto specified target |

## Agent Files

```
.claude/
├── agents/
│   ├── oracle.md                # Planning, architecture - Opus (purple)
│   ├── voyager.md               # Codebase exploration - Sonnet (red)
│   ├── mission-control.md       # Plan compliance review - Haiku (blue)
│   ├── telemetry.md             # Test coverage review - Haiku (green)
│   └── flight-director.md       # Code quality review - Sonnet (yellow)
├── hooks/
│   └── stop-check.sh            # Stop hook - blocks stop with uncommitted work
├── rules/                       # Modular project rules
│   ├── agents.md                # This file
│   ├── architecture.md          # Workspace tree, crate names
│   ├── debugging.md             # Debugging guides (path-scoped)
│   ├── git-workflow.md          # Deferral, issues, commit proposals
│   ├── philosophy.md            # Design philosophy, bug fixing
│   ├── session-workflow.md      # Plans, flight logs, stop protocol
│   └── testing.md               # Integration test infra (path-scoped)
├── settings.json                # Project-level settings (hooks config)
└── skills/
    ├── countdown/SKILL.md       # /countdown command
    ├── final-approach/SKILL.md  # /final-approach command
    ├── rebase/SKILL.md          # /rebase command
    └── stop/SKILL.md            # /stop command
```

## Output Structure

```
tmp/{ISSUE}/
├── {P}/                         # Plan number (01, 02, ...)
│   ├── countdown/               # Plan review (before coding)
│   │   └── round-{N}/
│   │       ├── mission-control.md
│   │       ├── telemetry.md
│   │       └── flight-director.md
│   └── landing/                 # Landing review (before merge)
│       └── round-{N}/
│           ├── mission-control.md
│           ├── telemetry.md
│           └── flight-director.md
├── landing.md                   # Issue-level landing document
└── audit/                       # Phase-by-phase verification (optional)
    └── voyager-phase{N}.md

tmp/commit.sh                    # Executable commit script
```
