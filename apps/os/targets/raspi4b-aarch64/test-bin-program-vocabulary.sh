#!/usr/bin/env bash
# Guard the root-shell executable surface: operator input resolves to /bin programs.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

flight_log_current_status=""
expected_bins=(
    help
    clear
    screentest
    pwd
    ls
    cd
    cat
    read
    mount
    input
    status
    proof
    device
    dmesg
    dump
    sched
    proc
    probe
    launch
    reovim
    halt
)

files=(
    "$ROOT/apps/os/bins/"*.rvs
    "$ROOT/system/lib/kernel/src/program.rs"
    "$ROOT/system/lib/kernel/src/program_tests.rs"
    "$ROOT/system/lib/kernel/src/root_shell.rs"
    "$ROOT/system/lib/kernel/src/root_shell_tests.rs"
    "$ROOT/system/lib/kernel/src/bin_fixture.rs"
    "$ROOT/system/lib/kernel/src/rootd.rs"
    "$ROOT/system/lib/kernel/src/proc.rs"
    "$ROOT/system/lib/kernel/src/proc_tests.rs"
    "$ROOT/system/lib/kernel/src/sched.rs"
    "$ROOT/system/lib/kernel/src/source_store.rs"
    "$ROOT/system/lib/kernel/src/source_store_tests.rs"
    "$ROOT/system/lib/kernel/src/syscall.rs"
    "$ROOT/system/lib/kernel/src/vfs.rs"
    "$ROOT/system/lib/kernel/src/vfs_tests.rs"
    "$ROOT/apps/os/src/bin.rs"
    "$ROOT/apps/os/src/boot.rs"
    "$ROOT/arch/tests/fixtures_exec.rs"
    "$ROOT/Documentation/01-Architecture/05-Configuration.md"
    "$ROOT/Documentation/01-Architecture/06-OS-Modes.md"
    "$ROOT/Documentation/02-Process/05-Machine-Boot.md"
    "$ROOT/Documentation/07-Surfaces/05-Log-CLI.md"
    "$ROOT/Documentation/future/tty-pty.md"
    "$ROOT/apps/os/targets/raspi4b-aarch64/README.md"
    "$ROOT/apps/os/targets/raspi4b-aarch64/evidence-template.md"
    "$ROOT/apps/os/targets/raspi4b-aarch64/validate-evidence.sh"
    "$ROOT/apps/os/targets/raspi4b-aarch64/test-validate-evidence.sh"
    "$ROOT/apps/os/targets/raspi4b-aarch64/test-bin-program-vocabulary.sh"
)

for optional in \
    "$ROOT/tmp/commit.sh" \
    "$ROOT/tmp/mission.md" \
    "$HOME/docs/plans/reovim/#800-system-kernel/11-process-syscall-foundation.md" \
    "$HOME/docs/plans/reovim/#800-system-kernel/12-persistent-dump-logging.md"
do
    if [ -f "$optional" ]; then
        files+=("$optional")
    fi
done

if [ -f "$HOME/docs/plans/reovim/#800-system-kernel/flight-log.md" ]; then
    flight_log_current_status="$TMP_DIR/flight-log-current-status.md"
    awk '
        /^## Current Status/ { emit = 1 }
        emit { print }
        emit && /^---$/ { exit }
    ' "$HOME/docs/plans/reovim/#800-system-kernel/flight-log.md" > "$flight_log_current_status"
    files+=("$flight_log_current_status")
fi

stale_re='[bB]uilt[- ]?[iI]n|[bB]uiltin[s]?|entry_fn=cm''d|\bcm''d_[A-Za-z0-9_]*|load_shell''_program|spawn_shell''_program|exec_from''_shell|run_com''mand_line|run_single_com''mand_line|append_shell_com''mand_log|in''-shell executable|in''-shell program|shell''-program|shell'' program'

stale_vocab_files=()
for file in "${files[@]}"; do
    if [ "$file" != "$ROOT/apps/os/targets/raspi4b-aarch64/test-bin-program-vocabulary.sh" ]; then
        stale_vocab_files+=("$file")
    fi
done

if rg -n "$stale_re" "${stale_vocab_files[@]}"; then
    printf 'error: stale executable vocabulary found; operator input must resolve only to /bin programs\n' >&2
    exit 1
fi

