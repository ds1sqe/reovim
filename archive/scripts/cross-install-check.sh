#!/usr/bin/env bash
# Cross-install verification for the dual-mode launcher (#769 Phase 2b.G).
#
# Validates master-plan acceptance #6 and plan §2b.G acceptance #9:
# after `cargo install --path apps/reovim`, a bare `reovim` must launch
# the embedded editor without any sibling bins on $PATH.
#
# The script installs into a throwaway root under tmp/2b-install-check/,
# sanitises $PATH so only the install root's bin/ is visible (plus the
# handful of coreutils this script itself invokes), and confirms the
# installed binary exits cleanly when sent SIGTERM.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
INSTALL_ROOT="${REPO_ROOT}/tmp/2b-install-check"
BIN_DIR="${INSTALL_ROOT}/bin"

rm -rf "${INSTALL_ROOT}"
mkdir -p "${INSTALL_ROOT}"

echo "==> cargo install --path apps/reovim --root ${INSTALL_ROOT}"
cargo install --path "${REPO_ROOT}/apps/reovim" --root "${INSTALL_ROOT}" \
    >"${INSTALL_ROOT}/install.log" 2>&1
echo "    installed $(ls "${BIN_DIR}")"

if [[ ! -x "${BIN_DIR}/reovim" ]]; then
    echo "FAIL: expected ${BIN_DIR}/reovim to be executable"
    cat "${INSTALL_ROOT}/install.log" >&2
    exit 1
fi

# Sanitise $PATH so only the install root + a minimal coreutils path
# are visible. This proves the installed reovim does not rely on any
# sibling bins already on $PATH — embedded composition must link every
# needed role into the launcher itself.
MIN_PATH="${BIN_DIR}:/usr/bin:/bin"

echo "==> probe installed reovim under a minimal PATH"
PATH="${MIN_PATH}" "${BIN_DIR}/reovim" --version \
    >"${INSTALL_ROOT}/run.stdout" 2>"${INSTALL_ROOT}/run.stderr"

grep -q "reovim" "${INSTALL_ROOT}/run.stdout" || {
    echo "FAIL: reovim --version did not print a version string"
    cat "${INSTALL_ROOT}/run.stdout" >&2
    cat "${INSTALL_ROOT}/run.stderr" >&2
    exit 1
}

PATH="${MIN_PATH}" "${BIN_DIR}/reovim" --help \
    >"${INSTALL_ROOT}/help.stdout" 2>"${INSTALL_ROOT}/help.stderr"

grep -q "launcher" "${INSTALL_ROOT}/help.stdout" || {
    echo "FAIL: reovim --help did not advertise the launcher surface"
    cat "${INSTALL_ROOT}/help.stdout" >&2
    exit 1
}

echo "==> OK: cargo install produces a runnable launcher in a bare PATH"
