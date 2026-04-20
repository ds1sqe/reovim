#!/bin/bash
# Publish workspace crates to crates.io in dependency order.
# Usage: ./scripts/publish.sh [--dry-run]
#
# Crate order is a topological sort of internal workspace dependencies
# (11 tiers, 135 publishable crates). Regenerate with:
#   cargo metadata --no-deps | python3 scripts/publish-order.py

set -e

DRY_RUN=""
SKIP_TESTS=""
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN="--dry-run"; echo "=== DRY RUN MODE ===" ;;
        --skip-tests) SKIP_TESTS=1 ;;
    esac
done

# Get current version from workspace
VERSION=$(grep -A2 '^\[workspace\.package\]' Cargo.toml | grep '^version' | sed 's/.*"\(.*\)"/\1/')
echo "Publishing version: $VERSION"

if [ -z "$VERSION" ]; then
    echo "Error: Could not determine workspace version"
    exit 1
fi

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

if [ -z "$SKIP_TESTS" ]; then
    # Run tests first
    echo ""
    echo "=== Running tests ==="
    cargo test --quiet

    # Run clippy
    echo ""
    echo "=== Running clippy ==="
    cargo clippy --quiet
else
    echo "=== Skipping tests and clippy ==="
fi

# Publish order: topological sort of workspace dependencies (tiers 0-10).
# Skips: perf-report, reovim-bench,
#         reovim-test-dynamic-module (not publishable).
CRATES=(
    # Tier 0 — no internal deps
    "arch"                                         # reovim-arch
    "lib/capabilities"                             # reovim-capabilities
    "clients/lib/model"                            # reovim-client-model
    "lib/depgraph"                                 # reovim-depgraph

    # Tier 1 — kernel, protocol, base drivers
    "clients/lib/driver"                           # reovim-client-driver
    "server/lib/subsys/module-registry"            # reovim-subsys-module-registry
    "ext/client/tui/drivers/tui"                   # reovim-driver-tui
    "server/lib/kernel"                            # reovim-kernel
    "uapi/protocol"                                # reovim-protocol

    # Tier 2 — drivers, shared libs, TUI client modules
    "clients/cli"                                  # reovim-client-cli
    "server/lib/subsys/annotation"                 # reovim-subsys-annotation
    "ext/server/drivers/buffer"                    # reovim-driver-buffer
    "server/lib/subsys/clipboard"                  # reovim-subsys-clipboard
    "server/lib/subsys/completion"                 # reovim-subsys-completion
    "server/lib/subsys/formatter"                  # reovim-subsys-formatter
    "server/lib/subsys/git"                        # reovim-subsys-git
    "server/lib/subsys/layout"                     # reovim-subsys-layout
    "server/lib/subsys/manifest"                   # reovim-subsys-manifest
    "server/lib/subsys/module-config"              # reovim-subsys-module-config
    "server/lib/subsys/module-loader"               # reovim-subsys-module-loader
    "server/lib/subsys/net"                        # reovim-subsys-net
    "ext/server/drivers/search"                    # reovim-driver-search
    "server/lib/subsys/statusline"                 # reovim-subsys-statusline
    "server/lib/subsys/vfs"                       # reovim-subsys-vfs
    "uapi/module-macros"                           # reovim-module-macros
    "ext/client/tui/modules/bufferline"               # reovim-tui-mod-bufferline
    "ext/client/tui/modules/cmdline"                  # reovim-tui-mod-cmdline
    "ext/client/tui/modules/completion"               # reovim-tui-mod-completion
    "ext/client/tui/modules/diagnostics"              # reovim-tui-mod-diagnostics
    "ext/client/tui/modules/explorer"                 # reovim-tui-mod-explorer
    "ext/client/tui/modules/fold"                     # reovim-tui-mod-fold
    "ext/client/tui/modules/hover"                    # reovim-tui-mod-hover
    "ext/client/tui/modules/illuminate"               # reovim-tui-mod-illuminate
    "ext/client/tui/modules/jump"                     # reovim-tui-mod-jump
    "ext/client/tui/modules/landing"                  # reovim-tui-mod-landing
    "ext/client/tui/modules/line-numbers"             # reovim-tui-mod-line-numbers
    "ext/client/tui/modules/markdown"                 # reovim-tui-mod-markdown
    "ext/client/tui/modules/microscope"               # reovim-tui-mod-microscope
    "ext/client/tui/modules/notification"             # reovim-tui-mod-notification
    "ext/client/tui/modules/pair"                     # reovim-tui-mod-pair
    "ext/client/tui/modules/signature-help"           # reovim-tui-mod-signature-help
    "ext/client/tui/modules/statusline"               # reovim-tui-mod-statusline
    "ext/client/tui/modules/tetromino"                # reovim-tui-mod-tetromino
    "ext/client/tui/modules/which-key"                # reovim-tui-mod-whichkey
    "ext/client/tui/modules/yank-flash"               # reovim-tui-mod-yank-flash

    # Tier 3 — testing, bench, command types, display, base server modules
    "tools/testing"                                # reovim-testing
    "lib/bench"                                    # reovim-bench-utils
    "ext/server/drivers/command-types"             # reovim-driver-command-types
    "ext/client/tui/drivers/display"               # reovim-driver-display
    "ext/server/drivers/undo"                      # reovim-driver-undo
    "ext/server/modules/buffer-ops"                    # reovim-module-buffer-ops
    "ext/server/modules/buffer-simple"                 # reovim-module-buffer-simple
    "ext/server/modules/git"                           # reovim-module-git
    "ext/server/modules/git-blame"                     # reovim-module-git-blame
    "ext/server/modules/git-statusline"                # reovim-module-git-statusline
    "ext/server/modules/keymap"                        # reovim-module-keymap
    "ext/server/modules/layout"                        # reovim-module-layout
    "ext/server/modules/mode-manager"                  # reovim-module-mode-manager
    "ext/server/modules/search"                        # reovim-module-search
    "ext/server/modules/vfs-local"                     # reovim-module-vfs-local

    # Tier 4 — TUI client, session driver
    "clients/tui"                                  # reovim-client-tui
    "ext/server/drivers/session"                   # reovim-driver-session
    "ext/server/modules/clipboard"                     # reovim-module-clipboard
    "ext/server/modules/undo"                          # reovim-module-undo

    # Content codec foundation
    "ext/content-codec/xxd"                        # reovim-content-codec-xxd

    # Tier 5 — higher-level drivers
    "ext/server/drivers/command"                   # reovim-driver-command
    "ext/server/drivers/ffi"                       # reovim-driver-ffi
    "ext/server/drivers/input"                     # reovim-driver-input
    "ext/server/drivers/lsp"                       # reovim-driver-lsp
    "ext/server/drivers/picker"                    # reovim-driver-picker
    "ext/server/drivers/syntax"                    # reovim-driver-syntax
    "ext/server/modules/cmdline"                       # reovim-module-cmdline
    "ext/server/modules/notification"                  # reovim-module-notification
    "ext/server/modules/scratch-buffer"                # reovim-module-scratch-buffer
    "ext/server/modules/settings"                      # reovim-module-settings

    # Tier 6 — treesitter, codecs, feature modules, server, pickers
    "ext/server/drivers/ffi-python"                # reovim-driver-ffi-python
    "ext/server/drivers/syntax-treesitter"         # reovim-driver-syntax-treesitter
    "ext/content-codec/binary-struct"                  # reovim-content-codec-binary-struct
    "ext/content-codec/cjk"                            # reovim-content-codec-cjk
    "ext/content-codec/csv"                            # reovim-content-codec-csv
    "ext/content-codec/hex"                            # reovim-content-codec-hex
    "ext/content-codec/legacy"                         # reovim-content-codec-legacy
    "ext/content-codec/pdf"                            # reovim-content-codec-pdf
    "ext/content-codec/rlib"                           # reovim-content-codec-rlib
    "ext/content-codec/tar-gz"                        # reovim-content-codec-tar-gz
    "ext/content-codec/utf8"                           # reovim-content-codec-utf8
    "ext/server/modules/commands"                      # reovim-module-commands
    "ext/server/modules/context"                       # reovim-module-context
    "ext/server/modules/editor"                        # reovim-module-editor
    "ext/server/modules/format"                        # reovim-module-format
    "ext/server/modules/health-check"                  # reovim-module-health-check
    "ext/server/modules/indent-guide"                  # reovim-module-indent-guide
    "ext/server/modules/lsp"                           # reovim-module-lsp
    "ext/server/modules/module-manager"                # reovim-module-module-manager
    "ext/server/modules/motions"                       # reovim-module-motions
    "ext/server/modules/pair"                          # reovim-module-pair
    "ext/server/modules/profiles"                      # reovim-module-profiles
    "ext/server/modules/tetromino"                     # reovim-module-tetromino
    "ext/server/modules/textobjects"                   # reovim-module-textobjects
    "ext/server/modules/whichkey"                      # reovim-module-whichkey
    "ext/server/modules/window-ops"                    # reovim-module-window-ops
    "ext/server/modules/picker-buffers"                # reovim-picker-buffers
    "ext/server/modules/picker-commands"               # reovim-picker-commands
    "ext/server/modules/picker-files"                  # reovim-picker-files
    "ext/server/modules/picker-git-branches"           # reovim-picker-git-branches
    "ext/server/modules/picker-git-log"                # reovim-picker-git-log
    "ext/server/modules/picker-git-stash"              # reovim-picker-git-stash
    "ext/server/modules/picker-git-status"             # reovim-picker-git-status
    "ext/server/modules/picker-grep"                   # reovim-picker-grep
    "ext/server/modules/picker-options"                # reovim-picker-options
    "server/lib/server"                            # reovim-server

    # Tier 7 — language modules, emacs, vim
    "ext/server/modules/emacs"                         # reovim-module-emacs
    "ext/server/modules/sticky-context"                # reovim-module-sticky-context
    "ext/server/modules/treesitter-bash"               # reovim-module-treesitter-bash
    "ext/server/modules/treesitter-c"                  # reovim-module-treesitter-c
    "ext/server/modules/treesitter-go"                 # reovim-module-treesitter-go
    "ext/server/modules/treesitter-javascript"         # reovim-module-treesitter-javascript
    "ext/server/modules/treesitter-json"               # reovim-module-treesitter-json
    "ext/server/modules/treesitter-python"             # reovim-module-treesitter-python
    "ext/server/modules/treesitter-rust"               # reovim-module-treesitter-rust
    "ext/server/modules/treesitter-toml"               # reovim-module-treesitter-toml
    "ext/server/modules/treesitter-typescript"         # reovim-module-treesitter-typescript
    "ext/server/modules/vim"                           # reovim-module-vim

    # Tier 8 — high-level feature modules
    "ext/server/modules/bufferline"                    # reovim-module-bufferline
    "ext/server/modules/completion"                    # reovim-module-completion
    "ext/server/modules/diagnostics-panel"             # reovim-module-diagnostics-panel
    "ext/server/modules/explorer"                      # reovim-module-explorer
    "ext/server/modules/git-signs"                     # reovim-module-git-signs
    "ext/server/modules/illuminate"                    # reovim-module-illuminate
    "ext/server/modules/microscope"                    # reovim-module-microscope
    "ext/server/modules/range-finder"                  # reovim-module-range-finder
    "ext/server/modules/snippet"                       # reovim-module-snippet
    "ext/server/modules/treesitter-markdown"           # reovim-module-treesitter-markdown

    # Tier 9 — depends on everything
    "ext/server/modules/lsp-navigation"                # reovim-module-lsp-navigation

    # Tier 10 — main binary (last)
    "apps/bin"                                     # reovim
)

