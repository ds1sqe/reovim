#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
unset NODE_OPTIONS NODE_PATH
node_pin="$(node -p "require('./apps/web/package.json').engines.node")"
ts_pin="$(node -p "require('./apps/web/package.json').config.typescript")"
if [[ "$(node --version)" != "v$node_pin" \
    || "$(tsc --version)" != "Version $ts_pin" ]]; then
    echo "web/check.sh: requires Node $node_pin and TypeScript $ts_pin" >&2
    exit 1
fi
echo "web/check.sh: Node $node_pin; TypeScript $ts_pin"
tsc -p apps/web/tsconfig.json
cp apps/web/package.json target/browser/package.json
node --test apps/web/tests.mjs apps/web/input-tests.mjs \
    apps/web/commands-tests.mjs
