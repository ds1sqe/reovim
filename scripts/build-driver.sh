#!/usr/bin/env bash
# Build reovim server drivers as shared libraries for dynamic loading.
#
# Sibling to scripts/build-module.sh. Iterates the migrated server-driver
# crate set, builds each with `--features dynamic`, stages the resulting
# .so artifacts to a discoverable layout under `target/<profile>/lib/reovim/driver/server/`,
# and audits each artifact for exactly one `REOVIM_*_DRIVER_VTABLE` symbol.
#
# Phase 4 (#769) reduced its cdylib migration to scaffolding-only after
# the round-3 trait FFI-routability audit. The migrated server-driver set
# is currently empty; #774 will populate it. This script handles the
# empty set gracefully: it logs the empty-set notice and exits 0.
#
# When #774 lands a driver, add the crate name to the MIGRATED_DRIVERS
# array below — that is the only edit needed to extend the script.
#
# Usage:
#   ./scripts/build-driver.sh --all          # Build all migrated drivers (release)
#   ./scripts/build-driver.sh --all --debug  # Build all migrated drivers (debug)
#   ./scripts/build-driver.sh <name>         # Build single migrated driver
#
# Options:
#   --all     Build all migrated drivers
#   --debug   Build in debug mode (default: release)
#   --verify  Verify FFI vtable symbols after build (default: on)
#   --no-verify  Skip FFI vtable symbol verification
#
# Output:
#   Release: target/release/libreovim_driver_<name>.so (Linux)
#            target/release/libreovim_driver_<name>.dylib (macOS)
#            target/release/reovim_driver_<name>.dll (Windows)
#   Staging: target/<profile>/lib/reovim/driver/server/<basename>
set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get the repository root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ============================================================================
# Migrated driver set — Phase 4 scaffolding-only (#769)
# ============================================================================
# Add a crate name to this array when #774 lands a server-driver cdylib
# migration. Example:
#     MIGRATED_DRIVERS=("text-syntax")
#
# The empty array below is intentional. When empty the script logs and
# exits 0 (--all path) or errors with a clear message (single-driver path).
# ============================================================================
MIGRATED_DRIVERS=()

# Default options
BUILD_MODE="release"
VERIFY=true
BUILD_ALL=false
DRIVER_NAME=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --all)
            BUILD_ALL=true
            shift
            ;;
        --debug)
            BUILD_MODE="debug"
            shift
            ;;
        --verify)
            VERIFY=true
            shift
            ;;
        --no-verify)
            VERIFY=false
            shift
            ;;
        -h|--help)
            head -32 "$0" | tail -30
            exit 0
            ;;
        -*)
            echo -e "${RED}Error: Unknown option $1${NC}" >&2
            exit 1
            ;;
        *)
            if [[ -z "$DRIVER_NAME" ]]; then
                DRIVER_NAME="$1"
            else
                echo -e "${RED}Error: Multiple driver names specified${NC}" >&2
                exit 1
            fi
            shift
            ;;
    esac
done

# Validate arguments
if [[ "$BUILD_ALL" == false && -z "$DRIVER_NAME" ]]; then
    echo -e "${RED}Error: Specify a driver name or --all${NC}" >&2
    echo "Usage: $0 <driver-name> [--debug]"
    echo "       $0 --all [--debug]"
    exit 1
fi

# Determine shared library extension based on OS
get_lib_extension() {
    case "$(uname -s)" in
        Linux*)  echo "so" ;;
        Darwin*) echo "dylib" ;;
        MINGW*|MSYS*|CYGWIN*) echo "dll" ;;
        *)       echo "so" ;;
    esac
}

# Determine library prefix based on OS
get_lib_prefix() {
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*) echo "" ;;
        *)                     echo "lib" ;;
    esac
}

LIB_EXT=$(get_lib_extension)
LIB_PREFIX=$(get_lib_prefix)

# Convert driver short-name (e.g. "text-syntax") to package name
# (e.g. "reovim-driver-text-syntax").
get_package_name() {
    local driver=$1
    echo "reovim-driver-${driver}"
}

# Convert package name to library file basename (replace - with _)
get_lib_name() {
    local package_name=$1
    echo "$package_name" | tr '-' '_'
}

# Verify the produced .so contains exactly one REOVIM_*_DRIVER_VTABLE symbol.
verify_driver_vtable_symbol() {
    local lib_path=$1

    if [[ ! -f "$lib_path" ]]; then
        echo -e "${RED}Error: Library not found: $lib_path${NC}" >&2
        return 1
    fi

    echo -e "${BLUE}  Verifying driver vtable symbol...${NC}"

    local symbol_dump
    symbol_dump="$(nm -gD "$lib_path" 2>/dev/null || nm "$lib_path" 2>/dev/null || true)"

    # Match REOVIM_<KIND>_DRIVER_VTABLE (anchored on word boundary).
    local matches
    matches=$(grep -E '\bREOVIM_[A-Z_]+_DRIVER_VTABLE\b' <<< "$symbol_dump" || true)
    local count
    count=$(printf '%s\n' "$matches" | grep -c . || true)

    if [[ "$count" -eq 0 ]]; then
        echo -e "${RED}    Missing REOVIM_<KIND>_DRIVER_VTABLE symbol${NC}" >&2
        echo -e "${YELLOW}    Hint: Ensure the driver invokes declare_server_*_driver!()${NC}" >&2
        return 1
    fi

    if [[ "$count" -gt 1 ]]; then
        echo -e "${RED}    Multiple REOVIM_*_DRIVER_VTABLE symbols ($count) — expected exactly 1${NC}" >&2
        printf '%s\n' "$matches" | sed 's/^/      /' >&2
        return 1
    fi

    echo -e "${GREEN}  Driver vtable symbol verified${NC}"
    return 0
}

