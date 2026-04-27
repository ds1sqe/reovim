#!/usr/bin/env bash
set -euo pipefail

# CORE purity test: clients/tui/src/ must not reference specific UI module
# type names or crate prefixes. The factory map and its tests are excluded
# because they name module crates (policy boundary) but contain no UI logic.
#
# This enforces the principle that CORE (render engine, app, notification
# handler) interacts with modules only via the ClientModule trait — never
# by concrete type name.

CORE_DIR="clients/tui/src"

EXCLUDE_GLOBS=(
    "--glob=!static_client_modules.rs"
    "--glob=!static_client_modules_tests.rs"
    "--glob=!*_tests.rs"
)

# Banned terms: concrete module types and crate prefixes
BANNED=(
    "StatuslineModule"
    "HoverModule"
    "SignatureHelpModule"
    "LandingModule"
    "CompletionModule"
    "NotificationModule"
    "WhichKeyModule"
    "CmdlineModule"
    "MicroscopeModule"
    "ExplorerModule"
    "TetrominoModule"
    "LineNumbersModule"
    "FoldModule"
    "JumpModule"
    "PairModule"
    "DiagnosticsModule"
    "MarkdownModule"
    "reovim_tui_mod_"
    "reovim_tui_ext_"
)

FAIL=0
for term in "${BANNED[@]}"; do
    if rg -l "$term" "${EXCLUDE_GLOBS[@]}" "$CORE_DIR" 2>/dev/null; then
        echo "FAIL: CORE references banned term: $term"
        FAIL=1
    fi
done

if [ "$FAIL" -eq 0 ]; then
    echo "PASS: CORE purity check passed"
else
    echo ""
    echo "CORE (clients/tui/src/) must not reference specific UI module types."
    echo "Use the ClientModule trait for all module interactions."
    exit 1
fi
