#!/usr/bin/env bash
set -euo pipefail

# ─── Configuration ───────────────────────────────────────────────────
# reovim-driver-display: v1 cell-grid rendering (client-side in v2)
EXCLUDE="--exclude reovim-driver-display"
TOOLCHAIN="+nightly"
CLIPPY_TARGET_DIR="target/check-clippy"
LOG_DIR="tmp/check-logs"
REPORT_FILE="tmp/check-report.md"

# ─── CLI ─────────────────────────────────────────────────────────────
MODE="parallel"
for arg in "$@"; do
    case "$arg" in
        --quick)       MODE="quick" ;;
        --sequential)  MODE="sequential" ;;
        --clean-cache)
            echo "Removing $CLIPPY_TARGET_DIR..."
            rm -rf "$CLIPPY_TARGET_DIR"
            echo "Done."
            exit 0
            ;;
        --help|-h)
            cat <<'USAGE'
Usage: ./scripts/check.sh [OPTIONS]

Options:
  (no args)        Full parallel check + report (default)
  --quick          Format + clippy only (fast dev iteration)
  --sequential     Sequential execution (for debugging / low-memory)
  --clean-cache    Remove target/check-clippy and exit
  --help, -h       Show this help

Output:
  tmp/check-report.md    Structured report with per-step timing
  tmp/check-logs/        Individual log files per step

The clippy job uses a separate CARGO_TARGET_DIR (target/check-clippy)
to avoid lock contention with the test job. First run builds a cold
cache (~3-5 min extra). Use --clean-cache to reclaim disk space.
USAGE
            exit 0
            ;;
        *)
            echo "Unknown option: $arg (try --help)" >&2
            exit 1
            ;;
    esac
done

# ─── Setup ───────────────────────────────────────────────────────────
mkdir -p "$LOG_DIR"

