#!/usr/bin/env bash
# Smoke-test the host dump analyzer with positive and negative fixtures.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"
ANALYZER="$TARGET_DIR/analyze-dump.sh"

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/reovim-dump-analyzer-test.XXXXXX")"
cleanup() {
    rm -rf "$tmp_root"
}
trap cleanup EXIT

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

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
        printf 'error: expected analyzer failure for %s\n' "$label" >&2
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
    local package="$2"
    local target="$3"
    local selected_profile="$4"
    local dropped_bytes="$5"
    local storage_mode="${6:-unavailable}"
    local continuation_count="${7:-0}"
    local header="$tmp_root/header.$$"
    local checksum
    local persistent="unavailable"
    local storage="none"
    local storage_capacity_bytes="0"
    local last_sync_attempted="false"
    local last_sync_persistent="unavailable"
    local last_sync_storage="none"
    local last_sync_storage_capacity_bytes="0"
    local last_sync_status="not-written"
    local last_sync_bytes="0"
    local last_sync_checksum="0"
    local last_sync_verified="false"
    local last_sync_reason="never-synced"
    local dump_sync="unavailable"

    if [ "$storage_mode" = "persistent" ]; then
        persistent="available"
        storage="qemu-diagnostic-dump0"
        storage_capacity_bytes="65536"
        last_sync_attempted="true"
        last_sync_persistent="available"
        last_sync_storage="qemu-diagnostic-dump0"
        last_sync_storage_capacity_bytes="65536"
        last_sync_status="written"
        last_sync_bytes="4096"
        last_sync_checksum="424242"
        last_sync_verified="true"
        last_sync_reason="written-readback-ok"
        dump_sync="available"
    fi

    cat >"$header" <<HEADER
reovim-dump-v1
boot_id=7
session_id=11
identity_source=rootd-volatile
package=$package
version=0.16.0-dev
target=$target
selected_profile=$selected_profile
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
persistent=$persistent
storage=$storage
storage_capacity_bytes=$storage_capacity_bytes
last_sync_attempted=$last_sync_attempted
last_sync_persistent=$last_sync_persistent
last_sync_storage=$last_sync_storage
last_sync_storage_capacity_bytes=$last_sync_storage_capacity_bytes
last_sync_status=$last_sync_status
last_sync_bytes=$last_sync_bytes
last_sync_checksum=$last_sync_checksum
last_sync_verified=$last_sync_verified
last_sync_reason=$last_sync_reason
klog_retained_bytes=2048
klog_dropped_bytes=$dropped_bytes
klog_next_event_seq=42
event_records=17
process_records=5
service_records=1
exec_load_records=3
pending_exec_records=1
wait_records=2
task_records=5
syscall_records=12
syscall_continuation_records=$continuation_count
HEADER
    checksum="$(checksum32 "$header")"
    {
        printf 'serial transcript prefix ignored by analyzer\n'
        printf 'dump:\n'
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
        printf 'dump_sync=%s\n' "$dump_sync"
        printf 'last_dump_sync_attempted=%s\n' "$last_sync_attempted"
        printf 'last_dump_sync_status=%s\n' "$last_sync_status"
        printf 'last_dump_sync_reason=%s\n' "$last_sync_reason"
        printf 'dump-sync:\n'
        printf 'attempted=%s\n' "$last_sync_attempted"
        printf 'persistent=%s\n' "$last_sync_persistent"
        printf 'storage=%s\n' "$last_sync_storage"
        printf 'storage_capacity_bytes=%s\n' "$last_sync_storage_capacity_bytes"
        printf 'status=%s\n' "$last_sync_status"
        printf 'bytes=%s\n' "$last_sync_bytes"
        printf 'sync_checksum=%s\n' "$last_sync_checksum"
        printf 'verified=%s\n' "$last_sync_verified"
        printf 'reason=%s\n' "$last_sync_reason"
        printf 'panic:\n'
        printf 'state=none\n'
        printf 'records=0\n'
        printf 'record=none\n'
        printf 'events:\n'
        printf -- '- seq=1 boot=7 session=11 source=rootd component=rootd severity=info kind=boot pid=1 task=1\n'
        if [ "$continuation_count" -gt 0 ]; then
            printf -- '- seq=2 boot=7 session=11 source=process component=syscall severity=info kind=syscall-continue-blocked pid=5 task=5\n'
        fi
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
        if [ "$continuation_count" -gt 0 ]; then
            printf -- '- pid=5 task=5 path=/bin/cat nr=1 op=read memory=raw-read-buffer a0=3 a1=4096 a2=4 a3=0 a4=0 a5=0 loader=linked-bin entry_fn=bin_cat\n'
        fi
        printf 'tasks:\n'
        printf -- '- task=1 pid=1 parent_task=0 state=running entry=rootd\n'
        printf 'waits:\n'
    } >"$output"
    rm -f "$header"
}

