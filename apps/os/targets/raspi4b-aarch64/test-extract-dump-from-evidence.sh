#!/usr/bin/env bash
# Smoke-test extraction of a dump snapshot from Pi 4 evidence.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"
EXTRACTOR="$TARGET_DIR/extract-dump-from-evidence.sh"
ANALYZER="$TARGET_DIR/analyze-dump.sh"

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/reovim-dump-evidence-test.XXXXXX")"
cleanup() {
    rm -rf "$tmp_root"
}
trap cleanup EXIT

expect_contains() {
    local haystack="$1"
    local needle="$2"
    local label="$3"

    if [[ "$haystack" != *"$needle"* ]]; then
        printf 'error: missing %s: %s\n' "$label" "$needle" >&2
        printf '%s\n' "$haystack" >&2
        exit 2
    fi
}

expect_failure() {
    local label="$1"
    shift
    local output

    if output="$("$@" 2>&1)"; then
        printf 'error: expected extractor failure for %s\n' "$label" >&2
        printf '%s\n' "$output" >&2
        exit 2
    fi
}

checksum32() {
    local path="$1"
    local hash=2166136261
    local byte
    local -a bytes

    while read -r -a bytes; do
        for byte in "${bytes[@]}"; do
            hash=$(( ((hash ^ byte) * 16777619) & 0xffffffff ))
        done
    done < <(od -An -v -t u1 "$path")

    printf '%u\n' "$hash"
}

write_dump() {
    local output="$1"
    local header="$tmp_root/header.$$"
    local checksum

    cat >"$header" <<'HEADER'
reovim-dump-v1
boot_id=7
session_id=11
identity_source=rootd-volatile
package=reovim-os
version=0.16.0-dev
target=aarch64-unknown-none
selected_profile=shell-only
profile_request=shell-only
bootline=absent
launch_profile_feature=disabled
boot_memory_ranges=1
boot_memory_usable_bytes=4096
boot_cpu_count=1
boot_heap_total_bytes=65536
device_records=1
proof_state=operator-required
panic_state=none
panic_records=0
persistent=unavailable
storage=none
storage_capacity_bytes=0
last_sync_attempted=false
last_sync_persistent=unavailable
last_sync_storage=none
last_sync_storage_capacity_bytes=0
last_sync_status=not-written
last_sync_bytes=0
last_sync_checksum=0
last_sync_verified=false
last_sync_reason=never-synced
klog_retained_bytes=2048
klog_dropped_bytes=0
klog_next_event_seq=42
event_records=17
process_records=5
service_records=1
exec_load_records=3
pending_exec_records=1
wait_records=2
task_records=5
syscall_records=12
syscall_continuation_records=0
HEADER
    checksum="$(checksum32 "$header")"
    {
        cat "$header"
        printf 'checksum=%s\n' "$checksum"
        printf 'boot:\n'
        printf 'memory_ranges=1\n'
        printf 'memory_usable_bytes=4096\n'
        printf 'cpu_count=1\n'
        printf 'heap_total_bytes=65536\n'
        printf 'devices:\n'
        printf -- '- index=0 class=uart compat=arm,pl011 mmio_base=0x1000 mmio_len=0x100 irq=32 capacity_bytes=0\n'
        printf 'proof:\n'
        printf 'state=operator-required\n'
        printf 'physical_usb_keyboard=required\n'
        printf 'zero_dropped_logs=required\n'
        printf 'dump_sync=unavailable\n'
        printf 'last_dump_sync_attempted=false\n'
        printf 'last_dump_sync_status=not-written\n'
        printf 'last_dump_sync_reason=never-synced\n'
        printf 'dump-sync:\n'
        printf 'attempted=false\n'
        printf 'persistent=unavailable\n'
        printf 'storage=none\n'
        printf 'storage_capacity_bytes=0\n'
        printf 'status=not-written\n'
        printf 'bytes=0\n'
        printf 'sync_checksum=0\n'
        printf 'verified=false\n'
        printf 'reason=never-synced\n'
        printf 'panic:\n'
        printf 'state=none\n'
        printf 'records=0\n'
        printf 'record=none\n'
        printf 'events:\n'
        printf 'event seq=1 component=rootd severity=info kind=boot boot=7 session=11 source=rootd pid=1 task=1\n'
        printf 'processes:\n'
        printf -- '- pid=1 ppid=0 task=1 state=running path=rootd exit=0 loader=kernel entry_fn=rootd_main\n'
        printf 'services:\n'
        printf -- '- seq=1 name=shell target=/bin/sh state=started reason=running owner_pid=3 owner_task=3 service_pid=2 service_task=2\n'
        printf 'execs:\n'
        printf -- '- seq=1 argv0=proc status=ok reason=loaded path=/bin/proc source=/bin/proc loader=linked-bin entry_fn=bin_proc truncated=false kind=bin origin=image-linked\n'
        printf 'pending:\n'
        printf -- '- pid=4 ppid=1 task=4 path=/bin/pwd loader=linked-bin entry_fn=bin_pwd kind=bin stdin_bytes=0\n'
        printf 'scheduler:\n'
        printf 'current_task=1\n'
        printf 'ready_queue_len=0\n'
        printf 'syscalls:\n'
        printf -- '- seq=1 pid=3 task=3 path=/bin/dump op=dump-status status=ok loader=source-image entry_fn=bin_dump\n'
        printf 'continuations:\n'
        printf 'tasks:\n'
        printf -- '- task=1 pid=1 parent_task=0 state=running entry=rootd\n'
        printf 'waits:\n'
    } >"$output"
    rm -f "$header"
}

