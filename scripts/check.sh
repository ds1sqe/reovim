#!/usr/bin/env bash
set -euo pipefail

echo -e "\033[1;33m==> Formatting code with nightly...\033[0m"
cargo +nightly fmt --all
echo -e "\033[0;32m✓ Code formatted\033[0m"

echo -e "\033[1;33m==> Checking code formatting...\033[0m"
cargo +nightly fmt --all -- --check
echo -e "\033[0;32m✓ Code formatting OK\033[0m"

echo -e "\033[1;33m==> Building workspace...\033[0m"
cargo build --workspace
echo -e "\033[0;32m✓ Build succeeded\033[0m"

echo -e "\033[1;33m==> Running clippy...\033[0m"
cargo +nightly clippy --all-targets --all-features --workspace -- -D warnings
echo -e "\033[0;32m✓ Clippy passed\033[0m"

echo -e "\033[1;33m==> Running tests...\033[0m"
cargo test --workspace
echo -e "\033[0;32m✓ All tests passed\033[0m"

echo -e "\033[1;33m==> Running doc tests...\033[0m"
cargo test --doc --workspace
echo -e "\033[0;32m✓ Doc tests passed\033[0m"

echo -e "\033[1;32m==> All checks passed!\033[0m"
