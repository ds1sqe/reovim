# Plan Files

**IMPORTANT**: Claude Code does NOT automatically enforce plan file naming. You MUST manually ensure plans follow this convention.

**Plan File Convention (directory per issue):**
```
~/.claude/plans/reovim/{ISSUE}/
├── flight-log.md              # Session handoff (append-only)
├── 01-{SUBJECT}.md            # First plan (unit of work)
├── 02-{SUBJECT}.md            # Second plan (different work)
└── ...
```

**Examples:**
- `567/01-buffer-api-redesign.md` - Issue #567, first plan
- `567/02-driver-integration.md` - Issue #567, second plan
- `489/01-documentation-drift.md` - Issue #489, single plan

**Naming Rules:**
- `{ISSUE}` - GitHub issue number (directory name)
- `01-`, `02-` - Sequence number (zero-padded, creation order)
- `{SUBJECT}` - Brief kebab-case description of the work
- One plan = one unit of work (NOT revisions — edit in place if approach changes)
- New file only for genuinely different work
- Even single-plan issues use directory structure

**Plan Requirements:**
- Plans must be **self-contained** (works after context compact/clear)
- Include all information needed for implementation:
  - Architecture diagrams, file locations, type definitions
  - Reference to related issues and dependencies
  - Acceptance criteria and test targets
- Add to every plan: **"Never stop until ALL phases are FULLY FINISHED."**
- Include a **Final Procedure** section referencing Final Approach

**IMPORTANT:**
Claude Code generates random plan filenames (e.g., `groovy-popping-kahn.md`) by default.
After creating a plan, **YOU MUST DO** verify and rename to follow the convention above.

**Workaround:**
1. When plan mode creates a file, note the random filename
2. After exiting plan mode, create issue directory and move:
   ```bash
   mkdir -p ~/.claude/plans/reovim/{ISSUE}
   mv ~/.claude/plans/{random-name}.md ~/.claude/plans/reovim/{ISSUE}/01-{SUBJECT}.md
   ```
3. Continue with countdown/implementation using the correctly named file

# Multi-Flight Workflow

Issues often span multiple Claude Code sessions (flights). Use flight logs to maintain continuity.

**Session Protocol:**
- **Start of flight**: Read `~/.claude/plans/reovim/{ISSUE}/flight-log.md` before doing anything
- **End of flight**: Append a flight entry before session ends

**Flight Log Format** (`flight-log.md`):

```markdown
# Flight Log — Issue #{ISSUE}: {TITLE}

## Current Status
- **Active Plan**: 02-driver-integration.md
- **Phase**: In Progress
- **Last Flight**: 3

---

## Flight 1 — {date}

### Work Done
- Completed plan 01-buffer-api-redesign.md
- Implemented BufferApi trait in server/lib/kernel/src/api/buffer.rs

### Decisions
- Chose trait-based dispatch over enum dispatch (compile-time safety)

### Commits
- abc1234 feat(kernel): add BufferApi trait (#567)

---

## Flight 2 — {date}

### Work Done
- Started plan 02-driver-integration.md
- Completed phases 1-2, phase 3 in progress
- Stopped at server/lib/drivers/buffer/src/lib.rs:142

### Blockers
- Found that DriverX needs a new kernel API — created plan 03

### Commits
- def5678 feat(driver): buffer driver scaffold (#567)

### Next
- Continue plan 02 phase 3 from server/lib/drivers/buffer/src/lib.rs:142
- Watch out for: DriverX depends on plan 03 completion
```

**Rules:**
- **Current Status** at top — always updated to reflect latest state
- **Append-only** flight entries — never edit previous flights
- **"Next" section** only on the latest flight — tells next session exactly where to pick up
- **Decisions recorded** — prevents re-debating things already decided

# Stop Protocol (MANDATORY)

**NEVER end a work session without running `/stop` or following this protocol manually.**

Claude sessions end for many reasons — context limits, user interruption, getting stuck. Regardless of the reason, you MUST hand off cleanly so the next session can resume without lost context.

**Before stopping, you MUST:**
1. Update `~/.claude/plans/reovim/{ISSUE}/flight-log.md` with what was done, decisions made, and exact resume point
2. Generate `tmp/commit.sh` if there are uncommitted changes
3. Report status to the user (what's done, what's left, where to resume)

**Red Flags (unprofessional):**
- Ending with "I'll stop here for now" without updating the flight log
- Leaving uncommitted work with no commit script
- Vague resume instructions like "continue from where we left off"

**Green Flags (professional):**
- Flight log updated with file paths and line numbers
- `tmp/commit.sh` ready for user to review and run
- Next session can start immediately from the flight log alone

**When context is getting large:** Proactively run `/stop` BEFORE compaction erases your progress. If you notice the conversation is very long, do the handoff early rather than risk losing context.

**System enforcement:** A `Stop` hook (`.claude/hooks/stop-check.sh`) blocks Claude from ending a session when tracked files have uncommitted changes. The hook forces Claude to run `/stop` first. On the second attempt (`stop_hook_active=true`), it allows through to prevent infinite loops.

# Cross-Session Communication

The `tmp/` directory is used for communication between Claude sessions:
- **Read `tmp/*` at start** to see proposals/notes from previous Claude sessions
- **Write to `tmp/*.md`** to leave proposals or context for user/future sessions

# Git Worktrees

Use `git worktree list` to see active worktrees. Each issue typically gets its own worktree (e.g., `reovim-{ISSUE_NUMBER}`).
