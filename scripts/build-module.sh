#!/usr/bin/env bash
# Build reovim modules as shared libraries for dynamic loading.
#
# Usage:
#   ./scripts/build-module.sh vim           # Build single module (release)
#   ./scripts/build-module.sh --all         # Build all modules (release)
#   ./scripts/build-module.sh vim --debug   # Build single module (debug)
#   ./scripts/build-module.sh vim --install # Build and install to XDG data dir
#   ./scripts/build-module.sh --all --install # Build all and install
#   ./scripts/build-module.sh --install-config # Install example config only
#
# Options:
#   --all            Build all modules with cdylib support
#   --debug          Build in debug mode (default: release)
#   --install        Install to ~/.local/share/reovim/modules/
#   --install-config Install example config to ~/.config/reovim/ (auto with --install)
#   --verify         Verify FFI symbols after build (default: on)
#   --no-verify      Skip FFI symbol verification
#   --client         Verify client-module exports (verify-only mode only)
#   --no-build       Skip cargo build (requires --verify-only)
#   --verify-only    Verify an already-built library path and exit
#
# Output:
#   Release: target/release/libreovim_module_<name>.so (Linux)
#            target/release/libreovim_module_<name>.dylib (macOS)
#            target/release/reovim_module_<name>.dll (Windows)
#
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

# Default options
BUILD_MODE="release"
INSTALL=false
INSTALL_CONFIG=false
INSTALL_CONFIG_ONLY=false
VERIFY=true
BUILD_ALL=false
CLIENT=false
NO_BUILD=false
VERIFY_ONLY_PATH=""
MODULE_NAME=""

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
        --install)
            INSTALL=true
            INSTALL_CONFIG=true
            shift
            ;;
        --install-config)
            INSTALL_CONFIG=true
            INSTALL_CONFIG_ONLY=true
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
        --client)
            CLIENT=true
            shift
            ;;
        --no-build)
            NO_BUILD=true
            shift
            ;;
        --verify-only)
            VERIFY_ONLY_PATH="$2"
            shift 2
            ;;
        -h|--help)
            head -30 "$0" | tail -28
            exit 0
            ;;
        -*)
            echo -e "${RED}Error: Unknown option $1${NC}" >&2
            exit 1
            ;;
        *)
            if [[ -z "$MODULE_NAME" ]]; then
                MODULE_NAME="$1"
            else
                echo -e "${RED}Error: Multiple module names specified${NC}" >&2
                exit 1
            fi
            shift
            ;;
    esac
done

# Validate arguments
if [[ "$CLIENT" == true && -z "$VERIFY_ONLY_PATH" ]]; then
    echo -e "${RED}Error: --client is only supported with --no-build --verify-only <path>${NC}" >&2
    echo -e "${YELLOW}Hint: build client modules with cargo, then verify the resulting .so artifact${NC}" >&2
    exit 1
fi

if [[ "$NO_BUILD" == true && -z "$VERIFY_ONLY_PATH" ]]; then
    echo -e "${RED}Error: --no-build requires --verify-only <path>${NC}" >&2
    exit 1
fi

if [[ -n "$VERIFY_ONLY_PATH" && "$NO_BUILD" == false ]]; then
    echo -e "${RED}Error: --verify-only requires --no-build${NC}" >&2
    exit 1
fi

if [[ "$INSTALL_CONFIG_ONLY" == false && "$BUILD_ALL" == false && -z "$MODULE_NAME" && -z "$VERIFY_ONLY_PATH" ]]; then
    echo -e "${RED}Error: Specify a module name or --all${NC}" >&2
    echo "Usage: $0 <module-name> [--debug] [--install]"
    echo "       $0 --all [--debug] [--install]"
    echo "       $0 --no-build --verify-only <path> [--client]"
    echo "       $0 --install-config"
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

