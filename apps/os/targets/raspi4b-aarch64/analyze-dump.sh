#!/usr/bin/env bash
# Analyze a saved Reovim OS dump snapshot from mounted media or a copied file.

set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: apps/os/targets/raspi4b-aarch64/analyze-dump.sh [options] <dump-file-or-directory>

Options:
  --expect-package NAME   Require package=NAME.
  --expect-target TRIPLE  Require target=TRIPLE.
  --expect-profile NAME   Require selected_profile=NAME.
  --require-zero-drops    Reject dumps with klog_dropped_bytes != 0.
  --require-persistent-proof
                         Reject dumps that do not report available storage and
                         a retained checked dump-sync result.
  -h, --help              Show this help.

The input may be the raw `dump snapshot` transcript, a file whose first dump
header starts at a `reovim-dump-v1` line, or a mounted bootfs directory
containing exactly one supported dump artifact:
  reovim-dump-v1.txt
  reovim-dump.txt
  reovim/dump-v1.txt
  reovim/dump.txt
USAGE
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

expect_package=""
expect_target=""
expect_profile=""
require_zero_drops=0
require_persistent_proof=0
input_path=""
dump_file=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --expect-package)
            [ "$#" -ge 2 ] || fail "--expect-package needs a value"
            expect_package="$2"
            shift 2
            ;;
        --expect-target)
            [ "$#" -ge 2 ] || fail "--expect-target needs a value"
            expect_target="$2"
            shift 2
            ;;
        --expect-profile)
            [ "$#" -ge 2 ] || fail "--expect-profile needs a value"
            expect_profile="$2"
            shift 2
            ;;
        --require-zero-drops)
            require_zero_drops=1
            shift
            ;;
        --require-persistent-proof)
            require_persistent_proof=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --*)
            fail "unknown option: $1"
            ;;
        *)
            if [ -n "$input_path" ]; then
                fail "multiple dump inputs provided"
            fi
            input_path="$1"
            shift
            ;;
    esac
done

[ -n "$input_path" ] || {
    usage >&2
    exit 2
}

resolve_dump_input() {
    local input="$1"
    local found=""
    local count=0
    local candidate
    local -a candidates

    if [ -f "$input" ]; then
        [ -s "$input" ] || fail "dump file is empty: $input"
        dump_file="$input"
        return
    fi

    if [ ! -d "$input" ]; then
        fail "dump input does not exist: $input"
    fi

    candidates=(
        "$input/reovim-dump-v1.txt"
        "$input/reovim-dump.txt"
        "$input/reovim/dump-v1.txt"
        "$input/reovim/dump.txt"
    )
    for candidate in "${candidates[@]}"; do
        if [ -f "$candidate" ] && [ -s "$candidate" ]; then
            found="$candidate"
            count=$((count + 1))
        fi
    done

    if [ "$count" -eq 0 ]; then
        fail "no dump artifact found under directory: $input"
    fi
    if [ "$count" -gt 1 ]; then
        fail "multiple dump artifacts found under directory: $input; pass the exact dump file"
    fi

    dump_file="$found"
}

resolve_dump_input "$input_path"

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/reovim-dump-analyze.XXXXXX")"
cleanup() {
    rm -rf "$tmp_dir"
}
trap cleanup EXIT

header_file="$tmp_dir/header.txt"
checksum_payload="$tmp_dir/checksum-payload.txt"

awk '
    $0 == "reovim-dump-v1" { in_header = 1 }
    in_header {
        print
        if ($0 ~ /^checksum=[0-9]+$/) {
            found = 1
            exit
        }
    }
    END { exit found ? 0 : 1 }
' "$dump_file" >"$header_file" || fail "no complete reovim-dump-v1 header found"

first_line="$(sed -n '1p' "$header_file")"
[ "$first_line" = "reovim-dump-v1" ] || fail "unsupported dump format: $first_line"

checksum_line_count="$(grep -Ec '^checksum=[0-9]+$' "$header_file" || true)"
[ "$checksum_line_count" -eq 1 ] || fail "dump header must contain exactly one checksum row"

sed '/^checksum=/,$d' "$header_file" >"$checksum_payload"

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

value_for() {
    local key="$1"
    awk -F= -v key="$key" '
        $1 == key {
            print substr($0, index($0, "=") + 1)
            found = 1
            exit
        }
        END { exit found ? 0 : 1 }
    ' "$header_file"
}

require_number() {
    local key="$1"
    local value="$2"
    [[ "$value" =~ ^[0-9]+$ ]] || fail "$key is not numeric: $value"
}

require_text() {
    local key="$1"
    local value="$2"
    [ -n "$value" ] || fail "$key is empty"
}

require_section() {
    local section="$1"
    grep -Eq "^${section}:$" "$dump_file" || fail "missing ${section}: section"
}