# Stage the produced .so into target/<profile>/lib/reovim/driver/server/.
stage_artifact() {
    local lib_path=$1

    local stage_dir="$REPO_ROOT/target/$BUILD_MODE/lib/reovim/driver/server"
    mkdir -p "$stage_dir"
    cp "$lib_path" "$stage_dir/"

    echo -e "${GREEN}  Staged: $stage_dir/$(basename "$lib_path")${NC}"
}

# Build a single migrated driver
build_driver() {
    local driver=$1

    echo -e "${YELLOW}==> Building driver: $driver${NC}"

    local package_name
    package_name=$(get_package_name "$driver")
    local cargo_toml="$REPO_ROOT/ext/server/drivers/$driver/Cargo.toml"

    if [[ ! -f "$cargo_toml" ]]; then
        echo -e "${RED}Error: Driver '$driver' not found at ext/server/drivers/$driver/${NC}" >&2
        return 1
    fi

    if ! grep -q 'cdylib' "$cargo_toml"; then
        echo -e "${RED}Error: Driver '$driver' does not have cdylib in Cargo.toml${NC}" >&2
        echo -e "${YELLOW}Hint: Add 'crate-type = [\"cdylib\", \"rlib\"]' to [lib] section${NC}" >&2
        return 1
    fi

    local lib_name
    lib_name=$(get_lib_name "$package_name")

    # Build the driver with dynamic feature enabled
    local cargo_args=("build" "-p" "$package_name" "--lib" "--features" "dynamic")
    if [[ "$BUILD_MODE" == "release" ]]; then
        cargo_args+=("--release")
    fi

    echo -e "${BLUE}  cargo ${cargo_args[*]}${NC}"
    cargo "${cargo_args[@]}"

    # Determine output path
    local target_dir="$REPO_ROOT/target/$BUILD_MODE"
    local lib_path="$target_dir/${LIB_PREFIX}${lib_name}.${LIB_EXT}"

    if [[ ! -f "$lib_path" ]]; then
        echo -e "${RED}Error: Expected library not found: $lib_path${NC}" >&2
        return 1
    fi

    echo -e "${GREEN}  Built: $lib_path${NC}"

    # Verify vtable symbol
    if [[ "$VERIFY" == true ]]; then
        verify_driver_vtable_symbol "$lib_path" || return 1
    fi

    # Stage to the runtime-discoverable layout
    stage_artifact "$lib_path"

    return 0
}

# Main execution
cd "$REPO_ROOT"

if [[ "$BUILD_ALL" == true ]]; then
    if [[ ${#MIGRATED_DRIVERS[@]} -eq 0 ]]; then
        echo -e "${BLUE}==> No migrated server drivers yet (#769 Phase 4 is scaffolding-only;${NC}"
        echo -e "${BLUE}    #774 will populate the migrated set). Exiting cleanly.${NC}"
        exit 0
    fi

    echo -e "${YELLOW}==> Building all migrated server drivers${NC}"
    echo -e "${BLUE}Found ${#MIGRATED_DRIVERS[@]} driver(s): ${MIGRATED_DRIVERS[*]}${NC}"

    failed=0
    for driver in "${MIGRATED_DRIVERS[@]}"; do
        if ! build_driver "$driver"; then
            failed=$((failed + 1))
        fi
        echo ""
    done

    if [[ $failed -gt 0 ]]; then
        echo -e "${RED}==> $failed driver(s) failed to build${NC}"
        exit 1
    fi

    echo -e "${GREEN}==> All ${#MIGRATED_DRIVERS[@]} driver(s) built successfully!${NC}"
else
    # Single-driver path: must appear in the migrated set
    found=false
    for driver in "${MIGRATED_DRIVERS[@]}"; do
        if [[ "$driver" == "$DRIVER_NAME" ]]; then
            found=true
            break
        fi
    done

    if [[ "$found" == false ]]; then
        if [[ ${#MIGRATED_DRIVERS[@]} -eq 0 ]]; then
            echo -e "${RED}Error: No migrated server drivers exist yet (#769 Phase 4 is${NC}" >&2
            echo -e "${RED}scaffolding-only; #774 will populate the migrated set).${NC}" >&2
        else
            echo -e "${RED}Error: Driver '$DRIVER_NAME' is not in the migrated set.${NC}" >&2
            echo -e "${YELLOW}Migrated drivers: ${MIGRATED_DRIVERS[*]}${NC}" >&2
        fi
        exit 1
    fi

    build_driver "$DRIVER_NAME"
fi
