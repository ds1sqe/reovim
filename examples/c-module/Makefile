# Makefile for example reovim C module
#
# Build:
#   make
#
# Clean:
#   make clean
#
# Note: This builds a shared library that can be loaded by reovim.
# The reovim_* service functions are resolved at load time.

# Compiler settings
CC ?= gcc
CFLAGS = -Wall -Wextra -Werror -fPIC -O2
LDFLAGS = -shared

# Include path for reovim.h
REOVIM_FFI_INCLUDE = ../../lib/drivers/ffi/include

# Source and output
SRC = example_module.c
OUT = example_module.so

# Default target
all: $(OUT)

$(OUT): $(SRC)
	$(CC) $(CFLAGS) -I$(REOVIM_FFI_INCLUDE) $(LDFLAGS) -o $@ $<

# Clean target
clean:
	rm -f $(OUT)

# Install target (example - adjust paths as needed)
install: $(OUT)
	@echo "To use this module, copy $(OUT) to your reovim modules directory"
	@echo "e.g., ~/.config/reovim/modules/"

# Check target - verify the shared object exports required symbols
check: $(OUT)
	@echo "Checking exported symbols..."
	@nm -D $(OUT) | grep -E "reovim_module_(probe|entry|init|exit|destroy)" || \
		(echo "ERROR: Missing required symbols" && exit 1)
	@nm -D $(OUT) | grep "REOVIM_MODULE_API_VERSION" || \
		(echo "ERROR: Missing REOVIM_MODULE_API_VERSION" && exit 1)
	@echo "All required symbols found!"

.PHONY: all clean install check