PIDS=()
cleanup() {
    for pid in "${PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

OVERALL_START=$(date +%s)

# Timing helpers
now_ms() { date +%s%N | cut -c1-13; }  # milliseconds since epoch
format_duration() {
    local ms="$1"
    local secs=$((ms / 1000))
    local frac=$((ms % 1000))
    printf "%d.%01ds" "$secs" "$((frac / 100))"
}

# Results array: "name status duration_ms"
declare -a RESULTS=()

# ─── Step runner (sequential) ────────────────────────────────────────
run_step() {
    local name="$1"; shift
    local logfile="$LOG_DIR/${name}.log"
    local start
    start=$(now_ms)
    printf "\033[1;33m==> %s...\033[0m\n" "$name"
    if "$@" > "$logfile" 2>&1; then
        local end
        end=$(now_ms)
        local elapsed=$((end - start))
        printf "\033[0;32m  %-12s PASS  %s\033[0m\n" "$name" "$(format_duration $elapsed)"
        RESULTS+=("$name PASS $elapsed")
        return 0
    else
        local code=$?
        local end
        end=$(now_ms)
        local elapsed=$((end - start))
        printf "\033[0;31m  %-12s FAIL  (exit %d)\033[0m\n" "$name" "$code"
        RESULTS+=("$name FAIL $elapsed")
        return "$code"
    fi
}

# ─── Step runner (background) ────────────────────────────────────────
run_bg() {
    local name="$1"; shift
    local logfile="$LOG_DIR/${name}.log"
    local start
    start=$(now_ms)
    (
        "$@" > "$logfile" 2>&1
    ) &
    local pid=$!
    PIDS+=("$pid")
    # Store metadata in temp files (bash doesn't share variables across subshells)
    echo "$pid $start" > "$LOG_DIR/.${name}.meta"
}

wait_bg() {
    local name="$1"
    local meta
    meta=$(cat "$LOG_DIR/.${name}.meta")
    local pid start
    pid=$(echo "$meta" | cut -d' ' -f1)
    start=$(echo "$meta" | cut -d' ' -f2)
    if wait "$pid" 2>/dev/null; then
        local end
        end=$(now_ms)
        local elapsed=$((end - start))
        printf "\033[0;32m  %-12s PASS  %s\033[0m\n" "$name" "$(format_duration $elapsed)"
        RESULTS+=("$name PASS $elapsed")
        return 0
    else
        local code=$?
        local end
        end=$(now_ms)
        local elapsed=$((end - start))
        printf "\033[0;31m  %-12s FAIL  (exit %d)\033[0m\n" "$name" "$code"
        RESULTS+=("$name FAIL $elapsed")
        return "$code"
    fi
}

# ─── Test summary parser ─────────────────────────────────────────────
parse_test_summary() {
    local logfile="$1"
    local passed=0 failed=0 ignored=0
    while IFS= read -r line; do
        local p f i
        p=$(echo "$line" | sed -n 's/.*\([0-9][0-9]*\) passed.*/\1/p')
        f=$(echo "$line" | sed -n 's/.*\([0-9][0-9]*\) failed.*/\1/p')
        i=$(echo "$line" | sed -n 's/.*\([0-9][0-9]*\) ignored.*/\1/p')
        passed=$((passed + ${p:-0}))
        failed=$((failed + ${f:-0}))
        ignored=$((ignored + ${i:-0}))
    done < <(grep "^test result:" "$logfile" 2>/dev/null || true)
    echo "$passed $failed $ignored"
}

# ─── Report generator ────────────────────────────────────────────────
generate_report() {
    local overall_end
    overall_end=$(date +%s)
    local wall=$((overall_end - OVERALL_START))
    local branch commit date_str
    branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
    commit=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
    date_str=$(date "+%Y-%m-%d %H:%M:%S")

    {
        echo "# Check Report"
        echo ""
        echo "Date: $date_str"
        echo "Branch: $branch"
        echo "Commit: $commit"
        echo ""
        echo "## Results"
        echo ""
        echo "| Step | Status | Duration |"
        echo "|------|--------|----------|"
        for entry in "${RESULTS[@]}"; do
            local name status ms
            name=$(echo "$entry" | cut -d' ' -f1)
            status=$(echo "$entry" | cut -d' ' -f2)
            ms=$(echo "$entry" | cut -d' ' -f3)
            printf "| %-12s | %-6s | %s |\n" "$name" "$status" "$(format_duration "$ms")"
        done
        echo ""
        printf "Total: %ds (wall-clock)\n" "$wall"

        # Test summary (only if test log exists)
        if [ -f "$LOG_DIR/test.log" ]; then
            local summary
            summary=$(parse_test_summary "$LOG_DIR/test.log")
            local p f i
            p=$(echo "$summary" | cut -d' ' -f1)
            f=$(echo "$summary" | cut -d' ' -f2)
            i=$(echo "$summary" | cut -d' ' -f3)
            echo ""
            echo "## Test Summary"
            echo "- $p passed, $f failed, $i ignored"
        fi

        # Clippy summary
        if [ -f "$LOG_DIR/clippy.log" ]; then
            local warns
            warns=$(grep -c "^warning\[" "$LOG_DIR/clippy.log" 2>/dev/null) || warns=0
            echo ""
            echo "## Clippy"
            echo "- $warns warnings"
        fi

        # Errors (show log tails on failure)
        local has_fail=false
        for entry in "${RESULTS[@]}"; do
            local status
            status=$(echo "$entry" | cut -d' ' -f2)
            if [ "$status" = "FAIL" ]; then
                has_fail=true
                break
            fi
        done
        echo ""
        echo "## Errors"
        if $has_fail; then
            for entry in "${RESULTS[@]}"; do
                local name status
                name=$(echo "$entry" | cut -d' ' -f1)
                status=$(echo "$entry" | cut -d' ' -f2)
                if [ "$status" = "FAIL" ] && [ -f "$LOG_DIR/${name}.log" ]; then
                    echo ""
                    echo "### $name"
                    echo '```'
                    tail -30 "$LOG_DIR/${name}.log"
                    echo '```'
                fi
            done
        else
            echo "(none)"
        fi
    } > "$REPORT_FILE"

    echo ""
    printf "\033[0;36mReport: %s\033[0m\n" "$REPORT_FILE"
}

# ─── Execution: Quick mode ───────────────────────────────────────────
if [ "$MODE" = "quick" ]; then
    echo -e "\033[1;36m==> Quick check (fmt + clippy)\033[0m"
    run_step fmt cargo $TOOLCHAIN fmt --all
    # shellcheck disable=SC2086
    run_step clippy cargo $TOOLCHAIN clippy \
        --all-targets --all-features --workspace $EXCLUDE -- -D warnings
    generate_report
    # Check for failures
    for entry in "${RESULTS[@]}"; do
        status=$(echo "$entry" | cut -d' ' -f2)
        [ "$status" = "FAIL" ] && exit 1
    done
    echo -e "\033[1;32m==> Quick check passed!\033[0m"
    exit 0
fi

# ─── Execution: Sequential mode ──────────────────────────────────────
if [ "$MODE" = "sequential" ]; then
    echo -e "\033[1;36m==> Sequential check\033[0m"
    run_step fmt cargo $TOOLCHAIN fmt --all
    # shellcheck disable=SC2086
    run_step clippy cargo $TOOLCHAIN clippy \
        --all-targets --all-features --workspace $EXCLUDE -- -D warnings
    # shellcheck disable=SC2086
    run_step build-grpc cargo $TOOLCHAIN build -p reovim-app --features grpc
    # shellcheck disable=SC2086
    run_step test cargo $TOOLCHAIN test --workspace $EXCLUDE
    generate_report
    for entry in "${RESULTS[@]}"; do
        status=$(echo "$entry" | cut -d' ' -f2)
        [ "$status" = "FAIL" ] && exit 1
    done
    echo -e "\033[1;32m==> All checks passed!\033[0m"
    exit 0
fi

# ─── Execution: Parallel mode (default) ──────────────────────────────
echo -e "\033[1;36m==> Parallel check\033[0m"

# Phase 1: Format (sequential, must complete before lint/test)
run_step fmt cargo $TOOLCHAIN fmt --all

# Phase 2: Clippy and tests in parallel
echo -e "\033[1;33m==> Launching parallel jobs...\033[0m"

# Job A: Clippy (separate target dir to avoid lock contention)
# shellcheck disable=SC2086
run_bg clippy env CARGO_TARGET_DIR="$CLIPPY_TARGET_DIR" \
    cargo $TOOLCHAIN clippy \
    --all-targets --all-features --workspace $EXCLUDE -- -D warnings

# Job B: Build gRPC binary + run tests (chained because tests need the binary)
# shellcheck disable=SC2086
run_bg test bash -c "cargo $TOOLCHAIN build -p reovim-app --features grpc && \
    cargo $TOOLCHAIN test --workspace $EXCLUDE"

# Wait for all parallel jobs
FAIL=0
printf "\033[1;33m==> Waiting for parallel jobs...\033[0m\n"
wait_bg clippy || FAIL=1
wait_bg test   || FAIL=1

generate_report

if [ "$FAIL" -ne 0 ]; then
    echo -e "\033[1;31m==> Some checks FAILED. See $REPORT_FILE and $LOG_DIR/\033[0m"
    exit 1
fi

echo -e "\033[1;32m==> All checks passed!\033[0m"
