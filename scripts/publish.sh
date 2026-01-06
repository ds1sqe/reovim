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

# Publish order (dependencies first):
# 1. Base libs (sys, lsp)
# 2. Core
# 3. Feature plugins (microscope & treesitter first, then others, then lsp/pickers which depend on microscope)
# 4. Language plugins (all depend on treesitter)
# 5. Main binary
CRATES=(
    # Base libraries
    "lib/sys"
    "lib/lsp"

    # Core
    "lib/core"

    # Feature plugins - tier 1 (no inter-plugin deps)
    "plugins/features/microscope"
    "plugins/features/treesitter"
    "plugins/features/range-finder"
    "plugins/features/settings-menu"
    "plugins/features/completion"
    "plugins/features/explorer"
    "plugins/features/notification"
    "plugins/features/pair"
    "plugins/features/statusline"
    "plugins/features/which-key"
    "plugins/features/profiles"
    "plugins/features/health-check"
    "plugins/features/cmdline-completion"

    # Language plugins (all depend on treesitter)
    "plugins/languages/rust"
    "plugins/languages/c"
    "plugins/languages/javascript"
    "plugins/languages/python"
    "plugins/languages/json"
    "plugins/languages/toml"
    "plugins/languages/markdown"
    "plugins/languages/bash"

    # Feature plugins - tier 2 (depend on microscope + language plugins)
    "plugins/features/pickers"
    "plugins/features/lsp"

    # Main binary
    "runner"
)

NAMES=(
    # Base libraries
    "reovim-sys"
    "reovim-lsp"

    # Core
    "reovim-core"

    # Feature plugins - tier 1
    "reovim-plugin-microscope"
    "reovim-plugin-treesitter"
    "reovim-plugin-range-finder"
    "reovim-plugin-settings-menu"
    "reovim-plugin-completion"
    "reovim-plugin-explorer"
    "reovim-plugin-notification"
    "reovim-plugin-pair"
    "reovim-plugin-statusline"
    "reovim-plugin-which-key"
    "reovim-plugin-profiles"
    "reovim-plugin-health-check"
    "reovim-plugin-cmdline-completion"

    # Language plugins
    "reovim-lang-rust"
    "reovim-lang-c"
    "reovim-lang-javascript"
    "reovim-lang-python"
    "reovim-lang-json"
    "reovim-lang-toml"
    "reovim-lang-markdown"
    "reovim-lang-bash"

    # Feature plugins - tier 2
    "reovim-plugin-pickers"
    "reovim-plugin-lsp"

    # Main binary
    "reovim"
)

for i in "${!CRATES[@]}"; do
    CRATE_PATH="${CRATES[$i]}"
    CRATE_NAME="${NAMES[$i]}"

    echo ""
    echo "=== Publishing $CRATE_NAME ==="

    cd "$CRATE_PATH"

    # Check if this version already exists on crates.io
    PUBLISHED_VERSION=$(cargo search "$CRATE_NAME" --limit 1 2>/dev/null | grep -E "^$CRATE_NAME = " | sed 's/.*"\(.*\)".*/\1/' || echo "")

    if [ "$PUBLISHED_VERSION" = "$VERSION" ]; then
        echo "⏭️  Skipping $CRATE_NAME@$VERSION (already published)"
    else
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
