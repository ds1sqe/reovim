#!/usr/bin/env bash
# Prepare Raspberry Pi 4 boot media for the #800 physical-board proof.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"
TARGET="aarch64-unknown-none"
PACKAGE="reovim-os"
IMAGE="$ROOT/apps/target/$TARGET/debug/$PACKAGE.kernel8.img"
PREFLIGHT_SCRIPT="$TARGET_DIR/preflight-real-board.sh"
INSTALL_SCRIPT="$TARGET_DIR/install-image.sh"
FIND_SCRIPT="${REOVIM_FIND_BOOTFS_SCRIPT:-$TARGET_DIR/find-bootfs.sh}"

BOOTFS=""
EVIDENCE_OUT=""
SKIP_QEMU=0
NO_BACKUP=0
tmp_evidence=""
tmp_prepared_evidence=""

usage() {
    cat <<'USAGE'
Usage: apps/os/targets/raspi4b-aarch64/prepare-boot-media.sh --bootfs DIR --evidence FILE [options]

Options:
  --bootfs DIR    Mounted Raspberry Pi 4 FAT boot partition, or "auto".
  --evidence FILE Final evidence seed path to create after install verifies.
  --skip-qemu     Skip the local aarch64 QEMU smoke.
  --no-backup     Replace kernel8.img without saving a timestamped backup.
  -h, --help      Show this help text.

Runs preflight, installs the exact built kernel8.img, verifies the installed
image hash, then publishes the evidence seed for the physical HDMI + USB
keyboard session. This script does not format or mount media.

With --bootfs auto, the script uses find-bootfs.sh and proceeds only when
exactly one Raspberry Pi 4 boot partition candidate is found.
USAGE
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

cleanup() {
    if [ -n "$tmp_evidence" ] && [ -e "$tmp_evidence" ]; then
        rm -f "$tmp_evidence"
    fi
    if [ -n "$tmp_prepared_evidence" ] && [ -e "$tmp_prepared_evidence" ]; then
        rm -f "$tmp_prepared_evidence"
    fi
}
trap cleanup EXIT

resolve_auto_bootfs() {
    local output line
    local -a candidates=()

    if ! output="$("$FIND_SCRIPT" 2>&1)"; then
        fail "auto bootfs discovery failed: $output"
    fi

    while IFS= read -r line; do
        case "$line" in
            bootfs_candidate=*) candidates+=("${line#bootfs_candidate=}") ;;
            *) ;;
        esac
    done <<<"$output"

    if [ "${#candidates[@]}" -eq 0 ]; then
        fail "auto bootfs discovery found no candidates"
    fi
    if [ "${#candidates[@]}" -gt 1 ]; then
        printf '%s\n' "$output" >&2
        fail "auto bootfs discovery found multiple candidates; pass --bootfs DIR"
    fi

    BOOTFS="${candidates[0]}"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --bootfs)
            shift
            if [ "$#" -eq 0 ]; then
                fail "--bootfs needs a directory"
            fi
            BOOTFS="$1"
            ;;
        --evidence)
            shift
            if [ "$#" -eq 0 ]; then
                fail "--evidence needs an output file"
            fi
            EVIDENCE_OUT="$1"
            ;;
        --skip-qemu)
            SKIP_QEMU=1
            ;;
        --no-backup)
            NO_BACKUP=1
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*)
            fail "unknown argument: $1"
            ;;
        *)
            fail "unexpected positional argument: $1"
            ;;
    esac
    shift
done

if [ -z "$BOOTFS" ]; then
    fail "--bootfs is required"
fi
if [ "$BOOTFS" = "auto" ]; then
    resolve_auto_bootfs
fi
if [ -z "$EVIDENCE_OUT" ]; then
    fail "--evidence is required"
fi
if [ -e "$EVIDENCE_OUT" ]; then
    fail "evidence output already exists: $EVIDENCE_OUT"
fi

