#!/usr/bin/env bash
# The web side: the modules built once, the Node tests against a real
# server. Pinned to the Node and TypeScript in package.json.
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
rm -rf target/browser
tsc -p apps/web/tsconfig.json
cp apps/web/package.json target/browser/package.json
node --test sdk/typescript/wire-tests.mjs clients/gui/client-tests.mjs \
    apps/web/bridge-tests.mjs