pass_dump="$tmp_root/pass.dump"
persistent_dump="$tmp_root/persistent.dump"
active_continuation_dump="$tmp_root/active-continuation.dump"
missing_continuation_row_dump="$tmp_root/missing-continuation-row.dump"
bad_checksum_dump="$tmp_root/bad-checksum.dump"
dropped_dump="$tmp_root/dropped.dump"
missing_section_dump="$tmp_root/missing-section.dump"
missing_continuations_dump="$tmp_root/missing-continuations.dump"
missing_exec_source_dump="$tmp_root/missing-exec-source.dump"
missing_continuation_event_dump="$tmp_root/missing-continuation-event.dump"
bootfs_dir="$tmp_root/bootfs"
empty_bootfs_dir="$tmp_root/empty-bootfs"
ambiguous_bootfs_dir="$tmp_root/ambiguous-bootfs"

write_dump "$pass_dump" reovim-os aarch64-unknown-none shell-only 0
write_dump "$persistent_dump" reovim-os aarch64-unknown-none shell-only 0 persistent
write_dump "$active_continuation_dump" reovim-os aarch64-unknown-none shell-only 0 unavailable 1
write_dump "$dropped_dump" reovim-os aarch64-unknown-none shell-only 3
cp "$pass_dump" "$bad_checksum_dump"
sed -i 's/^boot_id=7$/boot_id=8/' "$bad_checksum_dump"
grep -v '^proof:$' "$pass_dump" >"$missing_section_dump"
grep -v '^continuations:$' "$pass_dump" >"$missing_continuations_dump"
grep -v '^- pid=5 task=5 path=/bin/cat nr=1 op=read memory=raw-read-buffer ' "$active_continuation_dump" >"$missing_continuation_row_dump"
grep -v 'kind=syscall-continue-blocked' "$active_continuation_dump" >"$missing_continuation_event_dump"
sed 's/ source=\/bin\/proc//' "$pass_dump" >"$missing_exec_source_dump"
mkdir -p "$bootfs_dir" "$empty_bootfs_dir" "$ambiguous_bootfs_dir/reovim"
cp "$pass_dump" "$bootfs_dir/reovim-dump-v1.txt"
cp "$pass_dump" "$ambiguous_bootfs_dir/reovim-dump.txt"
cp "$pass_dump" "$ambiguous_bootfs_dir/reovim/dump.txt"

analysis="$("$ANALYZER" \
    --expect-package reovim-os \
    --expect-target aarch64-unknown-none \
    --expect-profile shell-only \
    --require-zero-drops \
    "$pass_dump")"

