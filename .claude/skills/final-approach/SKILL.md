---
name: final-approach
description: "Final Approach - the landing sequence. Runs Go Poll with review agents, creates landing document, and prepares commit script."
---

# Final Approach

The landing sequence for completing a mission. Runs the Go Poll to get status from all stations.

## Usage

```
/final-approach [issue-number] [plan-number]
```

- `issue-number` - GitHub issue (required, inferred from context if possible)
- `plan-number` - Plan sequence number (default: `01`)

## What This Does

Final Approach is the end-of-mission workflow:

1. **Check** - Run `./scripts/check.sh`
2. **Go Poll** - Review agents report status in `reentry` mode
3. **Landing Doc** - Create summary document at `tmp/{ISSUE}/landing.md`
4. **CHANGELOG** - Update CHANGELOG.md with correct phase/version
5. **Commit Script** - Generate `tmp/commit.sh` for user

## Go Poll

Review agents report status for landing:

| Agent | Focus | Call |
|-------|-------|------|
| **mission-control** | Plan compliance, docs | *"Mission Control, go."* |
| **telemetry** | Test coverage, quality | *"Telemetry, go."* |
| **flight-director** | Code quality, Unix philosophy | *"Flight Director, go."* |

Each agent uses its configured default model (see agent frontmatter).

## A+ Skip Rule

**If an agent gives A+ in round N, skip that agent in round N+1.**

Example:
```
Round 1: Run all 3 agents
  - mission-control: A+  <- Will be skipped
  - telemetry: B
  - flight-director: A

Round 2: Skip mission-control, run telemetry + flight-director
  - telemetry: A
  - flight-director: A+  <- Will be skipped

Round 3: Skip mission-control + flight-director, run only telemetry
  - telemetry: A+

Done: All agents at A or A+
```

## Output Structure

```
tmp/{ISSUE}/{P}/
└── landing/
    └── round-{N}/
        ├── mission-control.md
        ├── telemetry.md
        └── flight-director.md

tmp/{ISSUE}/landing.md         # Issue-level landing document
tmp/commit.sh                  # Executable commit script
```

Where `{P}` is the plan number (e.g., `01`, `02`) matching plan files.

## Grading Scale

| Grade | Meaning |
|-------|---------|
| **A+** | Exceeds all standards - exemplary (skip next round) |
| **A** | Perfect compliance - go for landing |
| **B** | Minor issues - quick fixes needed |
| **C** | Significant gaps - more work required |
| **F** | Major problems - no-go |

## Success Criteria

**Must achieve GO (A or A+) from ALL THREE agents** before landing.

- **A+** = Exceeds standards, skip this agent in next round
- **A** = Go for landing, GO
- **B or below** = NO-GO, fix issues

If any agent gives below A:
1. Fix all identified issues
2. Run `./scripts/check.sh`
3. Re-run `/final-approach`
4. Repeat until all agents report GO (A or A+)

## Instructions

When invoked, you MUST follow these steps in order:

### Step 1: Setup

1. Determine the issue number from context (git branch, plan file, or ask user)
2. Determine the plan number (default `01`, or from user input)
3. Run `./scripts/check.sh` - ALL checks must pass before proceeding
   - If checks fail, fix issues and re-run before continuing
   - Check report available at `tmp/check-report.md`
   - Individual logs at `tmp/check-logs/`

### Step 2: Go Poll

1. Determine the round number (start at 1, increment for re-reviews)
2. Check previous round grades - skip agents with A+ from prior rounds
3. Create the output directory: `tmp/{ISSUE}/{P}/landing/round-{N}/`
4. Launch review agents IN PARALLEL using the Agent tool
5. After all agents complete, summarize the grades in a table
6. If any grade is below A, list blocking issues and STOP - fix issues, then re-run

### Step 3: Landing Document

Create `tmp/{ISSUE}/landing.md` with three sections:

**Section 1: Go Poll Summary**
- Final grades table (all three agents) with links to round reports
- Files changed (new/modified with LOC)
- Highlights from each agent

**Section 2: Verification Checklist**
- `./scripts/check.sh` passed (see `tmp/check-report.md`)
- CHANGELOG.md updated
- All agents reported GO

**Section 3: Commit**
- Proposed commit message
- Files to stage
- Git commands for user

### Step 4: CHANGELOG

Update `CHANGELOG.md` under the `[Unreleased]` section with correct phase number (match issue title).

### Step 5: Commit Script

Generate `tmp/commit.sh` - an executable shell script containing:
- `git add` for all relevant files (list explicitly, no `git add -A`)
- `git commit` with the proposed message from the landing document
- NO `git push` (user decides when to push)

Make it executable: `chmod +x tmp/commit.sh`

### Agent Prompts

**Agent 1:** `subagent_type: "mission-control"`
```
Go Poll for issue #{ISSUE}, plan {P} - Final Approach (Round {N}).

You are Mission Control reporting status for landing.

Focus: Plan compliance and documentation quality.

1. Find the plan files (check ~/.claude/plans/reovim/{ISSUE}/ or tmp/)
2. Verify all planned phases are implemented
3. Audit documentation quality (module docs, API docs, guides)
4. Check CHANGELOG.md for completeness

Write your report to: tmp/{ISSUE}/{P}/landing/round-{N}/mission-control.md

End with: "Mission Control: GO / NO-GO" and grade (A+/A/B/C/F).
```

**Agent 2:** `subagent_type: "telemetry"`
```
Go Poll for issue #{ISSUE}, plan {P} - Final Approach (Round {N}).

You are Telemetry reporting status for landing.

Focus: Test coverage and test quality.

1. Inventory all tests (unit and integration)
2. Analyze coverage of happy paths, error paths, edge cases
3. Detect test anti-patterns (flaky, slow, giant tests)
4. Identify missing critical test cases

Write your report to: tmp/{ISSUE}/{P}/landing/round-{N}/telemetry.md

End with: "Telemetry: GO / NO-GO" and grade (A+/A/B/C/F).
```

**Agent 3:** `subagent_type: "flight-director"`
```
Go Poll for issue #{ISSUE}, plan {P} - Final Approach (Round {N}).

You are Flight Director reporting status for landing.

Focus: Code quality and Unix philosophy compliance.

1. Verify Unix philosophy: mechanism vs policy, single responsibility, composability
2. Check layer boundaries (kernel -> drivers -> modules)
3. Audit unsafe code for SAFETY documentation
4. Run cargo clippy and cargo doc to verify zero warnings
5. Assess complexity and anti-patterns

Write your report to: tmp/{ISSUE}/{P}/landing/round-{N}/flight-director.md

End with: "Flight Director: GO / NO-GO" and grade (A+/A/B/C/F).
```

CRITICAL:
- Launch review agents with `subagent_type` set to agent name
- Each agent uses its default model from frontmatter
- Skip agents that received A+ in previous rounds
- Do NOT skip Steps 3-5 - landing document, CHANGELOG, and commit script are mandatory
