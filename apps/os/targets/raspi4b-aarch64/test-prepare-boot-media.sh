#!/usr/bin/env bash
# Smoke-test the Raspberry Pi 4 boot media preparation wrapper.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET="aarch64-unknown-none"
PACKAGE="reovim-os"
IMAGE="$ROOT/apps/target/$TARGET/debug/$PACKAGE.kernel8.img"
PREPARE_SCRIPT="$ROOT/apps/os/targets/raspi4b-aarch64/prepare-boot-media.sh"

tmp_root="$(mktemp -d)"
tmp_bootfs="$tmp_root/bootfs"
tmp_evidence="$tmp_root/evidence.md"
existing_evidence="$tmp_root/existing.md"
cleanup() {
    rm -rf "$tmp_root"
}
trap cleanup EXIT

mkdir "$tmp_bootfs"
: >"$existing_evidence"

printf 'firmware-start\n' >"$tmp_bootfs/start4.elf"
printf 'firmware-fixup\n' >"$tmp_bootfs/fixup4.dat"
printf 'firmware-dtb\n' >"$tmp_bootfs/bcm2711-rpi-4-b.dtb"

expect_contains() {
    local haystack="$1"
    local needle="$2"
    local label="$3"

    case "$haystack" in
        *"$needle"*) ;;
        *)
            printf 'error: missing %s: %s\n' "$label" "$needle" >&2
            printf '%s\n' "$haystack" >&2
            exit 1
            ;;
    esac
}

if "$PREPARE_SCRIPT" --skip-qemu --bootfs "$tmp_bootfs" --evidence "$existing_evidence" \
    >"$tmp_bootfs/existing.out" 2>"$tmp_bootfs/existing.err"; then
    printf 'error: prepare script overwrote existing evidence\n' >&2
    cat "$tmp_bootfs/existing.out" >&2
    exit 1
fi

prepare_output="$("$PREPARE_SCRIPT" --skip-qemu --bootfs "$tmp_bootfs" --evidence "$tmp_evidence")"
source_bytes="$(wc -c <"$IMAGE" | tr -d ' ')"
source_sha="$(sha256sum "$IMAGE" | awk '{ print $1 }')"
installed_sha="$(sha256sum "$tmp_bootfs/kernel8.img" | awk '{ print $1 }')"

if [ "$source_sha" != "$installed_sha" ]; then
    printf 'error: installed kernel8.img SHA mismatch: expected %s got %s\n' "$source_sha" "$installed_sha" >&2
    exit 1
fi
if [ ! -s "$tmp_evidence" ]; then
    printf 'error: evidence seed was not published: %s\n' "$tmp_evidence" >&2
    exit 1
fi
if find "$tmp_root" -maxdepth 1 -name '.*.tmp.*' | grep -q .; then
    printf 'error: unexpected temporary evidence file remained\n' >&2
    exit 1
fi

expect_contains "$prepare_output" "preflight=ok" "preflight status"
expect_contains "$prepare_output" "bootfs=ok" "bootfs status"
expect_contains "$prepare_output" "qemu_smoke=skipped" "qemu skip status"
expect_contains "$prepare_output" "installed=$tmp_bootfs/kernel8.img" "install path"
expect_contains "$prepare_output" "media_prepare=ok" "media prepare status"
expect_contains "$prepare_output" "sha256=$source_sha" "installed sha"
expect_contains "$prepare_output" "evidence_seed=$tmp_evidence" "evidence path"

evidence="$(cat "$tmp_evidence")"
expect_contains "$evidence" "- Image SHA-256: $source_sha" "evidence image sha"
expect_contains "$evidence" "- Preflight result: preflight=ok qemu_smoke=skipped" "evidence preflight result"
expect_contains "$evidence" "- Boot partition path: $tmp_bootfs" "evidence bootfs"
expect_contains "$evidence" "pwd" "evidence pwd command"
expect_contains "$evidence" "ls /boot" "evidence boot ls command"
expect_contains "$evidence" "cat /boot/mounts" "evidence VFS mounts command"
expect_contains "$evidence" "\`pwd\` / \`ls\` / \`mount\` report the kernel VFS namespace and mounts." "evidence VFS namespace fact"
expect_contains "$evidence" "cat /boot/proof" "evidence VFS proof command"
expect_contains "$evidence" "\`cat /boot/proof\` prints the VFS-backed proof pseudo-file." "evidence VFS proof fact"
expect_contains "$evidence" "cat /boot/status" "evidence VFS status command"
expect_contains "$evidence" "cat /boot/input" "evidence VFS input command"
expect_contains "$evidence" "\`cat /boot/status\` / \`cat /boot/input\` report VFS-backed live diagnostics." "evidence VFS live diagnostics fact"
expect_contains "$evidence" "## Media Preparation" "evidence media section"
expect_contains "$evidence" "- [x] \`media_prepare=ok\`" "evidence media status checkbox"
expect_contains "$evidence" "- [x] installed \`kernel8.img\` SHA-256 matches image SHA-256." "evidence install hash checkbox"
expect_contains "$evidence" "- [x] installed \`kernel8.img\` byte count matches image byte count." "evidence install byte checkbox"
expect_contains "$evidence" "media_prepare=ok" "evidence media status"
expect_contains "$evidence" "installed=$tmp_bootfs/kernel8.img" "evidence installed path"
expect_contains "$evidence" "bytes=$source_bytes" "evidence installed bytes"
expect_contains "$evidence" "sha256=$source_sha" "evidence installed sha"

printf 'boot media prepare smoke ok\n'
printf 'sha256=%s\n' "$source_sha"
