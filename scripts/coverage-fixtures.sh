#!/usr/bin/env bash
# Builds and runs the arch and kernel no_std fixture bins under coverage
# instrumentation, emitting profraw via the ARCH-OWNED profiler runtime
# (#785 Phase 5, extended for #796 Phase 5).
#
# This is the per-flight fixture-coverage step that scripts/coverage.sh merges.
# The fixtures are #![no_std] #![no_main] bins that cannot link compiler-rt's
# profiler runtime (38 libc symbols; the Phase 4 spike failed), so they link
# arch/src/profiler.rs instead:
#
#   * -C instrument-coverage          : emit the __llvm_prf_* sections + the
#                                       __llvm_profile_runtime reference.
#   * -Z no-profiler-runtime          : do NOT auto-link profiler_builtins;
#                                       arch's profiler.rs provides the runtime.
#   * --cfg arch_coverage             : compile arch's profiler module + the
#                                       exit-shim __llvm_profile_write_file call.
#
# LLVM_PROFILE_FILE is read at exit by arch's profiler runtime from the
# captured envp (start::env_block); set it per-exec so each fixture writes a
# distinct profraw the merge step consumes.
#
# NOTE: the MAIN SESSION runs this; subagents do not run build/coverage
# commands. This script is the executable spec of the exact build command.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES_DIR="$ROOT/arch/tests/fixtures"
KERNEL_FIXTURES_DIR="$ROOT/server/lib/kernel/tests/fixtures"
OUT_DIR="${OUT_DIR:-$ROOT/target/arch-fixture-coverage}"
mkdir -p "$OUT_DIR"

# The instrumented-build RUSTFLAGS. `-Z no-profiler-runtime` requires nightly.
# `-nostartfiles` is emitted by each fixture's build.rs, not here, because a
# build-script link arg survives this RUSTFLAGS override.
COV_RUSTFLAGS="-C instrument-coverage -Z no-profiler-runtime --cfg arch_coverage"

# The fixture fleet: arch-selftest carries the full migrated arch test
# suite; smoke is the charter boot fixture; the three panic fixtures cover
# the AB12 handler paths (they exit non-zero BY DESIGN; the panic handler
# flushes profraw before exit_group under arch_coverage). The
# inject-failure selftest variant covers the runner's fail-fast path.
# uapi-selftest covers the full uapi test suite (#786 Phase 5).
FIXTURES=(arch-fixture-smoke arch-selftest arch-fixture-panic-halt arch-fixture-panic-recover arch-fixture-panic-ab13 uapi-selftest)

echo "==> building instrumented fixtures"
echo "    RUSTFLAGS=\"$COV_RUSTFLAGS\""
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$FIXTURES_DIR/Cargo.toml" \
    "${FIXTURES[@]/#/--package=}" \
    --message-format=json-render-diagnostics > "$OUT_DIR/build.json" 2> "$OUT_DIR/build.err"
if [ $? -ne 0 ]; then
    echo "fixture build FAILED; see $OUT_DIR/build.err" >&2
    exit 1
fi

# Locate a built executable in the cargo JSON (no jq dependency).
exe_for() {
    grep "\"$1\"" "$OUT_DIR/build.json" \
        | grep -o '"executable":"[^"]*"' | tail -1 \
        | sed 's/"executable":"//; s/"$//'
}

echo "==> running instrumented fixtures, one profraw per exec"
i=0
for pkg in "${FIXTURES[@]}"; do
    exe="$(exe_for "$pkg")"
    if [ -z "$exe" ]; then
        echo "no executable for $pkg" >&2
        exit 1
    fi
    prof="$OUT_DIR/${pkg}-${i}.profraw"
    echo "    $pkg -> $prof"
    case "$pkg" in
        *panic*)
            # Panic fixtures flush their LOG2 line to a sink file (argv[1])
            # and exit 70/75 by design; the exit code is not a failure here.
            LLVM_PROFILE_FILE="$prof" "$exe" "$OUT_DIR/${pkg}.sink"
            echo "      exit=$? (panic fixture: non-zero by design)"
            ;;
        *)
            LLVM_PROFILE_FILE="$prof" "$exe"
            echo "      exit=$?"
            ;;
    esac
    i=$((i + 1))
done

echo "==> building + running the inject-failure selftest variant (arch)"
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$FIXTURES_DIR/Cargo.toml" \
    --package=arch-selftest --features inject-failure \
    --message-format=json-render-diagnostics > "$OUT_DIR/build-fail.json" 2> "$OUT_DIR/build-fail.err"
if [ $? -ne 0 ]; then
    echo "arch inject-failure build FAILED; see $OUT_DIR/build-fail.err" >&2
    exit 1
