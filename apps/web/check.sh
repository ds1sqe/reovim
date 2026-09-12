#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
tsc -p apps/web/tsconfig.json
cp apps/web/package.json target/browser/package.json
node --test apps/web/tests.mjs
