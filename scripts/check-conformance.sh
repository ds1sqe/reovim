#!/usr/bin/env bash
# Bounded v2 diagnostics and exact Product evidence generation.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly CHECKER_TARGET=x86_64-unknown-linux-gnu

usage() {
    cat <<'EOF'
Usage:
  ./scripts/check-conformance.sh [candidate]
  ./scripts/check-conformance.sh exact EXPECTED_REVISION EVIDENCE_ROOT

`candidate` attempts a non-admitting diagnostic report under root `target/`.
Active Product requires `exact`, a clean revision, the committed explicit tool
environment, and an empty absolute evidence directory outside the repository.
Exact mode generates and verifies one content-addressed Source Role Map without
publishing it.
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
    if [[ "$path" != /* || ! -x "$path" ]]; then
        fail "$name did not resolve to an absolute executable"
    fi
    printf '%s\n' "$path"
}

held_cargo_available() {
    /proc/$$/fd/8 8<&- --version >/dev/null
}

reject_checker_build_overrides() {
    local name
    local -a exact=(
        CARGO_BUILD_RUSTC
        CARGO_BUILD_RUSTC_WRAPPER
        CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER
        CARGO_BUILD_RUSTFLAGS
        CARGO_BUILD_TARGET
        CARGO_BUILD_TARGET_DIR
        CARGO_ENCODED_RUSTFLAGS
        CARGO_INCREMENTAL
        RUSTC_WORKSPACE_WRAPPER
        RUSTC_WRAPPER
        RUSTFLAGS
    )

    for name in "${exact[@]}"; do
        if [[ -v "$name" ]]; then
            fail "CHECKER_BUILD_ENV_UNSUPPORTED: $name is set"
        fi
    done
    while IFS= read -r name; do
        case "$name" in
            CARGO_PROFILE_* | CARGO_TARGET_*)
                [[ "$name" == CARGO_TARGET_DIR ]] || \
                    fail "CHECKER_BUILD_ENV_UNSUPPORTED: $name is set"
                ;;
        esac
    done < <(compgen -e)
}

reject_checker_build_configs() {
    local path
    for path in \
        "$CARGO_HOME_PATH/config" \
        "$CARGO_HOME_PATH/config.toml" \
        /.cargo/config \
        /.cargo/config.toml; do
        if [[ -e "$path" ]]; then
            fail "CHECKER_BUILD_CONFIG_UNSUPPORTED: $path exists"
        fi
    done
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
if [[ "$GIT_BIN" != /* || ! -x "$GIT_BIN" ]]; then
    fail 'git did not resolve to an absolute executable'
fi

if [[ -n "${CARGO_HOME:-}" ]]; then
    CARGO_HOME_INPUT="$CARGO_HOME"
elif [[ -n "${HOME:-}" ]]; then
    CARGO_HOME_INPUT="$HOME/.cargo"
else
    fail 'CARGO_HOME is unset and no default Cargo home can be derived'
fi
if [[ ! -d "$CARGO_HOME_INPUT" ]]; then
    fail "Cargo home is not a directory: $CARGO_HOME_INPUT"
fi
CARGO_HOME_PATH="$(cd "$CARGO_HOME_INPUT" && pwd -P)"

reject_checker_build_overrides
reject_checker_build_configs
cd /

echo "==> build conformance checker"
if ! exec 8< "$CARGO_BIN"; then
    fail 'HELD_CARGO_UNAVAILABLE: held image could not be opened'
fi
if [[ ! -f /proc/self/fd/8 || ! -x /proc/self/fd/8 ]]; then
    fail 'HELD_CARGO_UNAVAILABLE: held image is not executable'
fi
if ! held_cargo_available; then
    fail 'HELD_CARGO_EXEC_UNAVAILABLE: parent procfd execution failed'
fi
CARGO_HOME="$CARGO_HOME_PATH" \
    CARGO_TARGET_DIR="$ROOT/target" \
    RUSTC="$RUSTC_BIN" \
    /proc/$$/fd/8 8<&- build --locked --offline \
    --manifest-path "$ROOT/Cargo.toml" \
    -p reovim-conformance --bin reovim-conformance \
    --target "$CHECKER_TARGET" --release
exec 8<&-
CHECKER_PATH="$ROOT/target/$CHECKER_TARGET/release/reovim-conformance"

hold_checker() {
    if ! exec 9< "$CHECKER_PATH"; then
        fail 'built checker could not be held open'
    fi
    if [[ ! -f /proc/self/fd/9 || ! -x /proc/self/fd/9 ]]; then
        fail 'HELD_CHECKER_UNAVAILABLE: held image is not executable'
    fi
    if ! run_checker --version >/dev/null; then
        fail 'HELD_CHECKER_EXEC_UNAVAILABLE: parent procfd execution failed'
    fi
}

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
    CARGO_HOME="$CARGO_HOME_PATH" \
        CARGO_TARGET_DIR="$ROOT/target" \
        RUSTC="$RUSTC_BIN" \
        /proc/$$/fd/9 9<&- "$@"
}

if [[ "$MODE" == candidate ]]; then
    REPORT_DIRECTORY="$ROOT/target/conformance"
    REPORT="$REPORT_DIRECTORY/candidate-source-role-map.json"
    mkdir -p "$REPORT_DIRECTORY"
    REPORT_TEMPLATE="$REPORT_DIRECTORY/.candidate-source-role-map.XXXXXX"
    REPORT_TEMP="$(mktemp "$REPORT_TEMPLATE")"
    cleanup_candidate_report() {
        if [[ -n "${REPORT_TEMP:-}" ]]; then
            rm -f -- "$REPORT_TEMP" 9<&-
        fi
    }
    trap cleanup_candidate_report EXIT
    echo "==> candidate Source Role Map"
    hold_checker
    set +e
    run_checker check "${COMMON[@]}" >"$REPORT_TEMP"
    status=$?
    set -e
    if ((status != 0 && status != 2)); then
        exec 9<&-
        printf '%s%s\n' \
            "check-conformance: candidate failed with status $status; " \
            'report was not replaced' >&2
        exit "$status"
    fi
    run_checker verify --report "$REPORT_TEMP"
    exec 9<&-
    mv -f -- "$REPORT_TEMP" "$REPORT"
    REPORT_TEMP=
    trap - EXIT
    if ((status == 2)); then
        printf '%s%s\n' \
            'check-conformance: candidate findings retained at ' \
            "$REPORT" >&2
        exit 2
    fi
    printf 'check-conformance: candidate report verified at %s\n' "$REPORT"
    exit 0
fi

EXPECTED_REVISION="$2"
EVIDENCE_ROOT_INPUT="$3"
EXECUTION_ENVIRONMENT=linux-x86-64-git-2-55-0
[[ "$EVIDENCE_ROOT_INPUT" = /* ]] || fail 'EVIDENCE_ROOT must be absolute'
if [[ ! -d "$EVIDENCE_ROOT_INPUT" ]]; then
    fail 'EVIDENCE_ROOT must already exist as a directory'
fi
EVIDENCE_ROOT="$(cd "$EVIDENCE_ROOT_INPUT" && pwd -P)"
[[ "$(stat -c '%a' "$EVIDENCE_ROOT")" == 700 ]] || \
    fail 'EVIDENCE_ROOT must have owner-only mode 0700'
if [[ "$EVIDENCE_ROOT" == "$ROOT" || "$EVIDENCE_ROOT" == "$ROOT/"* ]]; then
    fail 'EVIDENCE_ROOT must be outside the repository'
fi
mapfile -d '' -t existing < <(
    find "$EVIDENCE_ROOT" -mindepth 1 -maxdepth 1 -print0
)
((${#existing[@]} == 0)) || \
    fail "exact evidence root is not empty: $EVIDENCE_ROOT"

echo "==> exact Source Role Map"
hold_checker
run_checker generate "${COMMON[@]}" \
    --git "$GIT_BIN" \
    --expected-revision "$EXPECTED_REVISION" \
    --execution-environment "$EXECUTION_ENVIRONMENT" \
    --evidence-root "$EVIDENCE_ROOT"

mapfile -d '' -t reports < <(
    find "$EVIDENCE_ROOT" -type f \
        -path '*/conformance/source-role-map.json' -print0 9<&-
)
report_count="${#reports[@]}"
((${#reports[@]} == 1)) || \
    fail "expected exactly one generated Source Role Map, found $report_count"
run_checker verify --report "${reports[0]}"
exec 9<&-
printf '%s%s\n' \
    'check-conformance: exact report generated and verified at ' \
    "${reports[0]}"
