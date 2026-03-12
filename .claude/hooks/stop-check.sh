#!/usr/bin/env bash
# Stop hook — enforces clean handoff before ending a session.
#
# 3-step gate (lightweight — no cargo commands):
#   1. Check test layout violations exist → block
#   2. Check uncommitted changes without tmp/commit.sh → block
#   3. Remind about check.sh and final approach → pass through with note
#
# On second attempt (stop_hook_active=true), allow through to prevent
# infinite loops.
set -euo pipefail

INPUT=$(cat)

# Prevent infinite loop: if we already blocked once, allow stop
STOP_HOOK_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')
if [ "$STOP_HOOK_ACTIVE" = "true" ]; then
  exit 0
fi

CWD=$(echo "$INPUT" | jq -r '.cwd')
cd "$CWD" 2>/dev/null || exit 0

# Step 1: Quick test layout check (fast — no compilation, ~2s)
if [ -x ./scripts/check-test-layout.sh ]; then
  if ! ./scripts/check-test-layout.sh --check > /dev/null 2>&1; then
    cat <<'EOF'
{
  "decision": "block",
  "reason": "Test layout violations detected (inline test blocks). Run ./scripts/check-test-layout.sh --fix to see what needs migration, then fix before stopping."
}
EOF
    exit 0
  fi
fi

# Step 2: Check for uncommitted changes without commit script
if ! git diff --quiet HEAD 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
  if [ ! -f tmp/commit.sh ]; then
    cat <<'EOF'
{
  "decision": "block",
  "reason": "You have uncommitted changes but no tmp/commit.sh. Run /stop to update the flight log and create tmp/commit.sh before ending the session."
}
EOF
    exit 0
  fi
fi

# Step 3: Remind about check.sh and final approach (non-blocking)
cat <<'EOF'
{
  "systemMessage": "tmp/commit.sh is ready. Before the user commits, make sure you have run ./scripts/check.sh and /final-approach for this issue."
}
EOF
