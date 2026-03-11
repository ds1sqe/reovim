---
name: countdown
description: "Countdown - pre-implementation validation. Runs oracle (Opus) for planning, then Go Poll with review agents to validate before coding starts."
---

# Countdown

The pre-launch sequence for validating a plan before implementation begins.

## Usage

```
/countdown [issue-number] [plan-number]
```

- `issue-number` - GitHub issue (required, inferred from context if possible)
- `plan-number` - Plan sequence number (default: `01`)

## What This Does

Countdown is the pre-implementation workflow:

1. **Flight Log** - Check for multi-flight continuity
2. **Plan** - Oracle creates/refines implementation plan
3. **Go Poll** - Review agents validate the plan
4. **Iterate** - Fix issues until all agents report GO
5. **Launch** - Begin implementation only when validated

## Sequence

### Phase 0: Flight Log Check

For multi-flight issues, check `~/.claude/plans/reovim/{ISSUE}/flight-log.md`:
- If it exists, read it to understand prior work and current status
- If this is a continuation, skip to the appropriate step

### Phase 1: Oracle Planning

Oracle creates the implementation plan at:
```
~/.claude/plans/reovim/{ISSUE}/{P}-{subject}.md
```

### Phase 2: Go Poll

Review agents validate the plan:

| Agent | Focus |
|-------|-------|
| **mission-control** | Plan completeness, phases, acceptance criteria |
| **telemetry** | Test strategy, coverage targets |
| **flight-director** | Architecture, Unix philosophy, layer boundaries |

Each agent uses its configured default model (see agent frontmatter).

## A+ Skip Rule

**If an agent gives A+ in round N, skip that agent in round N+1.**

## Output Structure

```
tmp/{ISSUE}/{P}/
└── countdown/
    └── round-{N}/
        ├── mission-control.md
        ├── telemetry.md
        └── flight-director.md
```

Where `{P}` is the plan number (e.g., `01`, `02`) matching plan files.

## Grading Scale

| Grade | Meaning |
|-------|---------|
| **A+** | Exemplary plan (skip next round) |
| **A** | Ready for implementation |
| **B** | Minor gaps - refine plan |
| **C** | Significant gaps - more planning needed |
| **F** | Major issues - rethink approach |

## Success Criteria

**Must achieve GO (A or A+) from ALL THREE review agents** before implementation.

- **A+** = Exemplary, skip this agent in next round
- **A** = Ready for implementation, GO
- **B or below** = NO-GO, refine plan

If any agent gives below A:
1. Refine the plan based on feedback
2. Re-run `/countdown`
3. Repeat until all agents report GO (A or A+)

## Instructions

When invoked, you MUST:

1. Determine the issue number from context (git branch, user input, or ask)
2. Determine the plan number (default `01`, or from user input)
3. Check for flight log at `~/.claude/plans/reovim/{ISSUE}/flight-log.md`
   - If exists, read it to understand prior context and current status
4. Check if a plan exists at `~/.claude/plans/reovim/{ISSUE}/{P}-*.md`
5. If no plan exists or plan needs refinement:
   - Launch `oracle` agent to create/refine the plan
6. Determine the round number (start at 1, increment for re-reviews)
7. Check previous round grades - skip agents with A+ from prior rounds
8. Create the output directory: `tmp/{ISSUE}/{P}/countdown/round-{N}/`
9. Launch review agents IN PARALLEL using the Agent tool
10. After all agents complete, summarize the grades in a table
11. If any grade is below A, list what needs to be fixed in the plan

**Phase 1: Oracle (if needed)**

Launch with: `Agent tool, subagent_type: "oracle"`

```
Create/refine implementation plan for issue #{ISSUE}.

You are Oracle - the far-seeing architect.

1. Understand the requirements from the issue
2. Explore the codebase to understand current architecture
3. Design a phased implementation plan

Write the plan to: ~/.claude/plans/reovim/{ISSUE}/{P}-{subject}.md

Include:
- Summary and approach
- Phases with specific files and changes
- Risks and mitigations
- Test strategy
- Acceptance criteria
- Final Procedure section referencing Final Approach
- "Never stop until ALL phases are FULLY FINISHED."
```

**Phase 2: Review Agents**

Launch all three IN PARALLEL using the Agent tool:

**Agent 1:** `subagent_type: "mission-control"`
```
Countdown review for issue #{ISSUE}, plan {P} (Round {N}).

You are Mission Control validating the plan before launch.

Focus: Plan completeness and clarity.

1. Read the plan at ~/.claude/plans/reovim/{ISSUE}/{P}-*.md
2. Verify all phases have clear acceptance criteria
3. Check dependencies between phases
4. Ensure scope is well-bounded

Write your report to: tmp/{ISSUE}/{P}/countdown/round-{N}/mission-control.md

End with: "Mission Control: GO / NO-GO" and grade (A+/A/B/C/F).
```

**Agent 2:** `subagent_type: "telemetry"`
```
Countdown review for issue #{ISSUE}, plan {P} (Round {N}).

You are Telemetry validating the test strategy.

Focus: Test coverage planning.

1. Read the plan at ~/.claude/plans/reovim/{ISSUE}/{P}-*.md
2. Verify test strategy covers happy paths, errors, edge cases
3. Check that critical functionality has test targets
4. Identify any testing gaps

Write your report to: tmp/{ISSUE}/{P}/countdown/round-{N}/telemetry.md

End with: "Telemetry: GO / NO-GO" and grade (A+/A/B/C/F).
```

**Agent 3:** `subagent_type: "flight-director"`
```
Countdown review for issue #{ISSUE}, plan {P} (Round {N}).

You are Flight Director validating the architecture.

Focus: Unix philosophy and layer boundaries.

1. Read the plan at ~/.claude/plans/reovim/{ISSUE}/{P}-*.md
2. Verify mechanism vs policy separation
3. Check layer boundaries are respected
4. Assess complexity and simplicity

Write your report to: tmp/{ISSUE}/{P}/countdown/round-{N}/flight-director.md

End with: "Flight Director: GO / NO-GO" and grade (A+/A/B/C/F).
```

CRITICAL:
- Launch `oracle` with `subagent_type: "oracle"` (uses Opus by default)
- Launch review agents with `subagent_type` set to agent name
- Each agent uses its default model from frontmatter
- Skip agents that received A+ in previous rounds
- Do NOT start implementation until all agents report GO
