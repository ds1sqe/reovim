#!/bin/bash
# Fork a new git worktree from the current branch
#
# Interactive script — prompts for issue number, base branch, and path.
# Copies untracked Claude config (.claude/, CLAUDE.md) into the new worktree.
#
# Usage: ./scripts/fork.sh [issue-number]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CURRENT_BRANCH="$(git -C "$REPO_ROOT" branch --show-current)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# --- Step 1: Issue number ---
if [ -n "$1" ]; then
    ISSUE="$1"
else
    echo -en "${CYAN}Issue number or branch name: ${NC}"
    read -r ISSUE
fi

if [ -z "$ISSUE" ]; then
    echo -e "${RED}Error: issue number or branch name required${NC}"
    exit 1
fi

# Derive branch name
if [[ "$ISSUE" =~ ^[0-9]+$ ]]; then
    BRANCH_NAME="reovim-$ISSUE"
else
    BRANCH_NAME="$ISSUE"
fi

# Check if branch already exists
if git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$BRANCH_NAME"; then
    echo -e "${YELLOW}Branch '$BRANCH_NAME' already exists.${NC}"
    echo -en "${CYAN}Attach worktree to existing branch? [Y/n]: ${NC}"
    read -r ATTACH
    if [[ "$ATTACH" =~ ^[Nn] ]]; then
        echo "Aborted."
        exit 0
    fi
    EXISTING_BRANCH=true
else
    EXISTING_BRANCH=false
fi

# --- Step 2: Base branch (only for new branches) ---
if [ "$EXISTING_BRANCH" = false ]; then
    echo -e "${BOLD}Current branch:${NC} $CURRENT_BRANCH"
    echo -en "${CYAN}Fork from [$CURRENT_BRANCH]: ${NC}"
    read -r BASE
    BASE="${BASE:-$CURRENT_BRANCH}"

    # Verify base exists
    if ! git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/$BASE" 2>/dev/null &&
       ! git -C "$REPO_ROOT" rev-parse --verify "$BASE" >/dev/null 2>&1; then
        echo -e "${RED}Error: '$BASE' does not exist${NC}"
        exit 1
    fi
fi

# --- Step 3: Worktree path ---
if [[ "$ISSUE" =~ ^[0-9]+$ ]]; then
    DEFAULT_PATH="$(dirname "$REPO_ROOT")/reovim-$ISSUE"
else
    SUFFIX="${BRANCH_NAME##*/}"
    DEFAULT_PATH="$(dirname "$REPO_ROOT")/reovim-${SUFFIX}"
fi

echo -en "${CYAN}Worktree path [$DEFAULT_PATH]: ${NC}"
read -r WPATH
WPATH="${WPATH:-$DEFAULT_PATH}"

if [ -d "$WPATH" ]; then
    echo -e "${RED}Error: path already exists: $WPATH${NC}"
    exit 1
fi

# --- Confirm ---
echo ""
echo -e "${BOLD}Summary:${NC}"
echo "  Branch: $BRANCH_NAME"
if [ "$EXISTING_BRANCH" = false ]; then
    echo "  Base:   $BASE"
fi
echo "  Path:   $WPATH"
echo ""
echo -en "${CYAN}Create worktree? [Y/n]: ${NC}"
read -r CONFIRM
if [[ "$CONFIRM" =~ ^[Nn] ]]; then
    echo "Aborted."
    exit 0
fi

# --- Create worktree ---
echo ""
if [ "$EXISTING_BRANCH" = true ]; then
    git -C "$REPO_ROOT" worktree add "$WPATH" "$BRANCH_NAME"
else
    echo -e "${GREEN}Creating branch '$BRANCH_NAME' from '$BASE'${NC}"
    git -C "$REPO_ROOT" worktree add -b "$BRANCH_NAME" "$WPATH" "$BASE"
fi

# --- Copy untracked Claude config ---
echo ""
echo -e "${YELLOW}Copying Claude config...${NC}"

if [ -f "$REPO_ROOT/CLAUDE.md" ]; then
    cp "$REPO_ROOT/CLAUDE.md" "$WPATH/CLAUDE.md"
    echo "  CLAUDE.md"
fi

if [ -d "$REPO_ROOT/.claude" ]; then
    rm -rf "$WPATH/.claude"
    cp -r "$REPO_ROOT/.claude/." "$WPATH/.claude/"
    echo "  .claude/"
fi

mkdir -p "$WPATH/tmp"
echo "  tmp/"

# --- Done ---
echo ""
echo -e "${GREEN}Done!${NC}"
echo ""
echo "  cd $WPATH"