expect_contains "$analysis" "dump_analysis=ok" "success marker"
expect_contains "$analysis" "machine_result=live-dump-pass" "live machine result"
expect_contains "$analysis" "machine_result_persistent=false" "non-persistent machine result marker"
expect_contains "$analysis" "format=reovim-dump-v1" "format row"
expect_contains "$analysis" "package=reovim-os" "package row"
expect_contains "$analysis" "target=aarch64-unknown-none" "target row"
expect_contains "$analysis" "selected_profile=shell-only" "profile row"
expect_contains "$analysis" "device_records=1" "device row count"
expect_contains "$analysis" "service_records=1" "service row count"
expect_contains "$analysis" "exec_load_records=3" "exec load row count"
expect_contains "$analysis" "pending_exec_records=1" "pending exec row count"
expect_contains "$analysis" "syscall_continuation_records=0" "continuation row count"
expect_contains "$analysis" "proof_state=operator-required" "proof state row"
expect_contains "$analysis" "panic_state=none" "panic state row"
expect_contains "$analysis" "storage_capacity_bytes=0" "storage capacity row"
expect_contains "$analysis" "persistent_dump_proof=false" "non-persistent proof row"
expect_contains "$analysis" "last_sync_attempted=false" "last sync attempted row"
expect_contains "$analysis" "last_sync_reason=never-synced" "last sync reason row"
expect_contains "$analysis" "klog_dropped_bytes=0" "zero-drop row"

directory_analysis="$("$ANALYZER" \
    --expect-package reovim-os \
    --expect-target aarch64-unknown-none \
    --expect-profile shell-only \
    --require-zero-drops \
    "$bootfs_dir")"
expect_contains "$directory_analysis" "dump_analysis=ok" "directory success marker"
expect_contains "$directory_analysis" "dump_file=$bootfs_dir/reovim-dump-v1.txt" "resolved bootfs dump path"

persistent_analysis="$("$ANALYZER" \
    --expect-package reovim-os \
    --expect-target aarch64-unknown-none \
    --expect-profile shell-only \
    --require-zero-drops \
    --require-persistent-proof \
    "$persistent_dump")"
expect_contains "$persistent_analysis" "persistent_dump_proof=true" "persistent proof marker"
expect_contains "$persistent_analysis" "machine_result=post-poweroff-dump-pass" "persistent machine result"
expect_contains "$persistent_analysis" "machine_result_persistent=true" "persistent machine result marker"
expect_contains "$persistent_analysis" "last_sync_status=written" "persistent last sync status"

active_continuation_analysis="$("$ANALYZER" \
    --expect-package reovim-os \
    --expect-target aarch64-unknown-none \
    --expect-profile shell-only \
    --require-zero-drops \
    "$active_continuation_dump")"
expect_contains "$active_continuation_analysis" "syscall_continuation_records=1" "active continuation row count"

expect_failure "checksum-mismatch" "$ANALYZER" "$bad_checksum_dump"
expect_failure "wrong-package" "$ANALYZER" --expect-package other-os "$pass_dump"
expect_failure "wrong-target" "$ANALYZER" --expect-target x86_64-unknown-none "$pass_dump"
expect_failure "missing-section" "$ANALYZER" "$missing_section_dump"
expect_failure "missing-continuations" "$ANALYZER" "$missing_continuations_dump"
expect_failure "missing-continuation-row" "$ANALYZER" "$missing_continuation_row_dump"
expect_failure "missing-continuation-event" "$ANALYZER" "$missing_continuation_event_dump"
expect_failure "missing-exec-source" "$ANALYZER" "$missing_exec_source_dump"
expect_failure "dropped-log" "$ANALYZER" --require-zero-drops "$dropped_dump"
expect_failure "empty-bootfs-directory" "$ANALYZER" "$empty_bootfs_dir"
expect_failure "ambiguous-bootfs-directory" "$ANALYZER" "$ambiguous_bootfs_dir"
expect_failure "persistent-proof-unavailable" "$ANALYZER" --require-persistent-proof "$pass_dump"

dropped_analysis="$("$ANALYZER" "$dropped_dump")"
expect_contains "$dropped_analysis" "klog_dropped_bytes=3" "dropped-log report without strict mode"

printf 'dump analyzer smoke ok\n'