dump="$tmp_root/dump.txt"
evidence="$tmp_root/evidence.md"
missing="$tmp_root/missing.md"
ambiguous="$tmp_root/ambiguous.md"
incomplete="$tmp_root/incomplete.md"
extracted="$tmp_root/extracted.txt"

write_dump "$dump"
{
    printf '# Evidence\n\n'
    printf '```text\n'
    printf 'reovim-os> dump snapshot\n'
    cat "$dump"
    printf 'reovim-os> dump sync\n'
    printf 'status=not-written\n'
    printf 'reason=no-persistent-dump-sink\n'
    printf '```\n'
} >"$evidence"
{
    printf '# Evidence\n\n'
    printf '```text\n'
    printf 'reovim-os> dump status\n'
    printf 'format=reovim-dump-v1\n'
    printf '```\n'
} >"$missing"
{
    printf '# Evidence\n\n'
    printf '```text\n'
    cat "$dump"
    cat "$dump"
    printf '```\n'
} >"$ambiguous"
{
    printf '# Evidence\n\n'
    printf '```text\n'
    printf 'reovim-dump-v1\n'
    printf 'boot_id=7\n'
    printf '```\n'
} >"$incomplete"

extract_output="$("$EXTRACTOR" --output "$extracted" "$evidence")"
expect_contains "$extract_output" "dump_extract=ok" "extract success marker"
expect_contains "$extract_output" "source=live-transcript" "live transcript source marker"
expect_contains "$extract_output" "persistent_sd_proof=false" "non-persistent marker"
expect_contains "$extract_output" "extracted_dump=$extracted" "output path marker"

cmp "$dump" "$extracted"

analysis="$("$ANALYZER" \
    --expect-package reovim-os \
    --expect-target aarch64-unknown-none \
    --expect-profile shell-only \
    --require-zero-drops \
    "$extracted")"
expect_contains "$analysis" "dump_analysis=ok" "analyzer success marker"
expect_contains "$analysis" "service_records=1" "service row count"

inline_analysis="$("$EXTRACTOR" --analyze "$evidence")"
expect_contains "$inline_analysis" "dump_extract=ok" "inline extract success marker"
expect_contains "$inline_analysis" "dump_analysis=ok" "inline analyzer success marker"

raw_output="$("$EXTRACTOR" "$evidence")"
if [ "$raw_output" != "$(cat "$dump")" ]; then
    printf 'error: raw extraction did not match source dump\n' >&2
    exit 2
fi

expect_failure "missing-dump" "$EXTRACTOR" "$missing"
expect_failure "ambiguous-dump" "$EXTRACTOR" "$ambiguous"
expect_failure "incomplete-dump" "$EXTRACTOR" "$incomplete"
expect_failure "existing-output" "$EXTRACTOR" --output "$extracted" "$evidence"

printf 'dump evidence extractor smoke ok\n'
