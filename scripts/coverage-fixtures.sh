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
# Scope: host (x86_64-linux) only. The cross-target lanes are exercised
# functionally (qemu-user for aarch64-linux, qemu-system for the bare-metal
# image) but not instrumented. A freestanding target cannot run this
# profiler runtime at all — it writes profraw through fd/file syscalls that
# do not exist there — and per-target sys/ backends are cfg-excluded from
# host builds, so the host MC/DC denominator never counts code the host
# cannot execute: the scope stays honest by construction, with no
# coverage(off) markers anywhere.
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

# ── Server-runtime fixture fleet (#797 Phase 3) ──────────────────────────────
#
# server-selftest: the full server-runtime test suite on the no_std runner
#   (incl. the Phase 3 Hello/Attach/SendInput UDS integration smoke).
# server-selftest inject-failure: the runner's fail-fast path (exit 70 by design).
SERVER_FIXTURES_DIR="$ROOT/server/lib/server/tests/fixtures"
SERVER_FIXTURES=(server-selftest)

echo "==> building instrumented server-runtime fixtures"
echo "    RUSTFLAGS=\"$COV_RUSTFLAGS\""
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$SERVER_FIXTURES_DIR/Cargo.toml" \
    "${SERVER_FIXTURES[@]/#/--package=}" \
    --message-format=json-render-diagnostics > "$OUT_DIR/server-build.json" 2> "$OUT_DIR/server-build.err"
if [ $? -ne 0 ]; then
    echo "server fixture build FAILED; see $OUT_DIR/server-build.err" >&2
    exit 1
fi

server_exe_for() {
    grep "\"$1\"" "$OUT_DIR/server-build.json" \
        | grep -o '"executable":"[^"]*"' | tail -1 \
        | sed 's/"executable":"//; s/"$//'
}

echo "==> running instrumented server fixtures, one profraw per exec"
si=0
for pkg in "${SERVER_FIXTURES[@]}"; do
    exe="$(server_exe_for "$pkg")"
    if [ -z "$exe" ]; then
        echo "no executable for $pkg" >&2
        exit 1
    fi
    prof="$OUT_DIR/${pkg}-${si}.profraw"
    echo "    $pkg -> $prof"
    LLVM_PROFILE_FILE="$prof" "$exe"
    echo "      exit=$?"
    si=$((si + 1))
done

echo "==> building + running the inject-failure server-selftest variant (#797 Phase 3)"
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$SERVER_FIXTURES_DIR/Cargo.toml" \
    --package=server-selftest --features inject-failure \
    --message-format=json-render-diagnostics > "$OUT_DIR/server-build-fail.json" 2> "$OUT_DIR/server-build-fail.err"
if [ $? -ne 0 ]; then
    echo "server inject-failure build FAILED; see $OUT_DIR/server-build-fail.err" >&2
    exit 1
fi
exe="$(grep '"server-selftest"' "$OUT_DIR/server-build-fail.json" \
    | grep -o '"executable":"[^"]*"' | tail -1 \
    | sed 's/"executable":"//; s/"$//')"
LLVM_PROFILE_FILE="$OUT_DIR/server-selftest-fail.profraw" "$exe"
echo "      exit=$? (fail-fast variant: 70 by design)"

# ── TUI platform fixture fleet (#797 Phase 5) ────────────────────────────────
#
# tui-selftest: the full TUI platform test suite (carrier + frame unit tests)
#   on the no_std runner.
# tui-selftest inject-failure: the runner's fail-fast path (exit 70 by design).
TUI_FIXTURES_DIR="$ROOT/ext/client/platforms/tui/tests/fixtures"
TUI_FIXTURES=(tui-selftest)

echo "==> building instrumented TUI platform fixtures"
echo "    RUSTFLAGS=\"$COV_RUSTFLAGS\""
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$TUI_FIXTURES_DIR/Cargo.toml" \
    "${TUI_FIXTURES[@]/#/--package=}" \
    --message-format=json-render-diagnostics > "$OUT_DIR/tui-build.json" 2> "$OUT_DIR/tui-build.err"
if [ $? -ne 0 ]; then
    echo "TUI fixture build FAILED; see $OUT_DIR/tui-build.err" >&2
    exit 1
fi

tui_exe_for() {
    grep "\"$1\"" "$OUT_DIR/tui-build.json" \
        | grep -o '"executable":"[^"]*"' | tail -1 \
        | sed 's/"executable":"//; s/"$//'
}

echo "==> running instrumented TUI platform fixtures, one profraw per exec"
ti=0
for pkg in "${TUI_FIXTURES[@]}"; do
    exe="$(tui_exe_for "$pkg")"
    if [ -z "$exe" ]; then
        echo "no executable for $pkg" >&2
        exit 1
    fi
    prof="$OUT_DIR/${pkg}-${ti}.profraw"
    echo "    $pkg -> $prof"
    LLVM_PROFILE_FILE="$prof" "$exe"
    echo "      exit=$?"
    ti=$((ti + 1))
done

echo "==> building + running the inject-failure tui-selftest variant (#797 Phase 5)"
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$TUI_FIXTURES_DIR/Cargo.toml" \
    --package=tui-selftest --features inject-failure \
    --message-format=json-render-diagnostics > "$OUT_DIR/tui-build-fail.json" 2> "$OUT_DIR/tui-build-fail.err"
if [ $? -ne 0 ]; then
    echo "tui inject-failure build FAILED; see $OUT_DIR/tui-build-fail.err" >&2
    exit 1
fi
exe="$(grep '"tui-selftest"' "$OUT_DIR/tui-build-fail.json" \
    | grep -o '"executable":"[^"]*"' | tail -1 \
    | sed 's/"executable":"//; s/"$//')"
LLVM_PROFILE_FILE="$OUT_DIR/tui-selftest-fail.profraw" "$exe"
echo "      exit=$? (fail-fast variant: 70 by design)"

# ── DEV2 E2E composed launcher (#797 Phase 5) ────────────────────────────────
#
# The `reovim` composition-root binary (apps/reovim) boots kernel + text
# Domain + UDS listener + TUI client in one process. Running it under coverage
# instruments the full in-process composition path: carrier, paint, frame
# composer, and the runtime paths not reachable by the unit/selftest fleet
# (notably `paint::run_loop` and `paint::paint_projection` in their real
# round-trip context).
#
# The binary reads stdin + exits on EOF, so we pipe in one byte 'x' + close.
# Socket path is PID-unique so parallel CI runs don't collide.
APPS_DIR="$ROOT/apps"
REOVIM_SOCK="/tmp/reovim-cov-e2e-$$.sock"

echo "==> building instrumented apps/reovim (DEV2 E2E coverage)"
echo "    RUSTFLAGS=\"$COV_RUSTFLAGS\""
RUSTFLAGS="$COV_RUSTFLAGS" cargo build \
    --manifest-path "$APPS_DIR/Cargo.toml" \
    --package=reovim \
    --message-format=json-render-diagnostics > "$OUT_DIR/apps-build.json" 2> "$OUT_DIR/apps-build.err"
if [ $? -ne 0 ]; then
    echo "apps/reovim build FAILED; see $OUT_DIR/apps-build.err" >&2
    exit 1
fi
reovim_exe="$(grep '"reovim"' "$OUT_DIR/apps-build.json" \
    | grep -o '"executable":"[^"]*"' | tail -1 \
    | sed 's/"executable":"//; s/"$//')"
if [ -z "$reovim_exe" ]; then
    echo "no executable for apps/reovim" >&2
    exit 1
fi

echo "==> running DEV2 E2E scenario under coverage"
echo "    exe=$reovim_exe  sock=$REOVIM_SOCK"
# Remove any stale socket.
rm -f "$REOVIM_SOCK"
prof="$OUT_DIR/reovim-e2e.profraw"
# Pipe one keystroke 'x' then EOF; the binary exits 0 after processing it.
# Note: LLVM_PROFILE_FILE must be exported to the binary's environment.
# The pipe feeds stdin; the binary reads 'x', sends it to the server,
# paints the resulting projection, then exits on EOF.
printf 'x' | LLVM_PROFILE_FILE="$prof" "$reovim_exe" "$REOVIM_SOCK"
echo "      exit=$?"
# Clean up socket in case the binary didn't (best-effort).
rm -f "$REOVIM_SOCK"

echo "==> profraw files in $OUT_DIR:"
ls -la "$OUT_DIR"/*.profraw 2>/dev/null || echo "  (none — profiler runtime did not emit; investigate)"

echo
echo "Next (main session): merge + attribute, e.g."
echo "  llvm-profdata merge -sparse $OUT_DIR/*.profraw -o $OUT_DIR/fixtures.profdata"
echo "  llvm-cov export --instr-profile $OUT_DIR/fixtures.profdata <fixture-bin> ..."