format="$first_line"
boot_id="$(value_for boot_id)" || fail "missing boot_id"
session_id="$(value_for session_id)" || fail "missing session_id"
identity_source="$(value_for identity_source)" || fail "missing identity_source"
package="$(value_for package)" || fail "missing package"
version="$(value_for version)" || fail "missing version"
target="$(value_for target)" || fail "missing target"
selected_profile="$(value_for selected_profile)" || fail "missing selected_profile"
profile_request="$(value_for profile_request)" || fail "missing profile_request"
bootline="$(value_for bootline)" || fail "missing bootline"
launch_profile_feature="$(value_for launch_profile_feature)" || fail "missing launch_profile_feature"
boot_memory_ranges="$(value_for boot_memory_ranges)" || fail "missing boot_memory_ranges"
boot_memory_usable_bytes="$(value_for boot_memory_usable_bytes)" || fail "missing boot_memory_usable_bytes"
boot_cpu_count="$(value_for boot_cpu_count)" || fail "missing boot_cpu_count"
boot_heap_total_bytes="$(value_for boot_heap_total_bytes)" || fail "missing boot_heap_total_bytes"
device_records="$(value_for device_records)" || fail "missing device_records"
proof_state="$(value_for proof_state)" || fail "missing proof_state"
panic_state="$(value_for panic_state)" || fail "missing panic_state"
panic_records="$(value_for panic_records)" || fail "missing panic_records"
persistent="$(value_for persistent)" || fail "missing persistent"
storage="$(value_for storage)" || fail "missing storage"
storage_capacity_bytes="$(value_for storage_capacity_bytes)" || fail "missing storage_capacity_bytes"
last_sync_attempted="$(value_for last_sync_attempted)" || fail "missing last_sync_attempted"
last_sync_persistent="$(value_for last_sync_persistent)" || fail "missing last_sync_persistent"
last_sync_storage="$(value_for last_sync_storage)" || fail "missing last_sync_storage"
last_sync_storage_capacity_bytes="$(value_for last_sync_storage_capacity_bytes)" || fail "missing last_sync_storage_capacity_bytes"
last_sync_status="$(value_for last_sync_status)" || fail "missing last_sync_status"
last_sync_bytes="$(value_for last_sync_bytes)" || fail "missing last_sync_bytes"
last_sync_checksum="$(value_for last_sync_checksum)" || fail "missing last_sync_checksum"
last_sync_verified="$(value_for last_sync_verified)" || fail "missing last_sync_verified"
last_sync_reason="$(value_for last_sync_reason)" || fail "missing last_sync_reason"
klog_retained_bytes="$(value_for klog_retained_bytes)" || fail "missing klog_retained_bytes"
klog_dropped_bytes="$(value_for klog_dropped_bytes)" || fail "missing klog_dropped_bytes"
klog_next_event_seq="$(value_for klog_next_event_seq)" || fail "missing klog_next_event_seq"
event_records="$(value_for event_records)" || fail "missing event_records"
process_records="$(value_for process_records)" || fail "missing process_records"
service_records="$(value_for service_records)" || fail "missing service_records"
exec_load_records="$(value_for exec_load_records)" || fail "missing exec_load_records"
pending_exec_records="$(value_for pending_exec_records)" || fail "missing pending_exec_records"
wait_records="$(value_for wait_records)" || fail "missing wait_records"
task_records="$(value_for task_records)" || fail "missing task_records"
syscall_records="$(value_for syscall_records)" || fail "missing syscall_records"
syscall_continuation_records="$(value_for syscall_continuation_records)" || fail "missing syscall_continuation_records"
expected_checksum="$(value_for checksum)" || fail "missing checksum"
actual_checksum="$(checksum32 "$checksum_payload")"

for key_value in \
    "boot_id:$boot_id" \
    "session_id:$session_id" \
    "klog_retained_bytes:$klog_retained_bytes" \
    "klog_dropped_bytes:$klog_dropped_bytes" \
    "klog_next_event_seq:$klog_next_event_seq" \
    "boot_memory_ranges:$boot_memory_ranges" \
    "boot_memory_usable_bytes:$boot_memory_usable_bytes" \
    "boot_cpu_count:$boot_cpu_count" \
    "boot_heap_total_bytes:$boot_heap_total_bytes" \
    "device_records:$device_records" \
    "panic_records:$panic_records" \
    "event_records:$event_records" \
    "process_records:$process_records" \
    "service_records:$service_records" \
    "exec_load_records:$exec_load_records" \
    "pending_exec_records:$pending_exec_records" \
    "wait_records:$wait_records" \
    "task_records:$task_records" \
    "syscall_records:$syscall_records" \
    "syscall_continuation_records:$syscall_continuation_records" \
    "storage_capacity_bytes:$storage_capacity_bytes" \
    "last_sync_storage_capacity_bytes:$last_sync_storage_capacity_bytes" \
    "last_sync_bytes:$last_sync_bytes" \
    "last_sync_checksum:$last_sync_checksum" \
    "checksum:$expected_checksum"
