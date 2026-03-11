---
name: rebase
description: "Rebase current branch onto a specified target. Handles fetch for remote targets, stash/restore, and conflict resolution."
disable-model-invocation: true
argument-hint: "[target-branch]"
---

# Rebase Procedure

Rebase is a **standalone workflow** invoked explicitly by the user — NOT part of Final Approach.
The user always specifies the rebase target. There is no default.

## Usage

```
/rebase origin/develop
/rebase develop
/rebase phase-10
```

## Target Format

- `origin/develop` — remote branch (fetch + rebase)
- `develop` — local branch (rebase only, no fetch)
- `phase-10` — local branch (rebase only)
- `origin/feature-x` — remote branch (fetch + rebase)

Rule: if target contains `origin/`, fetch first. Otherwise rebase against local only.

## Steps

When invoked with `$ARGUMENTS` as the target:

1. Check what changed: `git log HEAD..$ARGUMENTS` and `git diff HEAD..$ARGUMENTS`
2. If remote target (`origin/*`): check context with `gh pr list --state merged --limit 5`
3. Stash current work: `git stash`
4. If remote target: `git fetch origin`
5. Rebase: `git rebase $ARGUMENTS`
6. Resolve conflicts carefully — contributors own their conflicts
7. Restore: `git stash pop`
8. Verify: `./scripts/check.sh`
