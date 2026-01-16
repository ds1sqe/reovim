# Makefile for reovim Haskell example module
#
# Targets:
#   make        - Build the shared library
#   make test   - Run FFI safety tests
#   make check  - Verify exported symbols
#   make clean  - Remove build artifacts
#   make help   - Show this help

.PHONY: all build test check clean help install

# Default target
all: build

# Build the shared library using cabal
build:
	@echo "Building Haskell module..."
	cabal build all
	@echo ""
	@echo "Build complete. Library location:"
	@find dist-newstyle -name "libexample-haskell*.so" -o \
	      -name "libexample-haskell*.dylib" -o \
	      -name "example-haskell*.dll" 2>/dev/null | head -1 || \
	    echo "  (library will be in dist-newstyle/)"

# Run FFI safety tests
test: build
	@echo "Running FFI safety tests..."
	cabal run test-ffi-safety

# Verify exported symbols match requirements
check: build
	@echo "Checking exported symbols..."
	@LIB=$$(find dist-newstyle -name "libexample-haskell*.so" -o \
	        -name "libexample-haskell*.dylib" 2>/dev/null | head -1); \
	if [ -z "$$LIB" ]; then \
	    echo "ERROR: Shared library not found. Run 'make build' first."; \
	    exit 1; \
	fi; \
	echo "Library: $$LIB"; \
	echo ""; \
	echo "Required symbols:"; \
	echo "-----------------"; \
	MISSING=0; \
	for sym in reovim_module_probe reovim_module_entry reovim_module_init \
	           reovim_module_exit reovim_module_destroy; do \
	    if nm -D "$$LIB" 2>/dev/null | grep -q "$$sym" || \
	       nm "$$LIB" 2>/dev/null | grep -q "$$sym"; then \
	        echo "[OK] $$sym"; \
	    else \
	        echo "[MISSING] $$sym"; \
	        MISSING=1; \
	    fi; \
	done; \
	echo ""; \
	echo "Optional symbols:"; \
	echo "-----------------"; \
	for sym in reovim_module_supports_hot_reload reovim_module_save_state \
	           reovim_module_restore_state reovim_module_free_state \
	           reovim_module_api_version_ptr; do \
	    if nm -D "$$LIB" 2>/dev/null | grep -q "$$sym" || \
	       nm "$$LIB" 2>/dev/null | grep -q "$$sym"; then \
	        echo "[OK] $$sym"; \
	    else \
	        echo "[--] $$sym (not exported)"; \
	    fi; \
	done; \
	echo ""; \
	if [ $$MISSING -eq 1 ]; then \
	    echo "ERROR: Missing required symbols!"; \
	    exit 1; \
	fi; \
	echo "All required symbols found!"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cabal clean
	rm -rf dist-newstyle

# Install to user's modules directory (example)
install: build check
	@echo "To install, copy the shared library to your reovim modules directory:"
	@LIB=$$(find dist-newstyle -name "libexample-haskell*.so" -o \
	        -name "libexample-haskell*.dylib" 2>/dev/null | head -1); \
	echo "  cp $$LIB ~/.config/reovim/modules/"

# Show help
help:
	@echo "Reovim Haskell Module - Build Targets"
	@echo ""
	@echo "  make          Build the shared library"
	@echo "  make test     Run FFI safety tests"
	@echo "  make check    Verify exported symbols"
	@echo "  make clean    Remove build artifacts"
	@echo "  make install  Show install instructions"
	@echo "  make help     Show this help"
	@echo ""
	@echo "Prerequisites:"
	@echo "  - GHC 9.4+ with cabal-install"
	@echo "  - reovim with FFI support (#290)"
