---
name: triple-review
description: "Launch the Request-for-Landing triple review with three parallel agents: requirements/docs, tests, and architecture/Unix philosophy. Use this skill before landing any significant implementation."
---

# Triple Review - Request-for-Landing Workflow

Launch a comprehensive code review using three specialized agents in parallel.

## Usage

```
/triple-review [issue-number]
```

## What This Does

This skill launches **three elite review agents in parallel** to ensure code quality before landing:

| Agent | Focus | Grade Criteria |
|-------|-------|----------------|
| **review-requirements** | Plan compliance, documentation quality | All planned items implemented, docs complete |
| **review-tests** | Test coverage, test quality | Critical paths covered, no anti-patterns |
| **review-architecture** | Unix philosophy, code quality | Mechanism vs policy, simplicity, safety |

## Execution

When you invoke this skill, Claude will:

1. **Launch all three agents in parallel** using the Task tool
2. **Each agent writes a review report** to `tmp/{ISSUE_NUMBER}-review-round{N}-{type}.md`
3. **Aggregate results** and report grades from all agents

## Grading Scale

| Grade | Meaning |
|-------|---------|
| **A+** | Exceeds all standards - exemplary |
| **A** | Perfect compliance - ready to land |
| **B** | Minor issues - quick fixes needed |
| **C** | Significant gaps - more work required |
| **F** | Major problems - blocking issues |

## Success Criteria

**Must achieve A or A+ from ALL THREE agents** before landing.

If any agent gives below A:
1. Fix all identified issues
2. Run `./scripts/check.sh`
3. Re-run `/triple-review`
4. Repeat until all agents give A or A+

## Instructions

When invoked, you MUST:

1. Determine the issue number from context (git branch, plan file, or ask user)
2. Determine the review round number (start at 1, increment for re-reviews)
3. Launch ALL THREE review agents IN PARALLEL using a single message with multiple Task tool calls
4. Each agent should write its review to `tmp/{ISSUE}-review-round{ROUND}-{requirements|tests|architecture}.md`
5. After all agents complete, summarize the grades in a table
6. If any grade is below A, list the blocking issues that must be fixed

Example agent prompts:

**Agent 1 (review-requirements):**
```
Review issue #{ISSUE} for Request-for-Landing (Round {N}).

Focus: Plan compliance and documentation quality.

1. Find the plan file (check ~/.claude/plans/ or tmp/)
2. Verify all planned phases are implemented
3. Audit documentation quality (module docs, API docs, guides)
4. Check CHANGELOG.md for completeness

Write your detailed review to: tmp/{ISSUE}-review-round{N}-requirements.md

End with a grade (A+/A/B/C/F) and verdict (APPROVED/CHANGES REQUIRED/BLOCKED).
```

**Agent 2 (review-tests):**
```
Review issue #{ISSUE} for Request-for-Landing (Round {N}).

Focus: Test coverage and test quality.

1. Inventory all tests (unit and integration)
2. Analyze coverage of happy paths, error paths, edge cases
3. Detect test anti-patterns (flaky, slow, giant tests)
4. Identify missing critical test cases

Write your detailed review to: tmp/{ISSUE}-review-round{N}-tests.md

End with a grade (A+/A/B/C/F) and verdict (APPROVED/CHANGES REQUIRED/BLOCKED).
```

**Agent 3 (review-architecture):**
```
Review issue #{ISSUE} for Request-for-Landing (Round {N}).

Focus: Code quality and Unix philosophy compliance.

1. Verify Unix philosophy: mechanism vs policy, single responsibility, composability
2. Check layer boundaries (kernel → drivers → modules)
3. Audit unsafe code for SAFETY documentation
4. Run cargo clippy and cargo doc to verify zero warnings
5. Assess complexity and anti-patterns

Write your detailed review to: tmp/{ISSUE}-review-round{N}-architecture.md

End with a grade (A+/A/B/C/F) and verdict (APPROVED/CHANGES REQUIRED/BLOCKED).
```

CRITICAL: Launch all three agents using the Task tool with `subagent_type` set to the corresponding agent name (review-requirements, review-tests, review-architecture).
