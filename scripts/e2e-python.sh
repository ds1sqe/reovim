#!/usr/bin/env bash
#
# E2E tests for Python FFI module loading
#
# Runs the full integration test suite for Python modules via PyO3.
# These tests verify that Python modules can actually be loaded and executed.
#
# Requirements:
# - Python 3.11+ installed
# - pyo3 dependencies satisfied
#
# Usage:
#   ./scripts/e2e-python.sh          # Run all Python e2e tests
#   ./scripts/e2e-python.sh --verbose # Run with verbose output

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Parse arguments
VERBOSE=""
for arg in "$@"; do
    case $arg in
        --verbose|-v)
            VERBOSE="-- --nocapture"
            ;;
    esac
done

echo -e "${YELLOW}==> Running Python FFI E2E tests...${NC}"

# Check Python version
PYTHON_VERSION=$(python3 --version 2>&1 | cut -d' ' -f2)
PYTHON_MAJOR=$(echo "$PYTHON_VERSION" | cut -d'.' -f1)
PYTHON_MINOR=$(echo "$PYTHON_VERSION" | cut -d'.' -f2)

if [[ "$PYTHON_MAJOR" -lt 3 ]] || [[ "$PYTHON_MAJOR" -eq 3 && "$PYTHON_MINOR" -lt 11 ]]; then
    echo -e "${RED}Error: Python 3.11+ required, found $PYTHON_VERSION${NC}"
    exit 1
fi

echo -e "  Python version: ${GREEN}$PYTHON_VERSION${NC}"

# Run the tests
echo -e "${YELLOW}==> Building and running tests...${NC}"

if cargo test -p reovim --features python --test python_e2e $VERBOSE; then
    echo -e "${GREEN}==> All Python E2E tests passed!${NC}"
else
    echo -e "${RED}==> Some Python E2E tests failed${NC}"
    exit 1
fi
