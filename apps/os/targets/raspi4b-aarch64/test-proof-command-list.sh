#!/usr/bin/env bash
# Smoke-test that Pi 4 physical proof operator surfaces stay in sync.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"
PREFLIGHT_SCRIPT="$TARGET_DIR/preflight-real-board.sh"
README="$TARGET_DIR/README.md"
TEMPLATE="$TARGET_DIR/evidence-template.md"
ROOT_SHELL="$ROOT/system/lib/kernel/src/root_shell.rs"
VALIDATOR="$TARGET_DIR/validate-evidence.sh"

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

extract_kernel_proof_commands() {
    local source="$1"
    local output="$2"

    awk '
        /fn write_boot_proof\(daemon:/ { in_func = 1 }
        in_func && /daemon\.write_line\("commands:"\)/ { in_commands = 1; next }
        in_commands && /daemon\.write_line\("terminal:"\)/ { exit }
        in_commands && /daemon\.write_line\("  [^"]*"\);/ {
            line = $0
            sub(/^.*daemon\.write_line\("  /, "", line)
            sub(/"\);.*$/, "", line)
            print line
        }
    ' "$source" >"$output"

    if [ ! -s "$output" ]; then
        printf 'error: empty kernel proof command list extracted from %s\n' "$source" >&2
        exit 1
    fi
}

extract_kernel_proof_halt() {
    local source="$1"
    local output="$2"

    awk '
        /fn write_boot_proof\(daemon:/ { in_func = 1 }
        in_func && /daemon\.write_line\("terminal:"\)/ { in_terminal = 1; next }
        in_terminal && /daemon\.write_line\("expected:"\)/ { exit }
        in_terminal && /daemon\.write_line\("  [^"]*"\);/ {
            line = $0
            sub(/^.*daemon\.write_line\("  /, "", line)
            sub(/"\);.*$/, "", line)
            print line
        }
    ' "$source" >"$output"

    if [ ! -s "$output" ]; then
        printf 'error: empty kernel proof halt command extracted from %s\n' "$source" >&2
        exit 1
    fi
}

extract_kernel_static_expected_facts() {
    local source="$1"
    local output="$2"

    awk '
        /fn write_boot_proof\(daemon:/ { in_func = 1 }
        in_func && /daemon\.write_line\("expected:"\)/ { in_expected = 1; next }
        in_expected && /^}/ { exit }
        in_expected && /daemon\.write_line\("  [^"]*"\);/ {
            line = $0
            sub(/^.*daemon\.write_line\("  /, "", line)
            sub(/"\);.*$/, "", line)
            print line
        }
        in_expected && /^[[:space:]]*"  [^"]*",$/ {
            line = $0
            sub(/^[[:space:]]*"  /, "", line)
            sub(/",$/, "", line)
            print line
        }
    ' "$source" | LC_ALL=C sort -u >"$output"

    if [ ! -s "$output" ]; then
        printf 'error: empty kernel proof expected-fact list extracted from %s\n' "$source" >&2
        exit 1
    fi
}

extract_validator_static_expected_facts() {
    local source="$1"
    local output="$2"

    awk -F"'" '
        $1 ~ /^require_re / && $4 ~ /^proof expected / && $2 ~ /^\^  / {
            line = $2
            sub(/^\^  /, "", line)
            sub(/\$$/, "", line)
            if (line ~ /^(package=|version=|target=|selected_profile=|profile_request=|launch_profile_feature=|profile=|launch=|payloads=)/) {
                next
            }
            gsub(/\\\./, ".", line)
            gsub(/\\\+/, "+", line)
            gsub(/\\\[/, "[", line)
            gsub(/\\\]/, "]", line)
            print line
        }
    ' "$source" | LC_ALL=C sort -u >"$output"

    if [ ! -s "$output" ]; then
        printf 'error: empty validator static expected-fact list extracted from %s\n' "$source" >&2
        exit 1
    fi
}

extract_validator_usb_source_commands() {
    local source="$1"
    local output="$2"

    awk -F"'" '
        /^require_usb_source_for_shell_command / {
            print $2
        }
    ' "$source" | LC_ALL=C sort -u >"$output"

    if [ ! -s "$output" ]; then
        printf 'error: empty validator USB-source command list extracted from %s\n' "$source" >&2
        exit 1
    fi
}

extract_validator_dmesg_audit_commands() {
    local source="$1"
    local output="$2"

    awk -F"'" '
        $1 ~ /^require_re / && $2 ~ /^\^shell: / {
            line = $2
            sub(/^\^shell: /, "", line)
            sub(/\$$/, "", line)
            print line
        }
    ' "$source" | LC_ALL=C sort -u >"$output"

    if [ ! -s "$output" ]; then
        printf 'error: empty validator dmesg-audit command list extracted from %s\n' "$source" >&2
        exit 1
    fi
}

extract_validator_prompt_commands() {
    local source="$1"
    local output="$2"

    awk -F"'" '
        $1 ~ /^require_(re|count_at_least) / && $2 ~ /^\^?reovim-os> / {
            line = $2
            sub(/^\^?reovim-os> /, "", line)
            sub(/\$$/, "", line)
            print line
        }
    ' "$source" | LC_ALL=C sort -u >"$output"

    if [ ! -s "$output" ]; then
        printf 'error: empty validator prompt command list extracted from %s\n' "$source" >&2
        exit 1
    fi
}

sort_unique_lines() {
    local source="$1"
    local output="$2"

    LC_ALL=C sort -u "$source" >"$output"
    if [ ! -s "$output" ]; then
        printf 'error: empty sorted command list extracted from %s\n' "$source" >&2
        exit 1
    fi
}

sort_unique_combined_lines() {
    local first="$1"
    local second="$2"
    local output="$3"

    cat "$first" "$second" | LC_ALL=C sort -u >"$output"
    if [ ! -s "$output" ]; then
        printf 'error: empty combined command list extracted from %s and %s\n' "$first" "$second" >&2
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
extract_kernel_proof_commands \
    "$ROOT_SHELL" \
    "$tmp_root/kernel.commands"
extract_kernel_proof_halt \
    "$ROOT_SHELL" \
    "$tmp_root/kernel.halt"
extract_kernel_static_expected_facts \
    "$ROOT_SHELL" \
    "$tmp_root/kernel.static-expected"
extract_validator_static_expected_facts \
    "$VALIDATOR" \
    "$tmp_root/validator.static-expected"
sort_unique_lines \
    "$tmp_root/template.commands" \
    "$tmp_root/template.commands.unique"
sort_unique_combined_lines \
    "$tmp_root/template.commands" \
    "$tmp_root/template.halt" \
    "$tmp_root/template.commands-plus-halt.unique"
extract_validator_usb_source_commands \
    "$VALIDATOR" \
    "$tmp_root/validator.usb-source.commands"
extract_validator_dmesg_audit_commands \
    "$VALIDATOR" \
    "$tmp_root/validator.dmesg-audit.commands"
extract_validator_prompt_commands \
    "$VALIDATOR" \
    "$tmp_root/validator.prompt.commands"
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
expect_same "$tmp_root/template.commands" "$tmp_root/kernel.commands" "system-kernel /boot/proof commands"
expect_same "$tmp_root/template.commands-plus-halt.unique" "$tmp_root/validator.prompt.commands" "validate-evidence prompt command guards"
expect_same "$tmp_root/template.commands.unique" "$tmp_root/validator.usb-source.commands" "validate-evidence USB-source command guards"
expect_same "$tmp_root/template.commands.unique" "$tmp_root/validator.dmesg-audit.commands" "validate-evidence dmesg audit command guards"
expect_same "$tmp_root/template.halt" "$tmp_root/readme.halt" "README.md halt command"
expect_same "$tmp_root/template.halt" "$tmp_root/seed.halt" "generated evidence seed halt command"
expect_same "$tmp_root/template.halt" "$tmp_root/kernel.halt" "system-kernel /boot/proof halt command"
expect_same "$tmp_root/kernel.static-expected" "$tmp_root/validator.static-expected" "validate-evidence static /boot/proof expected facts"
expect_same "$tmp_root/template.boot-facts" "$tmp_root/seed.boot-facts" "generated evidence seed boot facts"
expect_same "$tmp_root/template.success-facts" "$tmp_root/seed.success-facts" "generated evidence seed success facts"
expect_same "$tmp_root/template.result" "$tmp_root/seed.result" "generated evidence seed result labels"

printf 'proof contract smoke ok\n'