TOTAL=${#CRATES[@]}
PUBLISHED=0
SKIPPED=0
FAILED=0

# crates.io rate limit: ~1 publish/minute for new crates.
# publish_crate retries up to MAX_RETRIES with exponential backoff on 429s.
MAX_RETRIES=10
INDEX_WAIT=3  # seconds to wait for crates.io indexing between publishes

# Return codes: 0 = published, 2 = already exists (skip), 1 = error
publish_crate() {
    local attempt=1
    local wait=35

    while [ $attempt -le $MAX_RETRIES ]; do
        local output
        output=$(cargo publish 2>&1)
        local rc=$?

        if [ $rc -eq 0 ]; then
            return 0
        fi

        # Already published — not an error, just skip
        if echo "$output" | grep -qiE 'already exists|already uploaded'; then
            echo "  Already published — skipping"
            return 2
        fi

        # Check for rate limit (429) or "try again" messages
        if echo "$output" | grep -qiE '429|rate limit|try again|too many requests'; then
            echo "  Rate limited (attempt $attempt/$MAX_RETRIES), waiting ${wait}s..."
            sleep $wait
            wait=$((wait * 2))  # exponential backoff: 30, 60, 120, 240, 480
            attempt=$((attempt + 1))
        else
            # Non-rate-limit error — print and fail
            echo "$output" | tail -5
            return 1
        fi
    done

    echo "  Exhausted retries after $MAX_RETRIES attempts"
    return 1
}

for i in "${!CRATES[@]}"; do
    CRATE_PATH="${CRATES[$i]}"
    # Extract crate name from the comment
    CRATE_NAME=$(grep -m1 '^name' "$CRATE_PATH/Cargo.toml" | sed 's/name = "\(.*\)"/\1/')

    echo ""
    echo "=== [$((i+1))/$TOTAL] Publishing $CRATE_NAME ($CRATE_PATH) ==="

    cd "$CRATE_PATH"

    if [ -n "$DRY_RUN" ]; then
        cargo publish --dry-run --allow-dirty 2>&1 | tail -3
    else
        rc=0
        publish_crate || rc=$?
        if [ $rc -eq 0 ]; then
            PUBLISHED=$((PUBLISHED + 1))
            # Wait for crates.io to index only after a real publish
            if [ $i -lt $((TOTAL - 1)) ]; then
                printf "  Indexing "
                for t in $(seq $INDEX_WAIT -1 1); do
                    printf "%d..." "$t"
                    sleep 0.25; printf "."; sleep 0.25; printf "."; sleep 0.25; printf ". "; sleep 0.25
                done
                echo "go"
            fi
        elif [ $rc -eq 2 ]; then
            SKIPPED=$((SKIPPED + 1))
        else
            echo "  FAILED to publish $CRATE_NAME"
            FAILED=$((FAILED + 1))
        fi
    fi

    cd - > /dev/null
done

echo ""
echo "=== Done ==="
echo "Published: $PUBLISHED  Skipped: $SKIPPED  Failed: $FAILED  Total: $TOTAL"
if [ -z "$DRY_RUN" ] && [ $FAILED -eq 0 ]; then
    echo ""
    echo "Don't forget to:"
    echo "  1. Create a git tag: git tag v$VERSION"
    echo "  2. Push the tag: git push origin v$VERSION"
    echo "  3. Create a GitHub release: gh release create v$VERSION --title 'v$VERSION'"
fi
