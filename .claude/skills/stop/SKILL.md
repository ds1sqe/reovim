---
name: stop
description: "Stop Protocol - clean session handoff. Updates flight log, creates commit script, and documents exactly where to resume. Use before ending any work session."
---

# Stop Protocol

Clean handoff when ending a work session. Ensures the next flight can pick up exactly where you left off.

## Usage

```
/stop [issue-number]
```

## When to Use

- Before ending a session for ANY reason
- When context is getting large and compaction is imminent
- When blocked and waiting for user input
- When the user says to stop

**You should NEVER end a session with uncommitted work without running this protocol.**

## What This Does

1. **Flight Log** - Append entry to `~/.claude/plans/reovim/{ISSUE}/flight-log.md`
2. **Commit Script** - Generate `tmp/commit.sh` if there are uncommitted changes
3. **Status Report** - Tell the user exactly what's done and what's left

## Instructions

When invoked, you MUST:

### Step 1: Assess Current State

1. Determine the issue number from context (git branch, user input, or ask)
2. Run `git status` to check for uncommitted changes
3. Run `git diff --stat` to see what's changed
4. Identify the current plan and phase you were working on

### Step 2: Update Flight Log

Append a new flight entry to `~/.claude/plans/reovim/{ISSUE}/flight-log.md`:

```markdown
## Flight {N} - {date}

### Work Done
- [List completed items with specific file paths]

### Decisions
- [Any architectural or design decisions made this session]

### Commits
- [List any commits made, or "No commits yet"]

### Remaining (current phase)
- [Specific items NOT yet done in the current phase]
- [Include file paths and line numbers where applicable]

### Next
- [Exact instructions for next session to resume]
- [File path and line number where work stopped]
- [Watch out for: any gotchas or blockers discovered]
```

Also update the **Current Status** header at the top of the flight log.

### Step 3: Commit Script (if uncommitted changes exist)

If `git status` shows changes:

1. Generate `tmp/commit.sh` with:
   - `git add` for all relevant files (explicit list, no `git add -A`)
   - `git commit` with an appropriate message
   - NO `git push`
2. Make executable: `chmod +x tmp/commit.sh`
3. Tell the user: "Review and run `tmp/commit.sh` to commit current work"

If no changes: skip this step.

### Step 4: Status Report

Print a summary to the user:

```
STOP PROTOCOL COMPLETE

Issue:     #{ISSUE}
Plan:      {P}-{subject}.md
Phase:     {current phase} ({done}/{total} items)
Flight:    {N}

Done this session:
  - [key accomplishments]

Not yet done:
  - [remaining items]

Resume point:
  {file}:{line} - {what to do next}

Flight log: ~/.claude/plans/reovim/{ISSUE}/flight-log.md
Commit script: tmp/commit.sh (if applicable)
```

CRITICAL:
- ALWAYS update the flight log - this is the lifeline for the next session
- Be SPECIFIC about resume points - file paths, line numbers, function names
- The next Claude session reads ONLY the flight log to understand context
- If no plan directory exists yet, create it before writing the flight log
