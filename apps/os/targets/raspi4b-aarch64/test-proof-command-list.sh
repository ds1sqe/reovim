#!/usr/bin/env bash
# Smoke-test that Pi 4 physical proof operator surfaces stay in sync.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"
PREFLIGHT_SCRIPT="$TARGET_DIR/preflight-real-board.sh"
README="$TARGET_DIR/README.md"
TEMPLATE="$TARGET_DIR/evidence-template.md"

tmp_root="$(mktemp -d)"
cleanup() {
    rm -rf "$tmp_root"
}
trap cleanup EXIT

tmp_bootfs="$tmp_root/bootfs"
tmp_evidence="$tmp_root/evidence.md"
mkdir "$tmp_bootfs"
printf 'firmware-start\n' >"$tmp_bootfs/start4.elf"
printf 'firmware-fixup\n' >"$tmp_bootfs/fixup4.dat"
printf 'firmware-dtb\n' >"$tmp_bootfs/bcm2711-rpi-4-b.dtb"

extract_command_block() {
    local source="$1"
    local anchor="$2"
    local output="$3"

    awk -v anchor="$anchor" '
        index($0, anchor) { seen_anchor = 1; next }
        seen_anchor && /^```text$/ { in_block = 1; next }
        in_block && /^```$/ { exit }
        in_block { print }
    ' "$source" >"$output"

    if [ ! -s "$output" ]; then
        printf 'error: empty command block extracted from %s using anchor %s\n' "$source" "$anchor" >&2
        exit 1
    fi
}

extract_checkbox_block() {
    local source="$1"
    local anchor="$2"
    local output="$3"

    awk -v anchor="$anchor" '
        index($0, anchor) { seen_anchor = 1; next }
        seen_anchor && /^- \[ \]/ { in_block = 1 }
        in_block && /^$/ { exit }
        in_block { print }
    ' "$source" >"$output"

    if [ ! -s "$output" ]; then
        printf 'error: empty checkbox block extracted from %s using anchor %s\n' "$source" "$anchor" >&2
        exit 1
    fi
}

expect_same() {
    local expected="$1"
    local actual="$2"
    local label="$3"

    if ! cmp -s "$expected" "$actual"; then
        printf 'error: proof command list drifted in %s\n' "$label" >&2
        diff -u "$expected" "$actual" >&2 || true
        exit 1
    fi
}

"$PREFLIGHT_SCRIPT" --skip-qemu --bootfs "$tmp_bootfs" --evidence "$tmp_evidence" >/dev/null

extract_command_block \
    "$TEMPLATE" \
    "Type these commands from the physical USB keyboard for final acceptance:" \
    "$tmp_root/template.commands"
extract_command_block \
    "$README" \
    "keyboard for the main proof transcript:" \
    "$tmp_root/readme.commands"
extract_command_block \
    "$tmp_evidence" \
    "Type these commands from the physical USB keyboard for final acceptance:" \
    "$tmp_root/seed.commands"
extract_command_block \
    "$TEMPLATE" \
    "the physical USB keyboard:" \
    "$tmp_root/template.halt"
extract_command_block \
    "$README" \
    "physical USB keyboard:" \
    "$tmp_root/readme.halt"
extract_command_block \
    "$tmp_evidence" \
    "the physical USB keyboard:" \
    "$tmp_root/seed.halt"
extract_checkbox_block \
    "$TEMPLATE" \
    "Required facts:" \
    "$tmp_root/template.boot-facts"
extract_checkbox_block \
    "$tmp_evidence" \
    "Required facts:" \
    "$tmp_root/seed.boot-facts"
extract_checkbox_block \
    "$TEMPLATE" \
    "Required success facts:" \
    "$tmp_root/template.success-facts"
extract_checkbox_block \
    "$tmp_evidence" \
    "Required success facts:" \
    "$tmp_root/seed.success-facts"
extract_checkbox_block \
    "$TEMPLATE" \
    "## Result" \
    "$tmp_root/template.result"
extract_checkbox_block \
    "$tmp_evidence" \
    "## Result" \
    "$tmp_root/seed.result"

expect_same "$tmp_root/template.commands" "$tmp_root/readme.commands" "README.md"
expect_same "$tmp_root/template.commands" "$tmp_root/seed.commands" "generated evidence seed"
expect_same "$tmp_root/template.halt" "$tmp_root/readme.halt" "README.md halt command"
expect_same "$tmp_root/template.halt" "$tmp_root/seed.halt" "generated evidence seed halt command"
expect_same "$tmp_root/template.boot-facts" "$tmp_root/seed.boot-facts" "generated evidence seed boot facts"
expect_same "$tmp_root/template.success-facts" "$tmp_root/seed.success-facts" "generated evidence seed success facts"
expect_same "$tmp_root/template.result" "$tmp_root/seed.result" "generated evidence seed result labels"

printf 'proof contract smoke ok\n'
