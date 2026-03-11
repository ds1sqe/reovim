#!/usr/bin/env bash
# Stop hook — blocks Claude from ending a session with uncommitted work.
#
# How it works:
#   1. Claude finishes responding → Stop hook fires
#   2. If stop_hook_active=true → allow (prevents infinite loop)
#   3. If no uncommitted changes → allow (nothing to hand off)
#   4. If uncommitted changes exist → block with reminder to run /stop
#
# After blocking, Claude runs /stop (updates flight log, creates
# tmp/commit.sh), then tries to stop again. On the second attempt
# stop_hook_active=true, so it passes through.
set -euo pipefail

INPUT=$(cat)

# Prevent infinite loop: if we already blocked once, allow stop
STOP_HOOK_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')
if [ "$STOP_HOOK_ACTIVE" = "true" ]; then
  exit 0
fi

CWD=$(echo "$INPUT" | jq -r '.cwd')
cd "$CWD" 2>/dev/null || exit 0

# Check for uncommitted changes (staged + unstaged + untracked in tracked dirs)
if git diff --quiet HEAD 2>/dev/null && git diff --cached --quiet 2>/dev/null; then
  # No changes to tracked files — allow stop
  exit 0
fi

# There are uncommitted changes — block and tell Claude to run /stop
cat <<'EOF'
{
  "decision": "block",
  "reason": "You have uncommitted changes. Run /stop to update the flight log and create tmp/commit.sh before ending the session."
}
EOF
