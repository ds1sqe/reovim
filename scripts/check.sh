#!/usr/bin/env bash
# Check script: Run all tests and clippy for the entire workspace
# Usage: ./scripts/check.sh [--quick]
#
# Options:
#   --quick    Skip slow tests and benchmarks, only run essential checks

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_step() {
    echo -e "\n${YELLOW}==> $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

QUICK_MODE=false
if [[ "$1" == "--quick" ]]; then
    QUICK_MODE=true
fi

# Step 1: Format all workspaces with nightly
print_step "Formatting code with nightly..."
if cargo +nightly fmt --all; then
    print_success "Code formatted"
else
    print_error "Code formatting failed"
    exit 1
fi

# Step 2: Format check
print_step "Checking code formatting..."
if cargo fmt --check; then
    print_success "Code formatting OK"
else
    print_error "Code formatting issues found. Run 'cargo fmt' to fix."
    exit 1
fi

# Step 3: Build
print_step "Building workspace..."
if cargo build --workspace; then
    print_success "Build succeeded"
else
    print_error "Build failed"
    exit 1
fi

# Step 4: Build release
print_step "Building workspace release..."
if cargo build --release --workspace; then
    print_success "Build succeeded"
else
    print_error "Build failed"
    exit 1
fi

# Step 5: Clippy on all targets
# Uses workspace lint configuration from Cargo.toml
# (clippy::all = "deny", clippy::pedantic/nursery = "warn")
print_step "Running clippy on all workspace targets..."
if cargo clippy --workspace --all-targets; then
    print_success "Clippy passed"
else
    print_error "Clippy found errors"
    exit 1
fi

# Step 6: Tests
print_step "Running tests..."
if cargo test --workspace; then
    print_success "All tests passed"
else
    print_error "Tests failed"
    exit 1
fi

# Step 7: Doc tests (optional in quick mode)
if [[ "$QUICK_MODE" == false ]]; then
    print_step "Running doc tests..."
    if cargo test --doc --workspace; then
        print_success "Doc tests passed"
    else
        print_error "Doc tests failed"
        exit 1
    fi
fi

echo -e "\n${GREEN}========================================${NC}"
echo -e "${GREEN}All checks passed!${NC}"
echo -e "${GREEN}========================================${NC}"
