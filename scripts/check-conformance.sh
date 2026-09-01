#!/usr/bin/env bash
# Bounded Product-empty v2 repository check and exact evidence generation.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"

usage() {
    cat <<'EOF'
Usage:
  ./scripts/check-conformance.sh [candidate]
  ./scripts/check-conformance.sh exact EXPECTED_REVISION EVIDENCE_ROOT

`candidate` checks the current tree and retains its diagnostic report under
root `target/`. `exact` requires a clean exact revision, the committed explicit
execution-tool environment, and an empty absolute evidence directory outside
the repository; it generates and verifies one content-addressed Source Role
Map without publishing it.
EOF
}

fail() {
    printf 'check-conformance: %s\n' "$1" >&2
    exit 64
}

real_rust_tool() {
    local name="$1"
    local path
    if command -v rustup >/dev/null 2>&1; then
        path="$(rustup which "$name")"
    else
        path="$(command -v "$name")"
        path="$(readlink -f "$path")"
    fi
    [[ "$path" = /* && -x "$path" ]] || fail "$name did not resolve to an absolute executable"
    printf '%s\n' "$path"
}

MODE="${1:-candidate}"
case "$MODE" in
    candidate)
        (($# <= 1)) || fail 'candidate accepts no additional arguments'
        ;;
    exact)
        (($# == 3)) || fail 'exact requires EXPECTED_REVISION and EVIDENCE_ROOT'
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        usage >&2
        fail "unknown mode: $MODE"
        ;;
esac

CARGO_BIN="$(real_rust_tool cargo)"
RUSTC_BIN="$(real_rust_tool rustc)"
GIT_BIN="$(readlink -f "$(command -v git)")"
[[ "$GIT_BIN" = /* && -x "$GIT_BIN" ]] || fail 'git did not resolve to an absolute executable'

if [[ -n "${CARGO_HOME:-}" ]]; then
    CARGO_HOME_INPUT="$CARGO_HOME"
elif [[ -n "${HOME:-}" ]]; then
    CARGO_HOME_INPUT="$HOME/.cargo"
else
    fail 'CARGO_HOME is unset and no default Cargo home can be derived'
fi
[[ -d "$CARGO_HOME_INPUT" ]] || fail "Cargo home is not a directory: $CARGO_HOME_INPUT"
CARGO_HOME_PATH="$(cd "$CARGO_HOME_INPUT" && pwd -P)"

cd "$ROOT"

echo "==> build conformance checker"
CARGO_HOME="$CARGO_HOME_PATH" CARGO_TARGET_DIR="$ROOT/target" RUSTC="$RUSTC_BIN" \
    cargo build --locked --offline -p reovim-conformance \
    --bin reovim-conformance

COMMON=(
    --root "$ROOT"
    --policy "$ROOT/conformance/source-classification.json"
    --topology "$ROOT/conformance/workspace-topology.json"
    --entrypoint-reviews "$ROOT/conformance/entrypoint-reviews.json"
    --dependency-reviews "$ROOT/conformance/dependency-reviews.json"
    --product-closures "$ROOT/conformance/product-closures.json"
    --execution-tool-reviews "$ROOT/conformance/execution-tool-reviews.json"
    --cargo "$CARGO_BIN"
    --rustc "$RUSTC_BIN"
    --cargo-home "$CARGO_HOME_PATH"
)

run_checker() {
    CARGO_HOME="$CARGO_HOME_PATH" CARGO_TARGET_DIR="$ROOT/target" RUSTC="$RUSTC_BIN" \
        cargo run --quiet --locked --offline -p reovim-conformance \
        --bin reovim-conformance -- "$@"
}

if [[ "$MODE" == candidate ]]; then
    REPORT_DIRECTORY="$ROOT/target/conformance"
    REPORT="$REPORT_DIRECTORY/candidate-source-role-map.json"
    mkdir -p "$REPORT_DIRECTORY"
    REPORT_TEMP="$(mktemp "$REPORT_DIRECTORY/.candidate-source-role-map.XXXXXX")"
    cleanup_candidate_report() {
        if [[ -n "${REPORT_TEMP:-}" ]]; then
            rm -f -- "$REPORT_TEMP"
        fi
    }
    trap cleanup_candidate_report EXIT
    echo "==> candidate Source Role Map"
    set +e
    run_checker check "${COMMON[@]}" >"$REPORT_TEMP"
    status=$?
    set -e
    if ((status != 0 && status != 2)); then
        printf 'check-conformance: candidate failed with status %d; report was not replaced\n' \
            "$status" >&2
        exit "$status"
    fi
    run_checker verify --report "$REPORT_TEMP"
    mv -f -- "$REPORT_TEMP" "$REPORT"
    REPORT_TEMP=
    trap - EXIT
    if ((status == 2)); then
        printf 'check-conformance: candidate findings retained at %s\n' "$REPORT" >&2
        exit 2
    fi
    printf 'check-conformance: candidate report verified at %s\n' "$REPORT"
    exit 0
fi

EXPECTED_REVISION="$2"
EVIDENCE_ROOT_INPUT="$3"
EXECUTION_ENVIRONMENT=linux-x86-64-git-2-55-0
[[ "$EVIDENCE_ROOT_INPUT" = /* ]] || fail 'EVIDENCE_ROOT must be absolute'
[[ -d "$EVIDENCE_ROOT_INPUT" ]] || fail 'EVIDENCE_ROOT must already exist as a directory'
EVIDENCE_ROOT="$(cd "$EVIDENCE_ROOT_INPUT" && pwd -P)"
[[ "$(stat -c '%a' "$EVIDENCE_ROOT")" == 700 ]] || \
    fail 'EVIDENCE_ROOT must have owner-only mode 0700'
if [[ "$EVIDENCE_ROOT" == "$ROOT" || "$EVIDENCE_ROOT" == "$ROOT/"* ]]; then
    fail 'EVIDENCE_ROOT must be outside the repository'
fi
mapfile -d '' -t existing < <(find "$EVIDENCE_ROOT" -mindepth 1 -maxdepth 1 -print0)
((${#existing[@]} == 0)) || fail "exact evidence root is not empty: $EVIDENCE_ROOT"

echo "==> exact Source Role Map"
run_checker generate "${COMMON[@]}" \
    --git "$GIT_BIN" \
    --expected-revision "$EXPECTED_REVISION" \
    --execution-environment "$EXECUTION_ENVIRONMENT" \
    --evidence-root "$EVIDENCE_ROOT"

mapfile -d '' -t reports < <(
    find "$EVIDENCE_ROOT" -type f -path '*/conformance/source-role-map.json' -print0
)
((${#reports[@]} == 1)) || fail "expected exactly one generated Source Role Map, found ${#reports[@]}"
run_checker verify --report "${reports[0]}"
printf 'check-conformance: exact report generated and verified at %s\n' "${reports[0]}"