stale_loader_re='loader=static-image entry_fn=bin_[A-Za-z0-9_]+'
if rg -n "$stale_loader_re" "${files[@]}"; then
    printf 'error: stale /bin loader metadata found; image /bin programs must report source-image\n' >&2
    exit 1
fi

active_model_files=(
    "$ROOT/apps/os/bins/"*.rvs
    "$ROOT/system/lib/kernel/src/program.rs"
    "$ROOT/system/lib/kernel/src/program_tests.rs"
    "$ROOT/system/lib/kernel/src/root_shell.rs"
    "$ROOT/system/lib/kernel/src/root_shell_tests.rs"
    "$ROOT/system/lib/kernel/src/bin_fixture.rs"
    "$ROOT/system/lib/kernel/src/rootd.rs"
    "$ROOT/system/lib/kernel/src/exec_tests.rs"
    "$ROOT/system/lib/kernel/src/exec.rs"
    "$ROOT/system/lib/kernel/src/source_store.rs"
    "$ROOT/system/lib/kernel/src/source_store_tests.rs"
    "$ROOT/system/lib/kernel/src/syscall.rs"
    "$ROOT/system/lib/kernel/src/vfs.rs"
    "$ROOT/apps/os/src/bin.rs"
    "$ROOT/apps/os/src/boot.rs"
    "$ROOT/Documentation/01-Architecture/05-Configuration.md"
    "$ROOT/Documentation/01-Architecture/06-OS-Modes.md"
    "$ROOT/Documentation/02-Process/05-Machine-Boot.md"
    "$ROOT/Documentation/07-Surfaces/05-Log-CLI.md"
    "$ROOT/Documentation/future/tty-pty.md"
    "$ROOT/apps/os/targets/raspi4b-aarch64/README.md"
    "$ROOT/apps/os/targets/raspi4b-aarch64/evidence-template.md"
    "$ROOT/apps/os/targets/raspi4b-aarch64/validate-evidence.sh"
    "$ROOT/apps/os/targets/raspi4b-aarch64/test-validate-evidence.sh"
    "$ROOT/apps/os/targets/raspi4b-aarch64/test-bin-program-vocabulary.sh"
)

for optional in \
    "$ROOT/tmp/commit.sh" \
    "$ROOT/tmp/mission.md" \
    "$HOME/docs/plans/reovim/#800-system-kernel/11-process-syscall-foundation.md" \
    "$HOME/docs/plans/reovim/#800-system-kernel/12-persistent-dump-logging.md"
do
    if [ -f "$optional" ]; then
        active_model_files+=("$optional")
    fi
done

if [ -n "$flight_log_current_status" ]; then
    active_model_files+=("$flight_log_current_status")
fi

ambiguous_exec_re='shell''-private|shell''-owned|private she''ll|she''ll-local|kernel-internal com''mand|root-shell com''mand catalog|com''mand catalog|com''mand implementation table|com''mand bod(y|ies)|special-com''mand executable'
if rg -n "$ambiguous_exec_re" "${active_model_files[@]}"; then
    printf 'error: ambiguous executable vocabulary found; every operator executable must be a /bin program\n' >&2
    exit 1
fi

if rg -n 'commands:' \
    "$ROOT/apps/os/src/bin.rs" \
    "$ROOT/system/lib/kernel/src/bin_fixture.rs" \
    "$ROOT/apps/os/targets/raspi4b-aarch64/validate-evidence.sh" \
    "$ROOT/apps/os/targets/raspi4b-aarch64/test-validate-evidence.sh"
then
    printf 'error: proof executable inventory must be labeled /bin programs, not commands\n' >&2
    exit 1
fi

bare_programs_re='b"programs'': help|"programs'': help|stdout_bytes\(b"programs'': "\)'
if rg -n "$bare_programs_re" \
    "$ROOT/apps/os/src/bin.rs" \
    "$ROOT/system/lib/kernel/src/bin_fixture.rs" \
    "$ROOT/system/lib/kernel/src/root_shell_tests.rs" \
    "$ROOT/arch/tests/fixtures_exec.rs" \
    "$ROOT/apps/os/targets/raspi4b-aarch64/validate-evidence.sh" \
    "$ROOT/apps/os/targets/raspi4b-aarch64/test-validate-evidence.sh"
