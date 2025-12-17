#!/bin/bash
# Publish workspace crates to crates.io
# Usage: ./scripts/publish.sh [--dry-run]

set -e

DRY_RUN=""
if [ "$1" = "--dry-run" ]; then
    DRY_RUN="--dry-run"
    echo "=== DRY RUN MODE ==="
fi

# Get current version
VERSION=$(grep -E '^version = "' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "Publishing version: $VERSION"

# Check for uncommitted changes
if [ -n "$(git status --porcelain)" ]; then
    echo "Warning: You have uncommitted changes"
    if [ -z "$DRY_RUN" ]; then
        read -p "Continue anyway? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
fi

# Run tests first
echo ""
echo "=== Running tests ==="
cargo test --quiet

# Run clippy
echo ""
echo "=== Running clippy ==="
cargo clippy --quiet

# Publish order: sys -> core -> main (dependencies first)
CRATES=("lib/sys" "lib/core" "main")
NAMES=("reovim-sys" "reovim-core" "reovim")

for i in "${!CRATES[@]}"; do
    CRATE_PATH="${CRATES[$i]}"
    CRATE_NAME="${NAMES[$i]}"

    echo ""
    echo "=== Publishing $CRATE_NAME ==="

    cd "$CRATE_PATH"

    if [ -n "$DRY_RUN" ]; then
        cargo publish --dry-run --allow-dirty
    else
        cargo publish

        # Wait for crates.io to index (except for last crate)
        if [ $i -lt $((${#CRATES[@]} - 1)) ]; then
            echo "Waiting for crates.io to index $CRATE_NAME..."
            sleep 30
        fi
    fi

    cd - > /dev/null
done

echo ""
echo "=== Done ==="
if [ -z "$DRY_RUN" ]; then
    echo "Published $VERSION to crates.io"
    echo ""
    echo "Don't forget to:"
    echo "  1. Create a git tag: git tag v$VERSION"
    echo "  2. Push the tag: git push origin v$VERSION"
fi
