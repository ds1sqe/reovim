#!/bin/bash
# Create a new git worktree and copy Claude context files
#
# Usage: ./scripts/new-worktree.sh <issue-number-or-branch> [worktree-path]
#
# Examples:
#   ./scripts/new-worktree.sh 152                    # -> branch: reovim-152, path: ../reovim-152
#   ./scripts/new-worktree.sh feat/foo               # -> branch: feat/foo, path: ../reovim_foo
#   ./scripts/new-worktree.sh feat/foo ~/proj/foo    # -> branch: feat/foo, path: ~/proj/foo

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

usage() {
    echo "Usage: $0 <issue-number-or-branch> [worktree-path]"
    echo ""
    echo "Arguments:"
    echo "  issue-number   GitHub issue number (e.g., 152)"
    echo "  branch-name    Or a branch name (e.g., feat/phase0-crates)"
    echo "  worktree-path  Optional path for worktree"
    echo ""
    echo "Examples:"
    echo "  $0 152                         # ../reovim-152 with branch reovim-152"
    echo "  $0 feat/foo                    # ../reovim_foo with branch feat/foo"
    echo "  $0 feat/foo ~/proj/reovim_foo"
    exit 1
}

if [ -z "$1" ]; then
    usage
fi

INPUT="$1"

# Check if input is a number (issue number)
if [[ "$INPUT" =~ ^[0-9]+$ ]]; then
    BRANCH_NAME="reovim-$INPUT"
    WORKTREE_PATH="${2:-$(dirname "$REPO_ROOT")/reovim-$INPUT}"
else
    BRANCH_NAME="$INPUT"
    if [ -z "$2" ]; then
        BRANCH_SUFFIX="${BRANCH_NAME##*/}"
        WORKTREE_PATH="$(dirname "$REPO_ROOT")/reovim_${BRANCH_SUFFIX}"
    else
        WORKTREE_PATH="$2"
    fi
fi

echo -e "${YELLOW}Creating worktree...${NC}"
echo "  Branch: $BRANCH_NAME"
echo "  Path:   $WORKTREE_PATH"
echo ""

# Check if worktree path already exists
if [ -d "$WORKTREE_PATH" ]; then
    echo -e "${RED}Error: Worktree path already exists: $WORKTREE_PATH${NC}"
    exit 1
fi

# Check if branch already exists
if git show-ref --verify --quiet "refs/heads/$BRANCH_NAME"; then
    echo -e "${YELLOW}Branch '$BRANCH_NAME' exists, using existing branch${NC}"
    git worktree add "$WORKTREE_PATH" "$BRANCH_NAME"
else
    echo -e "${GREEN}Creating new branch '$BRANCH_NAME' from develop${NC}"
    git worktree add -b "$BRANCH_NAME" "$WORKTREE_PATH" develop
fi

echo ""
echo -e "${YELLOW}Copying Claude context files...${NC}"

# Copy .claude directory if it exists
if [ -d "$REPO_ROOT/.claude" ]; then
    rm -rf "$WORKTREE_PATH/.claude"
    cp -r "$REPO_ROOT/.claude/." "$WORKTREE_PATH/.claude/"
    echo "  Copied .claude/"
else
    echo "  No .claude/ directory found, skipping"
fi

# Copy tmp directory if it exists
if [ -d "$REPO_ROOT/tmp" ]; then
    rm -rf "$WORKTREE_PATH/tmp"
    cp -r "$REPO_ROOT/tmp/." "$WORKTREE_PATH/tmp/"
    echo "  Copied tmp/"
else
    mkdir -p "$WORKTREE_PATH/tmp"
    echo "  Created empty tmp/"
fi

echo ""
echo -e "${GREEN}Worktree created successfully!${NC}"
echo ""
echo "Next steps:"
echo "  cd $WORKTREE_PATH"
echo "  git status"
echo ""
echo "To remove worktree later:"
echo "  git worktree remove $WORKTREE_PATH"