# Get list of modules with cdylib support
get_cdylib_modules() {
    local modules=()
    for cargo_toml in "$REPO_ROOT"/server/modules/*/Cargo.toml; do
        if grep -q 'cdylib' "$cargo_toml" 2>/dev/null; then
            local dir_name=$(dirname "$cargo_toml")
            local module_name=$(basename "$dir_name")
            modules+=("$module_name")
        fi
    done
    echo "${modules[@]}"
}

# Get cargo package name for a module
get_package_name() {
    local module=$1
    local cargo_toml="$REPO_ROOT/server/modules/$module/Cargo.toml"

    if [[ ! -f "$cargo_toml" ]]; then
        echo -e "${RED}Error: Module '$module' not found${NC}" >&2
        return 1
    fi

    # Extract package name from Cargo.toml
    grep -m1 '^name = ' "$cargo_toml" | sed 's/name = "\([^"]*\)"/\1/'
}

# Convert package name to library name (replace - with _)
get_lib_name() {
    local package_name=$1
    echo "$package_name" | tr '-' '_'
}

# Verify FFI symbols in the built library
verify_ffi_symbols() {
    local lib_path=$1
    local module=$2

    if [[ ! -f "$lib_path" ]]; then
        echo -e "${RED}Error: Library not found: $lib_path${NC}" >&2
        return 1
    fi

    echo -e "${BLUE}  Verifying FFI symbols...${NC}"

    local symbol_dump
    symbol_dump="$(nm -gD "$lib_path" 2>/dev/null || nm "$lib_path" 2>/dev/null || true)"

    # Expected FFI symbols
    local expected_symbols=(
        "REOVIM_MODULE_API_VERSION"
        "reovim_module_probe"
        "reovim_module_entry"
        "reovim_module_init"
        "reovim_module_exit"
        "reovim_module_destroy"
        "reovim_module_supports_hot_reload"
        "reovim_module_save_state"
        "reovim_module_restore_state"
        "reovim_module_free_state"
    )

    local missing=0
    for sym in "${expected_symbols[@]}"; do
        if ! grep -Fq "$sym" <<< "$symbol_dump"; then
            echo -e "${RED}    Missing symbol: $sym${NC}" >&2
            missing=$((missing + 1))
        fi
    done

    if [[ $missing -gt 0 ]]; then
        echo -e "${RED}  Error: Missing $missing FFI symbol(s)${NC}" >&2
        echo -e "${YELLOW}  Hint: Ensure the module uses declare_module!() macro${NC}" >&2
        return 1
    fi

    echo -e "${GREEN}  All 10 FFI symbols verified${NC}"
    return 0
}

# Verify client FFI exports in a built library
verify_client_ffi_symbols() {
    local lib_path=$1

    if [[ ! -f "$lib_path" ]]; then
        echo -e "${RED}Error: Library not found: $lib_path${NC}" >&2
        return 1
    fi

    echo -e "${BLUE}  Verifying client FFI exports...${NC}"

    local symbol_dump
    symbol_dump="$(nm -gD "$lib_path" 2>/dev/null || nm "$lib_path" 2>/dev/null || true)"

    local expected_symbols=(
        "REOVIM_CLIENT_MODULE_API_VERSION"
        "reovim_client_module_probe"
        "reovim_client_module_entry"
        "reovim_client_module_init"
        "reovim_client_module_exit"
        "reovim_client_module_destroy"
        "reovim_client_module_on_all_loaded"
        "reovim_client_module_on_notification"
        "reovim_client_module_on_mode_change"
        "reovim_client_module_on_cursor_update"
        "reovim_client_module_on_buffer_focus"
        "reovim_client_module_on_buffer_update"
        "reovim_client_module_on_option_changed"
        "reovim_client_module_tick"
        "reovim_client_module_has_chrome"
        "reovim_client_module_has_buffer_contrib"
        "reovim_client_module_has_annotations"
        "reovim_client_module_chrome_position"
        "reovim_client_module_chrome_requested_size"
        "reovim_client_module_chrome_priority"
        "reovim_client_module_chrome_z_order"
        "reovim_client_module_buffer_contrib_priority"
        "reovim_client_module_annotation_priority"
        "reovim_client_module_chrome_render"
        "reovim_client_module_annotate"
        "reovim_client_module_annotation_column_width"
        "reovim_client_module_transform_line"
        "reovim_client_module_free_transformed_line"
        "reovim_client_module_map_cursor_column"
        "reovim_client_module_fold_ranges"
        "reovim_client_module_virtual_lines"
        "reovim_client_module_inline_decorations"
        "reovim_client_module_cursor_position"
        "reovim_client_module_classify_token"
        "reovim_client_module_on_capabilities_changed"
        "reovim_client_module_on_theme_changed"
    )

    local missing=0
    for sym in "${expected_symbols[@]}"; do
        if ! grep -Fq "$sym" <<< "$symbol_dump"; then
            echo -e "${RED}    Missing symbol: $sym${NC}" >&2
            missing=$((missing + 1))
        fi
    done

    if [[ $missing -gt 0 ]]; then
        echo -e "${RED}  Error: Missing $missing client FFI export(s)${NC}" >&2
        echo -e "${YELLOW}  Hint: Ensure the module uses declare_client_module!() macro${NC}" >&2
        return 1
    fi

    echo -e "${GREEN}  All 36 client exports verified${NC}"
    return 0
}

verify_symbols_for_mode() {
    local lib_path=$1
    local module=${2:-artifact}

    if [[ "$CLIENT" == true ]]; then
        verify_client_ffi_symbols "$lib_path"
    else
        verify_ffi_symbols "$lib_path" "$module"
    fi
}

# Install module to XDG data directory
install_module() {
    local lib_path=$1
    local module=$2

    # Determine install directory
    local install_dir="${XDG_DATA_HOME:-$HOME/.local/share}/reovim/modules"

    echo -e "${BLUE}  Installing to $install_dir/...${NC}"

    mkdir -p "$install_dir"
    cp "$lib_path" "$install_dir/"

    echo -e "${GREEN}  Installed: $(basename "$lib_path")${NC}"
}

# Install example config to XDG config directory
install_config() {
    local config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/reovim"
    local config_file="$config_dir/config.toml"
    local example_file="$REPO_ROOT/config/config.toml.example"

    if [[ ! -f "$example_file" ]]; then
        echo -e "${RED}Error: Example config not found: $example_file${NC}" >&2
        return 1
    fi

    mkdir -p "$config_dir"

    if [[ -f "$config_file" ]]; then
        echo -e "${YELLOW}  Config already exists: $config_file${NC}"
        echo -e "${BLUE}  Skipping config installation (won't overwrite)${NC}"
    else
        cp "$example_file" "$config_file"
        echo -e "${GREEN}  Installed config: $config_file${NC}"
    fi
}

# Build a single module
build_module() {
    local module=$1

    echo -e "${YELLOW}==> Building module: $module${NC}"

    # Check if module exists
    local cargo_toml="$REPO_ROOT/server/modules/$module/Cargo.toml"
    if [[ ! -f "$cargo_toml" ]]; then
        echo -e "${RED}Error: Module '$module' not found at server/modules/$module/${NC}" >&2
        return 1
    fi

    # Check if module has cdylib
    if ! grep -q 'cdylib' "$cargo_toml"; then
        echo -e "${RED}Error: Module '$module' does not have cdylib in Cargo.toml${NC}" >&2
        echo -e "${YELLOW}Hint: Add 'crate-type = [\"cdylib\", \"rlib\"]' to [lib] section${NC}" >&2
        return 1
    fi

    local package_name
    package_name=$(get_package_name "$module") || return 1
    local lib_name
    lib_name=$(get_lib_name "$package_name")

    # Build the module with dynamic feature enabled for FFI symbols
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

    # Verify FFI symbols
    if [[ "$VERIFY" == true ]]; then
        verify_symbols_for_mode "$lib_path" "$module" || return 1
    fi

    # Install if requested
    if [[ "$INSTALL" == true ]]; then
        install_module "$lib_path" "$module"
    fi

    return 0
}

# Main execution
cd "$REPO_ROOT"

# Handle --install-config only mode
if [[ "$INSTALL_CONFIG_ONLY" == true ]]; then
    echo -e "${YELLOW}==> Installing config${NC}"
    install_config
    exit 0
fi

# Handle verify-only mode
if [[ -n "$VERIFY_ONLY_PATH" ]]; then
    echo -e "${YELLOW}==> Verifying artifact: $VERIFY_ONLY_PATH${NC}"
    verify_symbols_for_mode "$VERIFY_ONLY_PATH" || exit 1
    exit 0
fi

if [[ "$BUILD_ALL" == true ]]; then
    echo -e "${YELLOW}==> Building all modules with cdylib support${NC}"

    modules=($(get_cdylib_modules))

    if [[ ${#modules[@]} -eq 0 ]]; then
        echo -e "${RED}Error: No modules with cdylib support found${NC}" >&2
        exit 1
    fi

    echo -e "${BLUE}Found ${#modules[@]} module(s): ${modules[*]}${NC}"

    failed=0
    for module in "${modules[@]}"; do
        if ! build_module "$module"; then
            failed=$((failed + 1))
        fi
        echo ""
    done

    if [[ $failed -gt 0 ]]; then
        echo -e "${RED}==> $failed module(s) failed to build${NC}"
        exit 1
    fi

    echo -e "${GREEN}==> All ${#modules[@]} module(s) built successfully!${NC}"

    # Install config if requested
    if [[ "$INSTALL_CONFIG" == true ]]; then
        echo ""
        echo -e "${YELLOW}==> Installing config${NC}"
        install_config
    fi
else
    build_module "$MODULE_NAME"

    # Install config if requested (for single module install)
    if [[ "$INSTALL_CONFIG" == true && "$INSTALL" == true ]]; then
        echo ""
        echo -e "${YELLOW}==> Installing config${NC}"
        install_config
    fi
fi
