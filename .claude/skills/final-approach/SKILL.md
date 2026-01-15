---
name: final-approach
description: "Final Approach - the landing sequence. Runs Go Poll with all three agents (mission-control, telemetry, flight-director) in reentry mode. Use before landing any significant implementation."
---

# Final Approach

The landing sequence for completing a mission. Runs the Go Poll to get status from all stations.

## Usage

```
/final-approach [issue-number]
```

## What This Does

Final Approach is the end-of-mission workflow:

1. **Sync** - Fetch and rebase with upstream
2. **Check** - Run `./scripts/check.sh`
3. **Go Poll** - All three agents report status in `reentry` mode
4. **Landing Doc** - Create summary document
5. **Commit** - Ready for merge

## Go Poll

The Go Poll launches all three agents in parallel:

| Agent | Focus | Call |
|-------|-------|------|
| **mission-control** | Plan compliance, docs | *"Mission Control, go."* |
| **telemetry** | Test coverage, quality | *"Telemetry, go."* |
| **flight-director** | Code quality, Unix philosophy | *"Flight Director, go."* |

## Output Structure

```
tmp/{ISSUE}/
└── landing/
    └── round-{N}/
        ├── mission-control.md
        ├── telemetry.md
        └── flight-director.md
```

## Grading Scale

| Grade | Meaning |
|-------|---------|
| **A+** | Exceeds all standards - exemplary |
| **A** | Perfect compliance - go for landing |
| **B** | Minor issues - quick fixes needed |
| **C** | Significant gaps - more work required |
| **F** | Major problems - no-go |

## Success Criteria

**Must achieve A or A+ from ALL THREE agents** before landing.

If any agent gives below A:
1. Fix all identified issues
2. Run `./scripts/check.sh`
3. Re-run `/final-approach`
4. Repeat until all agents report "go"

## Instructions

When invoked, you MUST:

1. Determine the issue number from context (git branch, plan file, or ask user)
2. Determine the round number (start at 1, increment for re-reviews)
3. Create the output directory: `tmp/{ISSUE}/landing/round-{N}/`
4. Launch ALL THREE agents IN PARALLEL using a single message with multiple Task tool calls
5. After all agents complete, summarize the grades in a table
6. If any grade is below A, list the blocking issues that must be fixed

Example agent prompts:

**Agent 1 (mission-control):**
```
Go Poll for issue #{ISSUE} - Final Approach (Round {N}).

You are Mission Control reporting status for landing.

Focus: Plan compliance and documentation quality.

1. Find the plan file (check ~/.claude/plans/ or tmp/)
2. Verify all planned phases are implemented
3. Audit documentation quality (module docs, API docs, guides)
4. Check CHANGELOG.md for completeness

Write your report to: tmp/{ISSUE}/landing/round-{N}/mission-control.md

End with: "Mission Control: GO / NO-GO" and grade (A+/A/B/C/F).
```

**Agent 2 (telemetry):**
```
Go Poll for issue #{ISSUE} - Final Approach (Round {N}).

You are Telemetry reporting status for landing.

Focus: Test coverage and test quality.

1. Inventory all tests (unit and integration)
2. Analyze coverage of happy paths, error paths, edge cases
3. Detect test anti-patterns (flaky, slow, giant tests)
4. Identify missing critical test cases

Write your report to: tmp/{ISSUE}/landing/round-{N}/telemetry.md

End with: "Telemetry: GO / NO-GO" and grade (A+/A/B/C/F).
```

**Agent 3 (flight-director):**
```
Go Poll for issue #{ISSUE} - Final Approach (Round {N}).

You are Flight Director reporting status for landing.

Focus: Code quality and Unix philosophy compliance.

1. Verify Unix philosophy: mechanism vs policy, single responsibility, composability
2. Check layer boundaries (kernel → drivers → modules)
3. Audit unsafe code for SAFETY documentation
4. Run cargo clippy and cargo doc to verify zero warnings
5. Assess complexity and anti-patterns

Write your report to: tmp/{ISSUE}/landing/round-{N}/flight-director.md

End with: "Flight Director: GO / NO-GO" and grade (A+/A/B/C/F).
```

CRITICAL: Launch all three agents using the Task tool with `subagent_type` set to the corresponding agent name (mission-control, telemetry, flight-director).
