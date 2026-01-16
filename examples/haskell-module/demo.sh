#!/bin/bash
# Fun demo: Haskell FFI Module for Reovim

set -e
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/../.."

MODULE_PATH="$SCRIPT_DIR/dist-newstyle/build/x86_64-linux/ghc-9.4.8/reovim-example-module-1.0.0/f/example-haskell/build/example-haskell/libexample-haskell.so.1.0.0"
REOVIM="./target/release/reovim"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${PURPLE}"
cat << 'EOF'
  _    _           _        _ _   _____ _____ ___
 | |  | |         | |      | | | |  ___|  ___|_ _|
 | |__| | __ _ ___| | _____| | | | |_  | |_   | |
 |  __  |/ _` / __| |/ / _ \ | | |  _| |  _|  | |
 | |  | | (_| \__ \   <  __/ | | | |   | |   _| |_
 |_|  |_|\__,_|___/_|\_\___|_|_| |_|   |_|  |_____|
EOF
echo -e "${NC}"
echo -e "${CYAN}     Haskell FFI Module Demo for Reovim${NC}"
echo ""

# Check if module is built
if [ ! -f "$MODULE_PATH" ]; then
    echo -e "${YELLOW}Building Haskell module...${NC}"
    cd "$SCRIPT_DIR"
    cabal build flib:example-haskell 2>&1 | tail -3
    cd "$SCRIPT_DIR/../.."
    echo ""
fi

# Check for running server
PORT=$($REOVIM cli list 2>/dev/null | grep -o '127.0.0.1:[0-9]*' | head -1 | cut -d: -f2 || true)

if [ -z "$PORT" ]; then
    echo -e "${YELLOW}Starting reovim server...${NC}"
    $REOVIM server --no-defaults 2>&1 &
    sleep 2
    PORT=$($REOVIM cli list 2>/dev/null | grep -o '127.0.0.1:[0-9]*' | head -1 | cut -d: -f2)
fi

echo -e "${GREEN}Server on port $PORT${NC}"
echo ""

# Fun message sequence via load/unload
echo -e "${BLUE}=== Load/Unload Animation ===${NC}"
echo ""

for word in "  Hello" "  Reovim!" "  I" "  am" "  a" "  Haskell" "  module!"; do
    $REOVIM cli --tcp 127.0.0.1:$PORT unload example-haskell >/dev/null 2>&1 || true
    $REOVIM cli --tcp 127.0.0.1:$PORT load "$MODULE_PATH" >/dev/null 2>&1
    echo -e "${GREEN}$word${NC}"
    sleep 0.25
done

echo ""

# Show final state
echo -e "${CYAN}Loaded modules:${NC}"
$REOVIM cli --tcp 127.0.0.1:$PORT modules 2>&1

echo ""
echo -e "${PURPLE}Haskell module loaded ${GREEN}7 times${PURPLE}!${NC}"
echo -e "${CYAN}Functional programming meets systems programming.${NC}"
