#!/usr/bin/env bash
# Build the Raspberry Pi 4 shell-only RTOS image and convert it to kernel8.img.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET="aarch64-unknown-none"
PACKAGE="reovim-os"
ELF="$ROOT/apps/target/$TARGET/debug/$PACKAGE"
IMAGE="$ELF.kernel8.img"

usage() {
    cat <<'USAGE'
Usage: apps/os/targets/raspi4b-aarch64/build-image.sh

Builds the no-bootline shell-only Raspberry Pi 4 OS image:
  apps/target/aarch64-unknown-none/debug/reovim-os.kernel8.img

The script intentionally unsets REOVIM_OS_BOOTLINE and REOVIM_OS_PROFILE so the
result is suitable for real-machine input proof.
USAGE
}

case "${1:-}" in
    "") ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        echo "error: unknown argument: $1" >&2
        usage >&2
        exit 2
        ;;
esac

rust_host() {
    rustc -vV | while IFS= read -r line; do
        case "$line" in
            "host: "*)
                printf '%s\n' "${line#host: }"
                return 0
                ;;
        esac
    done
}

llvm_objcopy() {
    local sysroot
    local host
    sysroot="$(rustc --print sysroot)"
    host="$(rust_host)"
    if [ -z "$host" ]; then
        echo "error: rustc -vV did not report a host triple" >&2
        exit 1
    fi
    printf '%s/lib/rustlib/%s/bin/llvm-objcopy\n' "$sysroot" "$host"
}

cd "$ROOT"

env -u REOVIM_OS_BOOTLINE -u REOVIM_OS_PROFILE \
    cargo build --manifest-path apps/Cargo.toml \
    -p "$PACKAGE" \
    --target "$TARGET"

"$(llvm_objcopy)" -O binary "$ELF" "$IMAGE"

bytes="$(wc -c <"$IMAGE" | tr -d ' ')"
printf 'image=%s\n' "${IMAGE#$ROOT/}"
printf 'bytes=%s\n' "$bytes"