do
    require_number "${key_value%%:*}" "${key_value#*:}"
done

for key_value in \
    "identity_source:$identity_source" \
    "package:$package" \
    "version:$version" \
    "target:$target" \
    "selected_profile:$selected_profile" \
    "profile_request:$profile_request" \
    "bootline:$bootline" \
    "launch_profile_feature:$launch_profile_feature" \
    "proof_state:$proof_state" \
    "panic_state:$panic_state" \
    "storage:$storage" \
    "last_sync_storage:$last_sync_storage" \
    "last_sync_reason:$last_sync_reason"
do
    require_text "${key_value%%:*}" "${key_value#*:}"
done

[ "$persistent" = "available" ] || [ "$persistent" = "unavailable" ] \
    || fail "persistent must be available or unavailable: $persistent"
[ "$last_sync_attempted" = "true" ] || [ "$last_sync_attempted" = "false" ] \
    || fail "last_sync_attempted must be true or false: $last_sync_attempted"
[ "$last_sync_persistent" = "available" ] || [ "$last_sync_persistent" = "unavailable" ] \
    || fail "last_sync_persistent must be available or unavailable: $last_sync_persistent"
[ "$last_sync_status" = "written" ] || [ "$last_sync_status" = "not-written" ] \
    || fail "last_sync_status must be written or not-written: $last_sync_status"
[ "$last_sync_verified" = "true" ] || [ "$last_sync_verified" = "false" ] \
    || fail "last_sync_verified must be true or false: $last_sync_verified"
if [ "$persistent" = "unavailable" ] && [ "$storage" != "none" ]; then
    fail "unavailable dump persistence must report storage=none"
fi
if [ "$persistent" = "unavailable" ] && [ "$storage_capacity_bytes" != "0" ]; then
    fail "unavailable dump persistence must report storage_capacity_bytes=0"
fi
if [ "$last_sync_attempted" = "false" ] && [ "$last_sync_reason" != "never-synced" ]; then
    fail "unattempted last sync must report reason=never-synced"
fi
if [ "$last_sync_persistent" = "unavailable" ] && [ "$last_sync_storage" != "none" ]; then
    fail "unavailable last sync must report storage=none"
fi
if [ "$last_sync_persistent" = "unavailable" ] && [ "$last_sync_storage_capacity_bytes" != "0" ]; then
    fail "unavailable last sync must report last_sync_storage_capacity_bytes=0"
fi
if [ "$last_sync_attempted" = "true" ] && [ "$last_sync_status" = "written" ]; then
    [ "$last_sync_persistent" = "available" ] \
        || fail "written last sync must report last_sync_persistent=available"
    [ "$last_sync_verified" = "true" ] \
        || fail "written last sync must report last_sync_verified=true"
    [ "$last_sync_reason" = "written-readback-ok" ] \
        || fail "written last sync must report reason=written-readback-ok"
    [ "$last_sync_bytes" -gt 0 ] \
        || fail "written last sync must report nonzero last_sync_bytes"
    [ "$last_sync_checksum" -gt 0 ] \
        || fail "written last sync must report nonzero last_sync_checksum"
fi
[ "$panic_state" = "none" ] || [ "$panic_state" = "recorded" ] \
    || fail "panic_state must be none or recorded: $panic_state"
if [ "$panic_state" = "none" ] && [ "$panic_records" != "0" ]; then
    fail "panic_state=none must report panic_records=0"
fi
if [ "$panic_state" = "recorded" ] && [ "$panic_records" = "0" ]; then
    fail "panic_state=recorded must report at least one panic record"
fi
if [ "$actual_checksum" != "$expected_checksum" ]; then
    fail "checksum mismatch: expected $expected_checksum got $actual_checksum"
fi

if [ -n "$expect_package" ] && [ "$package" != "$expect_package" ]; then
    fail "package mismatch: expected $expect_package got $package"
fi
if [ -n "$expect_target" ] && [ "$target" != "$expect_target" ]; then
    fail "target mismatch: expected $expect_target got $target"
fi
if [ -n "$expect_profile" ] && [ "$selected_profile" != "$expect_profile" ]; then
    fail "profile mismatch: expected $expect_profile got $selected_profile"
fi
if [ "$require_zero_drops" -eq 1 ] && [ "$klog_dropped_bytes" != "0" ]; then
    fail "dropped log bytes present: $klog_dropped_bytes"
fi

