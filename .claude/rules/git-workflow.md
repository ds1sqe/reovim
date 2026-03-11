# Deferral Protocol (MANDATORY)

When deferring work (skipping a test, leaving a TODO, postponing a feature), you MUST:

1. **Create a draft proposal** - Write `tmp/deferral-draft-{topic}.md` with:
   ```markdown
   # Deferral Proposal: {topic}

   ## What
   [Description of deferred work]

   ## Why
   [Reason for deferring]

   ## Draft Issue
   - Title: `fix: {description}`
   - Body: [Acceptance criteria, context]

   ## Code Reference
   [File and line where `#[ignore]` or `// TODO` will be added]
   ```
2. **Continue working** - Don't block on approval
3. **Mention at finish** - At end of work session, remind user: "Review deferral draft at `tmp/deferral-draft-*.md`"
4. **After user approval** - Create issue with `gh issue create`, update code with issue reference

**Never create tracking issues without user approval.** The draft file gives user control over what gets tracked and how.

Example workflow:
```
[During work] Claude encounters failing test
[During work] Claude creates tmp/deferral-draft-dG-motion.md
[During work] Claude continues with other tasks
[At finish]  Claude: "Review deferral draft at tmp/deferral-draft-dG-motion.md"
[User]       Reviews and approves
[Claude]     Creates issue #429, updates #[ignore = "... (#429)"]
```

# Self-Contained Issues Policy

- **Every GitHub issue MUST be completely self-contained**
- A developer reading ONLY the issue must have ALL information needed to implement it
- **NEVER reference local files** - No "see `tmp/plan.md`", "details in `~/.claude/plans/...`", or "see file X"
- **NEVER reference plan files** - All relevant plan content must be copied INTO the issue
- **NEVER use relative file paths** for repo files - No "see `docs/heritage/legacy-memorial.md`"
- **DO use git blob permalinks** for repo file references:
  - Get blob URL: `gh browse <file> --commit HEAD` or GitHub UI
  - Format: `[Description](https://github.com/owner/repo/blob/<commit-sha>/path/to/file.md)`
  - Permalinks point to a specific version - paths break when files move or change
- Include in the issue itself:
  - Full technical context and architecture decisions
  - Acceptance criteria and test requirements
  - Code snippets, type definitions, file paths as needed
  - Any diagrams or examples (use markdown code blocks)
- **Epics follow the same rule** - Epic descriptions must be self-sufficient, not depend on external docs
- Local plan files (`tmp/`, `~/.claude/plans/`) are for Claude sessions only, not for issue references

# Issue Title Format

```
type: short description
```
- **Types**: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `proposal`
- Examples:
  - `feat: add insert mode character input`
  - `fix: cursor not updating after delete`
  - `chore: rename agents to NASA theme`
  - `docs: update architecture overview`
  - `proposal: new buffer management API`

# Issue Body Format

```markdown
## Summary
[1-3 sentences describing what and why]

## Changes
[Bullet list of planned changes]

## Why
[Motivation/context for this work]

## Related
- Part of #EPIC_NUMBER (if applicable)
- Depends on #ISSUE (if applicable)
```

# Commit Proposal Format

For any changes, Claude should create:
1. **`tmp/commit.sh`** - Executable script with git commands
2. Optionally **`tmp/<feature-name>-commit.md`** for complex commits with:
   - Proposed commit message
   - List of files changed (including `CHANGELOG.md`)
   - Verification checklist
