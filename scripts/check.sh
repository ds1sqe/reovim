#!/usr/bin/env bash
set -euo pipefail

# Legacy/render-related crates excluded from checks (Phase 8+ architecture)
# - runner: old monolithic binary (replaced by apps/reovim + lib/server)
# - display driver: v1 cell-grid rendering (client-side in v2)
# - layout module: v1 window layout (client-side in v2)
# - statusline module: v1 statusline rendering (client-side in v2)
# - which-key module: v1 popup rendering (client-side in v2)
EXCLUDE="--exclude runner"
EXCLUDE="$EXCLUDE --exclude reovim-driver-display"
EXCLUDE="$EXCLUDE --exclude reovim-module-layout"
EXCLUDE="$EXCLUDE --exclude reovim-module-statusline"
EXCLUDE="$EXCLUDE --exclude reovim-module-which-key"

echo -e "\033[1;33m==> Formatting code with nightly...\033[0m"
cargo +nightly fmt --all
echo -e "\033[0;32m✓ Code formatted\033[0m"

echo -e "\033[1;33m==> Checking code formatting...\033[0m"
cargo +nightly fmt --all -- --check
echo -e "\033[0;32m✓ Code formatting OK\033[0m"

echo -e "\033[1;33m==> Building workspace...\033[0m"
cargo build --workspace $EXCLUDE
echo -e "\033[0;32m✓ Build succeeded\033[0m"

echo -e "\033[1;33m==> Building reovim-app with gRPC for integration tests...\033[0m"
cargo build -p reovim-app --features grpc
echo -e "\033[0;32m✓ gRPC build succeeded\033[0m"

echo -e "\033[1;33m==> Running clippy...\033[0m"
cargo +nightly clippy --all-targets --all-features --workspace $EXCLUDE -- -D warnings
echo -e "\033[0;32m✓ Clippy passed\033[0m"

echo -e "\033[1;33m==> Running tests...\033[0m"
cargo test --workspace $EXCLUDE
echo -e "\033[0;32m✓ All tests passed\033[0m"

echo -e "\033[1;33m==> Running doc tests...\033[0m"
cargo test --doc --workspace $EXCLUDE
echo -e "\033[0;32m✓ Doc tests passed\033[0m"

echo -e "\033[1;32m==> All checks passed!\033[0m"