persistent_dump_proof=false
if [ "$persistent" = "available" ] \
    && [ "$storage" != "none" ] \
    && [ "$storage_capacity_bytes" -gt 0 ] \
    && [ "$last_sync_attempted" = "true" ] \
    && [ "$last_sync_persistent" = "available" ] \
    && [ "$last_sync_status" = "written" ] \
    && [ "$last_sync_verified" = "true" ] \
    && [ "$last_sync_reason" = "written-readback-ok" ] \
    && [ "$last_sync_bytes" -gt 0 ] \
    && [ "$last_sync_checksum" -gt 0 ]; then
    persistent_dump_proof=true
fi
if [ "$require_persistent_proof" -eq 1 ] && [ "$persistent_dump_proof" != "true" ]; then
    fail "persistent dump proof is not satisfied"
fi

for section in boot devices proof dump-sync panic events processes services execs pending scheduler syscalls continuations tasks waits; do
    require_section "$section"
done

if [ "$exec_load_records" -gt 0 ]; then
    grep -Eq '^- seq=[0-9]+ .* path=[^ ]+ source=[^ ]+ loader=[^ ]+ entry_fn=[^ ]+ .* kind=(bin|payload) origin=(image-linked|installed-overlay|source-media-catalog|source-media-single|block-bundle|none)$' "$dump_file" \
        || fail "missing exec source path row"
fi
if [ "$syscall_continuation_records" -gt 0 ]; then
    grep -Eq '^- pid=[0-9]+ task=[0-9]+ path=[^ ]+ nr=[0-9]+ op=[^ ]+ memory=(none|raw-read-buffer|raw-write-buffer|raw-wait-report) .* loader=[^ ]+ entry_fn=[^ ]+$' "$dump_file" \
        || fail "missing active syscall continuation row"
    grep -Eq '^- seq=[0-9]+ boot=[0-9]+ session=[0-9]+ source=process component=syscall severity=info kind=syscall-continue-blocked pid=[0-9]+ task=[0-9]+$' "$dump_file" \
        || fail "missing active syscall continuation event row"
fi

printf 'dump_analysis=ok\n'
printf 'dump_file=%s\n' "$dump_file"
printf 'format=%s\n' "$format"
printf 'boot_id=%s\n' "$boot_id"
printf 'session_id=%s\n' "$session_id"
printf 'identity_source=%s\n' "$identity_source"
printf 'package=%s\n' "$package"
printf 'version=%s\n' "$version"
printf 'target=%s\n' "$target"
printf 'selected_profile=%s\n' "$selected_profile"
printf 'profile_request=%s\n' "$profile_request"
printf 'bootline=%s\n' "$bootline"
printf 'launch_profile_feature=%s\n' "$launch_profile_feature"
printf 'boot_memory_ranges=%s\n' "$boot_memory_ranges"
printf 'boot_memory_usable_bytes=%s\n' "$boot_memory_usable_bytes"
printf 'boot_cpu_count=%s\n' "$boot_cpu_count"
printf 'boot_heap_total_bytes=%s\n' "$boot_heap_total_bytes"
printf 'device_records=%s\n' "$device_records"
printf 'proof_state=%s\n' "$proof_state"
printf 'panic_state=%s\n' "$panic_state"
printf 'panic_records=%s\n' "$panic_records"
printf 'persistent=%s\n' "$persistent"
printf 'storage=%s\n' "$storage"
printf 'storage_capacity_bytes=%s\n' "$storage_capacity_bytes"
printf 'persistent_dump_proof=%s\n' "$persistent_dump_proof"
printf 'last_sync_attempted=%s\n' "$last_sync_attempted"
printf 'last_sync_persistent=%s\n' "$last_sync_persistent"
printf 'last_sync_storage=%s\n' "$last_sync_storage"
printf 'last_sync_storage_capacity_bytes=%s\n' "$last_sync_storage_capacity_bytes"
printf 'last_sync_status=%s\n' "$last_sync_status"
printf 'last_sync_bytes=%s\n' "$last_sync_bytes"
printf 'last_sync_checksum=%s\n' "$last_sync_checksum"
printf 'last_sync_verified=%s\n' "$last_sync_verified"
printf 'last_sync_reason=%s\n' "$last_sync_reason"
printf 'klog_retained_bytes=%s\n' "$klog_retained_bytes"
printf 'klog_dropped_bytes=%s\n' "$klog_dropped_bytes"
printf 'klog_next_event_seq=%s\n' "$klog_next_event_seq"
printf 'event_records=%s\n' "$event_records"
printf 'process_records=%s\n' "$process_records"
printf 'service_records=%s\n' "$service_records"
printf 'exec_load_records=%s\n' "$exec_load_records"
printf 'pending_exec_records=%s\n' "$pending_exec_records"
printf 'wait_records=%s\n' "$wait_records"
printf 'task_records=%s\n' "$task_records"
printf 'syscall_records=%s\n' "$syscall_records"
printf 'syscall_continuation_records=%s\n' "$syscall_continuation_records"
printf 'checksum=%s\n' "$expected_checksum"