fi
exe="$(grep '"arch-selftest"' "$OUT_DIR/build-fail.json" \
    | grep -o '"executable":"[^"]*"' | tail -1 \
    | sed 's/"executable":"//; s/"$//')"
LLVM_PROFILE_FILE="$OUT_DIR/arch-selftest-fail.profraw" "$exe"
echo "      exit=$? (fail-fast variant: 70 by design)"

echo "==> building + running the inject-failure uapi-selftest variant (#786 Phase 5)"
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$FIXTURES_DIR/Cargo.toml" \
    --package=uapi-selftest --features inject-failure \
    --message-format=json-render-diagnostics > "$OUT_DIR/build-uapi-fail.json" 2> "$OUT_DIR/build-uapi-fail.err"
if [ $? -ne 0 ]; then
    echo "uapi inject-failure build FAILED; see $OUT_DIR/build-uapi-fail.err" >&2
    exit 1
fi
exe="$(grep '"uapi-selftest"' "$OUT_DIR/build-uapi-fail.json" \
    | grep -o '"executable":"[^"]*"' | tail -1 \
    | sed 's/"executable":"//; s/"$//')"
LLVM_PROFILE_FILE="$OUT_DIR/uapi-selftest-fail.profraw" "$exe"
echo "      exit=$? (fail-fast variant: 70 by design)"

# ── Kernel fixture fleet (#796 Phase 5) ──────────────────────────────────────
#
# kernel-selftest: the full kernel test suite on the no_std runner.
# kernel-panic-halt/recover: AB12 kernel panic-path fixtures, exit 70/75 by
#   design (panic handler exits after flush, same as arch panic fixtures).
# kernel-selftest inject-failure: the runner's fail-fast path (exit 70).
KERNEL_FIXTURES=(kernel-selftest kernel-panic-halt kernel-panic-recover)

echo "==> building instrumented kernel fixtures"
echo "    RUSTFLAGS=\"$COV_RUSTFLAGS\""
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$KERNEL_FIXTURES_DIR/Cargo.toml" \
    "${KERNEL_FIXTURES[@]/#/--package=}" \
    --message-format=json-render-diagnostics > "$OUT_DIR/kernel-build.json" 2> "$OUT_DIR/kernel-build.err"
if [ $? -ne 0 ]; then
    echo "kernel fixture build FAILED; see $OUT_DIR/kernel-build.err" >&2
    exit 1
fi

# Locate a built kernel executable in the cargo JSON.
kernel_exe_for() {
    grep "\"$1\"" "$OUT_DIR/kernel-build.json" \
        | grep -o '"executable":"[^"]*"' | tail -1 \
        | sed 's/"executable":"//; s/"$//'
}

echo "==> running instrumented kernel fixtures, one profraw per exec"
ki=0
for pkg in "${KERNEL_FIXTURES[@]}"; do
    exe="$(kernel_exe_for "$pkg")"
    if [ -z "$exe" ]; then
        echo "no executable for $pkg" >&2
        exit 1
    fi
    prof="$OUT_DIR/${pkg}-${ki}.profraw"
    echo "    $pkg -> $prof"
    case "$pkg" in
        *panic*)
            # Panic fixtures flush their LOG2 line to a sink file (argv[1])
            # and exit 70/75 by design; the exit code is not a failure here.
            LLVM_PROFILE_FILE="$prof" "$exe" "$OUT_DIR/${pkg}.sink"
            echo "      exit=$? (panic fixture: non-zero by design)"
            ;;
        *)
            LLVM_PROFILE_FILE="$prof" "$exe"
            echo "      exit=$?"
            ;;
    esac
    ki=$((ki + 1))
done

echo "==> building + running the inject-failure kernel-selftest variant (#796 Phase 5)"
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$KERNEL_FIXTURES_DIR/Cargo.toml" \
    --package=kernel-selftest --features inject-failure \
    --message-format=json-render-diagnostics > "$OUT_DIR/kernel-build-fail.json" 2> "$OUT_DIR/kernel-build-fail.err"
if [ $? -ne 0 ]; then
    echo "kernel inject-failure build FAILED; see $OUT_DIR/kernel-build-fail.err" >&2
    exit 1
fi
exe="$(grep '"kernel-selftest"' "$OUT_DIR/kernel-build-fail.json" \
    | grep -o '"executable":"[^"]*"' | tail -1 \
    | sed 's/"executable":"//; s/"$//')"
LLVM_PROFILE_FILE="$OUT_DIR/kernel-selftest-fail.profraw" "$exe"
echo "      exit=$? (fail-fast variant: 70 by design)"

echo "==> profraw files in $OUT_DIR:"
ls -la "$OUT_DIR"/*.profraw 2>/dev/null || echo "  (none — profiler runtime did not emit; investigate)"

echo
echo "Next (main session): merge + attribute, e.g."
echo "  llvm-profdata merge -sparse $OUT_DIR/*.profraw -o $OUT_DIR/fixtures.profdata"
echo "  llvm-cov export --instr-profile $OUT_DIR/fixtures.profdata <fixture-bin> ..."