then
    printf 'error: help executable inventory must be labeled /bin programs, not bare programs\n' >&2
    exit 1
fi

direct_shell_re='execute_root_program_line|argv0_for_line'
if rg -n "$direct_shell_re" \
    "$ROOT/system/lib/kernel/src/root_shell.rs" \
    "$ROOT/system/lib/kernel/src/root_shell_tests.rs" \
    "$ROOT/system/lib/kernel/src/rootd.rs"
then
    printf 'error: direct root-shell executable path found; /bin programs must enter through exec/syscall\n' >&2
    exit 1
fi

for bin_name in "${expected_bins[@]}"; do
    source_file="$ROOT/apps/os/bins/$bin_name.rvs"
    if [ ! -f "$source_file" ]; then
        printf 'error: operator program %s is missing its /bin source artifact %s\n' "$bin_name" "$source_file" >&2
        exit 1
    fi

    if ! rg -q "\"/bin/$bin_name\"" "$ROOT/apps/os/src/bin.rs"; then
        printf 'error: operator program %s is missing from the reovim-os /bin catalog\n' "$bin_name" >&2
        exit 1
    fi

    if ! rg -q "source_bytes_bin_artifact\\(\"/bin/$bin_name\"" "$ROOT/apps/os/src/bin.rs"; then
        printf 'error: operator program %s is missing from the reovim-os /bin source store\n' "$bin_name" >&2
        exit 1
    fi

    if ! rg -q "\"program=$bin_name" "$ROOT/arch/tests/fixtures_exec.rs"; then
        printf 'error: operator program %s has no /bin descriptor proof in the x86 transcript\n' "$bin_name" >&2
        exit 1
    fi

    if ! rg -q "path=/bin/$bin_name" "$ROOT/arch/tests/fixtures_exec.rs"; then
        printf 'error: operator program %s has no /bin path proof in the x86 transcript\n' "$bin_name" >&2
        exit 1
    fi
done

system_product_bin_re='fn bin_|BIN_PROGRAMS|\bbin_program\(|BIN_HELP|show command help|print current kernel VFS|request root daemon shutdown'
if rg -n "$system_product_bin_re" \
    "$ROOT/system/lib/kernel/src/program.rs" \
    "$ROOT/system/lib/kernel/src/root_shell.rs" \
    "$ROOT/system/lib/kernel/src/rootd.rs" \
    "$ROOT/system/lib/kernel/src/exec.rs" \
    "$ROOT/system/lib/kernel/src/syscall.rs" \
    "$ROOT/system/lib/kernel/src/vfs.rs"
then
    printf 'error: concrete image program bodies found in system-kernel product code; image bins must live outside system/lib/kernel\n' >&2
    exit 1
fi

opcode_source_re='ProgramSource(Image|Op|Arg1Case)|ProgramImage::Source\(|ProgramImage::SourceBytes|ProgramImage::Static|ProgramEntry|StaticImage'
if rg -n "$opcode_source_re" \
    "$ROOT/system/lib/kernel/src/program.rs" \
    "$ROOT/system/lib/kernel/src/program_tests.rs" \
    "$ROOT/system/lib/kernel/src/bin_fixture.rs" \
    "$ROOT/apps/os/src/bin.rs"
then
    printf 'error: stale /bin image API found; /bin descriptors must point at source-store artifacts\n' >&2
    exit 1
fi

payload_callback_image_re='PayloadSourceImage|PayloadImage::SourceBytes|PayloadImage::CompositionRootCallback|PayloadImageKind::CompositionRootCallback|composition-root-callback'
if rg -n "$payload_callback_image_re" \
    "$ROOT/system/lib/kernel/src/rootd.rs" \
    "$ROOT/system/lib/kernel/src/exec.rs" \
    "$ROOT/system/lib/kernel/src/exec_tests.rs" \
    "$ROOT/system/lib/kernel/src/root_shell_tests.rs" \
    "$ROOT/apps/os/src/boot.rs"
then
    printf 'error: stale payload image API found; payload descriptors must point at source-store artifacts\n' >&2
    exit 1
fi

if ! rg -q 'pub const BIN_PROGRAMS' "$ROOT/apps/os/src/bin.rs"; then
    printf 'error: reovim-os image does not expose a /bin program catalog\n' >&2
    exit 1
fi

printf 'bin program vocabulary smoke ok\n'