evidence_parent="$(dirname "$EVIDENCE_OUT")"
evidence_base="$(basename "$EVIDENCE_OUT")"
if [ ! -d "$evidence_parent" ]; then
    fail "evidence output directory does not exist: $evidence_parent"
fi
tmp_evidence="$evidence_parent/.${evidence_base}.tmp.$$"
if [ -e "$tmp_evidence" ]; then
    fail "temporary evidence path already exists: $tmp_evidence"
fi

preflight_args=(--bootfs "$BOOTFS" --evidence "$tmp_evidence")
if [ "$SKIP_QEMU" -eq 1 ]; then
    preflight_args+=(--skip-qemu)
fi

"$PREFLIGHT_SCRIPT" "${preflight_args[@]}"

install_args=()
if [ "$NO_BACKUP" -eq 1 ]; then
    install_args+=(--no-backup)
fi
"$INSTALL_SCRIPT" "${install_args[@]}" "$BOOTFS"

bootfs_abs="$(cd "$BOOTFS" && pwd -P)"
installed="$bootfs_abs/kernel8.img"
if [ ! -s "$installed" ]; then
    fail "installed kernel8.img missing or empty: $installed"
fi

source_sha="$(sha256sum "$IMAGE" | awk '{ print $1 }')"
installed_sha="$(sha256sum "$installed" | awk '{ print $1 }')"
if [ "$source_sha" != "$installed_sha" ]; then
    fail "installed kernel8.img SHA mismatch: expected $source_sha got $installed_sha"
fi

bytes="$(wc -c <"$installed" | tr -d ' ')"
tmp_prepared_evidence="$evidence_parent/.${evidence_base}.prepared.$$"
if [ "$NO_BACKUP" -eq 1 ]; then
    install_command="${INSTALL_SCRIPT#$ROOT/} --no-backup $bootfs_abs"
else
    install_command="${INSTALL_SCRIPT#$ROOT/} $bootfs_abs"
fi
preflight_command="${PREFLIGHT_SCRIPT#$ROOT/} --bootfs $bootfs_abs --evidence $tmp_evidence"
if [ "$SKIP_QEMU" -eq 1 ]; then
    preflight_command="$preflight_command --skip-qemu"
fi
awk \
    -v bootfs="$bootfs_abs" \
    -v installed="$installed" \
    -v bytes="$bytes" \
    -v install_command="$install_command" \
    -v preflight_command="$preflight_command" \
    -v sha256="$installed_sha" '
function write_media_section() {
    print "";
    print "## Media Preparation";
    print "";
    print "Required media facts:";
    print "";
    print "- [x] `media_prepare=ok`";
    print "- [x] installed `kernel8.img` SHA-256 matches image SHA-256.";
    print "- [x] installed `kernel8.img` byte count matches image byte count.";
    print "";
    print "Paste the media-prep result:";
    print "";
    print "```text";
    print "media_prepare=ok";
    print "bootfs=" bootfs;
    print "installed=" installed;
    print "bytes=" bytes;
    print "sha256=" sha256;
    print "```";
}
/^## Boot Evidence$/ && !inserted {
    write_media_section();
    inserted = 1;
}
/^- Image install command:/ {
    print "- Image install command: " install_command;
    next;
}
/^- Preflight command:/ {
    print "- Preflight command: " preflight_command;
    next;
}
{ print }
END {
    if (!inserted) {
        write_media_section();
    }
}
' "$tmp_evidence" >"$tmp_prepared_evidence"
mv "$tmp_prepared_evidence" "$tmp_evidence"
tmp_prepared_evidence=""

mv "$tmp_evidence" "$EVIDENCE_OUT"
tmp_evidence=""

printf 'media_prepare=ok\n'
printf 'bootfs=%s\n' "$bootfs_abs"
printf 'installed=%s\n' "$installed"
printf 'bytes=%s\n' "$bytes"
printf 'sha256=%s\n' "$installed_sha"
printf 'evidence_seed=%s\n' "$EVIDENCE_OUT"
printf 'next=boot Pi 4 with HDMI and physical USB keyboard, then fill evidence\n'
