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
tmp_bootfs_no_backup="$tmp_root/bootfs-no-backup"
tmp_bootfs_auto="$tmp_root/bootfs-auto"
tmp_evidence="$tmp_root/evidence.md"
tmp_evidence_no_backup="$tmp_root/evidence-no-backup.md"
tmp_evidence_auto="$tmp_root/evidence-auto.md"
tmp_evidence_auto_many="$tmp_root/evidence-auto-many.md"
tmp_evidence_auto_none="$tmp_root/evidence-auto-none.md"
existing_evidence="$tmp_root/existing.md"
tmp_find_one="$tmp_root/find-one.sh"
tmp_find_many="$tmp_root/find-many.sh"
tmp_find_none="$tmp_root/find-none.sh"
cleanup() {
    rm -rf "$tmp_root"
}
trap cleanup EXIT

mkdir "$tmp_bootfs"
mkdir "$tmp_bootfs_no_backup"
mkdir "$tmp_bootfs_auto"
: >"$existing_evidence"

printf 'firmware-start\n' >"$tmp_bootfs/start4.elf"
printf 'firmware-fixup\n' >"$tmp_bootfs/fixup4.dat"
printf 'firmware-dtb\n' >"$tmp_bootfs/bcm2711-rpi-4-b.dtb"
printf 'firmware-start\n' >"$tmp_bootfs_no_backup/start4.elf"
printf 'firmware-fixup\n' >"$tmp_bootfs_no_backup/fixup4.dat"
printf 'firmware-dtb\n' >"$tmp_bootfs_no_backup/bcm2711-rpi-4-b.dtb"
printf 'old-kernel\n' >"$tmp_bootfs_no_backup/kernel8.img"
printf 'firmware-start\n' >"$tmp_bootfs_auto/start4.elf"
printf 'firmware-fixup\n' >"$tmp_bootfs_auto/fixup4.dat"
printf 'firmware-dtb\n' >"$tmp_bootfs_auto/bcm2711-rpi-4-b.dtb"

cat >"$tmp_find_one" <<FIND
#!/usr/bin/env bash
printf 'bootfs_candidate=$tmp_bootfs_auto\n'
printf 'source=test\n'
printf 'fstype=vfat\n'
FIND
cat >"$tmp_find_many" <<FIND
#!/usr/bin/env bash
printf 'bootfs_candidate=$tmp_bootfs\n'
printf 'source=test-a\n'
printf 'fstype=vfat\n'
printf 'bootfs_candidate=$tmp_bootfs_auto\n'
printf 'source=test-b\n'
printf 'fstype=vfat\n'
FIND
cat >"$tmp_find_none" <<'FIND'
#!/usr/bin/env bash
printf 'no mounted Raspberry Pi 4 bootfs candidates found\n' >&2
exit 1
FIND
chmod +x "$tmp_find_one" "$tmp_find_many" "$tmp_find_none"

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

expect_not_contains() {
    local haystack="$1"
    local needle="$2"
    local label="$3"

    case "$haystack" in
        *"$needle"*)
            printf 'error: unexpected %s: %s\n' "$label" "$needle" >&2
            printf '%s\n' "$haystack" >&2
            exit 1
            ;;
        *) ;;
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
no_backup_output="$("$PREPARE_SCRIPT" --skip-qemu --no-backup --bootfs "$tmp_bootfs_no_backup" --evidence "$tmp_evidence_no_backup")"
no_backup_sha="$(sha256sum "$tmp_bootfs_no_backup/kernel8.img" | awk '{ print $1 }')"
auto_output="$(
    REOVIM_FIND_BOOTFS_SCRIPT="$tmp_find_one" \
        "$PREPARE_SCRIPT" --skip-qemu --bootfs auto --evidence "$tmp_evidence_auto"
)"
auto_sha="$(sha256sum "$tmp_bootfs_auto/kernel8.img" | awk '{ print $1 }')"

if REOVIM_FIND_BOOTFS_SCRIPT="$tmp_find_many" \
    "$PREPARE_SCRIPT" --skip-qemu --bootfs auto --evidence "$tmp_evidence_auto_many" \
    >"$tmp_root/auto-many.out" 2>"$tmp_root/auto-many.err"; then
    printf 'error: prepare script accepted multiple auto bootfs candidates\n' >&2
    cat "$tmp_root/auto-many.out" >&2
    exit 1
fi
expect_contains "$(cat "$tmp_root/auto-many.err")" "multiple candidates" "auto multiple candidates failure"
if [ -e "$tmp_evidence_auto_many" ]; then
    printf 'error: multiple-candidate auto discovery published evidence\n' >&2
    exit 1
fi

if REOVIM_FIND_BOOTFS_SCRIPT="$tmp_find_none" \
    "$PREPARE_SCRIPT" --skip-qemu --bootfs auto --evidence "$tmp_evidence_auto_none" \
    >"$tmp_root/auto-none.out" 2>"$tmp_root/auto-none.err"; then
    printf 'error: prepare script accepted missing auto bootfs candidates\n' >&2
    cat "$tmp_root/auto-none.out" >&2
    exit 1
fi
expect_contains "$(cat "$tmp_root/auto-none.err")" "auto bootfs discovery failed" "auto no candidates failure"
if [ -e "$tmp_evidence_auto_none" ]; then
    printf 'error: missing-candidate auto discovery published evidence\n' >&2
    exit 1
fi

if [ "$source_sha" != "$installed_sha" ]; then
    printf 'error: installed kernel8.img SHA mismatch: expected %s got %s\n' "$source_sha" "$installed_sha" >&2
    exit 1
fi
if [ "$source_sha" != "$no_backup_sha" ]; then
    printf 'error: no-backup installed kernel8.img SHA mismatch: expected %s got %s\n' "$source_sha" "$no_backup_sha" >&2
    exit 1
fi
if [ "$source_sha" != "$auto_sha" ]; then
    printf 'error: auto installed kernel8.img SHA mismatch: expected %s got %s\n' "$source_sha" "$auto_sha" >&2
    exit 1
fi
if [ ! -s "$tmp_evidence" ]; then
    printf 'error: evidence seed was not published: %s\n' "$tmp_evidence" >&2
    exit 1
fi
if [ ! -s "$tmp_evidence_no_backup" ]; then
    printf 'error: no-backup evidence seed was not published: %s\n' "$tmp_evidence_no_backup" >&2
    exit 1
fi
if [ ! -s "$tmp_evidence_auto" ]; then
    printf 'error: auto evidence seed was not published: %s\n' "$tmp_evidence_auto" >&2
    exit 1
fi
if find "$tmp_bootfs_no_backup" -maxdepth 1 -name 'kernel8.img.bak-*' | grep -q .; then
    printf 'error: no-backup prepare created a kernel8.img backup\n' >&2
    find "$tmp_bootfs_no_backup" -maxdepth 1 -type f -printf '%f\n' >&2
    exit 1
fi
if find "$tmp_root" -maxdepth 1 -name '.*.tmp.*' | grep -q .; then
    printf 'error: unexpected temporary evidence file remained\n' >&2
    exit 1
fi
if find "$tmp_root" -maxdepth 1 -name '.*.prepared.*' | grep -q .; then
    printf 'error: unexpected prepared evidence file remained\n' >&2
    exit 1
fi

expect_contains "$prepare_output" "preflight=ok" "preflight status"
expect_contains "$prepare_output" "bootfs=ok" "bootfs status"
expect_contains "$prepare_output" "qemu_smoke=skipped" "qemu skip status"
expect_contains "$prepare_output" "installed=$tmp_bootfs/kernel8.img" "install path"
expect_contains "$prepare_output" "media_prepare=ok" "media prepare status"
expect_contains "$prepare_output" "sha256=$source_sha" "installed sha"
expect_contains "$prepare_output" "evidence_seed=$tmp_evidence" "evidence path"
expect_contains "$prepare_output" "warning=completed evidence validation requires qemu_smoke=passed; rerun without --skip-qemu before physical proof" "skip-qemu completion warning"
expect_contains "$prepare_output" "next=rerun prepare-boot-media without --skip-qemu before physical proof" "skip-qemu next action"
expect_not_contains "$prepare_output" "next=boot Pi 4 with HDMI and physical USB keyboard, then fill evidence" "physical-proof next action after skipped qemu"
expect_contains "$no_backup_output" "media_prepare=ok" "no-backup media prepare status"
expect_contains "$no_backup_output" "sha256=$source_sha" "no-backup installed sha"
expect_contains "$no_backup_output" "evidence_seed=$tmp_evidence_no_backup" "no-backup evidence path"
expect_contains "$no_backup_output" "warning=completed evidence validation requires qemu_smoke=passed; rerun without --skip-qemu before physical proof" "no-backup skip-qemu completion warning"
expect_contains "$no_backup_output" "next=rerun prepare-boot-media without --skip-qemu before physical proof" "no-backup skip-qemu next action"
expect_contains "$auto_output" "media_prepare=ok" "auto media prepare status"
expect_contains "$auto_output" "bootfs=$tmp_bootfs_auto" "auto resolved bootfs"
expect_contains "$auto_output" "sha256=$source_sha" "auto installed sha"
expect_contains "$auto_output" "evidence_seed=$tmp_evidence_auto" "auto evidence path"
expect_contains "$auto_output" "warning=completed evidence validation requires qemu_smoke=passed; rerun without --skip-qemu before physical proof" "auto skip-qemu completion warning"
expect_contains "$auto_output" "next=rerun prepare-boot-media without --skip-qemu before physical proof" "auto skip-qemu next action"

evidence="$(cat "$tmp_evidence")"
no_backup_evidence="$(cat "$tmp_evidence_no_backup")"
auto_evidence="$(cat "$tmp_evidence_auto")"
media_section_count="$(grep -c '^## Media Preparation$' "$tmp_evidence")"
if [ "$media_section_count" -ne 1 ]; then
    printf 'error: expected one media preparation section, found %s\n' "$media_section_count" >&2
    exit 1
fi
media_line="$(grep -n '^## Media Preparation$' "$tmp_evidence" | cut -d: -f1)"
boot_line="$(grep -n '^## Boot Evidence$' "$tmp_evidence" | cut -d: -f1)"
if [ "$media_line" -ge "$boot_line" ]; then
    printf 'error: media preparation section must appear before boot evidence\n' >&2
    exit 1
fi
expect_contains "$evidence" "- Image SHA-256: $source_sha" "evidence image sha"
expect_contains "$evidence" "- Image install command: apps/os/targets/raspi4b-aarch64/install-image.sh $tmp_bootfs" "evidence actual install command"
expect_contains "$evidence" "- Preflight command: apps/os/targets/raspi4b-aarch64/preflight-real-board.sh --bootfs $tmp_bootfs --evidence $tmp_root/.evidence.md.tmp." "evidence actual preflight command"
expect_contains "$evidence" "--skip-qemu" "evidence qemu skip argument"
expect_contains "$evidence" "- Preflight warning: completed evidence requires qemu_smoke=passed; rerun without --skip-qemu before physical proof." "evidence skipped-qemu warning"
expect_contains "$no_backup_evidence" "- Image install command: apps/os/targets/raspi4b-aarch64/install-image.sh --no-backup $tmp_bootfs_no_backup" "no-backup evidence actual install command"
expect_contains "$no_backup_evidence" "- Preflight command: apps/os/targets/raspi4b-aarch64/preflight-real-board.sh --bootfs $tmp_bootfs_no_backup --evidence $tmp_root/.evidence-no-backup.md.tmp." "no-backup evidence actual preflight command"
expect_contains "$no_backup_evidence" "--skip-qemu" "no-backup evidence qemu skip argument"
expect_contains "$auto_evidence" "- Image install command: apps/os/targets/raspi4b-aarch64/install-image.sh $tmp_bootfs_auto" "auto evidence actual install command"
expect_contains "$auto_evidence" "- Preflight command: apps/os/targets/raspi4b-aarch64/preflight-real-board.sh --bootfs $tmp_bootfs_auto --evidence $tmp_root/.evidence-auto.md.tmp." "auto evidence actual preflight command"
expect_contains "$auto_evidence" "--skip-qemu" "auto evidence qemu skip argument"
expect_contains "$evidence" "- Preflight result: preflight=ok qemu_smoke=skipped" "evidence preflight result"
expect_contains "$evidence" "- Boot partition path: $tmp_bootfs" "evidence bootfs"
expect_contains "$evidence" "- [ ] \`package=reovim-os\`" "evidence package fact"
expect_contains "$evidence" "- [ ] \`/boot/image\` reports a \`version=\` line." "evidence version fact"
expect_contains "$evidence" "- [ ] \`selected_profile=shell-only\`" "evidence selected profile fact"
expect_contains "$evidence" "- [ ] \`profile_request=shell-only\`" "evidence profile request fact"
expect_contains "$evidence" "- [ ] \`launch_profile_feature=disabled\`" "evidence launch-profile feature fact"
expect_contains "$evidence" "help" "evidence help command"
expect_contains "$evidence" "help clear" "evidence targeted clear help command"
expect_contains "$evidence" "help screentest" "evidence targeted screentest help command"
expect_contains "$evidence" "help input" "evidence targeted input help command"
expect_contains "$evidence" "help proof" "evidence targeted proof help command"
expect_contains "$evidence" "help pwd" "evidence targeted pwd help command"
expect_contains "$evidence" "help ls" "evidence targeted ls help command"
expect_contains "$evidence" "help cd" "evidence targeted cd help command"
expect_contains "$evidence" "help cat" "evidence targeted cat help command"
expect_contains "$evidence" "help mount" "evidence targeted mount help command"
expect_contains "$evidence" "help device" "evidence targeted device help command"
expect_contains "$evidence" "help dmesg" "evidence targeted dmesg help command"
expect_contains "$evidence" "help status" "evidence targeted status help command"
expect_contains "$evidence" "help probe" "evidence targeted probe help command"
expect_contains "$evidence" "help launch" "evidence targeted launch help command"
expect_contains "$evidence" "help reovim" "evidence targeted reovim help command"
expect_contains "$evidence" "help halt" "evidence targeted halt help command"
expect_contains "$evidence" "clear" "evidence clear command"
expect_contains "$evidence" "screentest" "evidence screentest command"
expect_contains "$evidence" "pwd" "evidence pwd command"
expect_contains "$evidence" "ls /boot" "evidence boot ls command"
expect_contains "$evidence" "ls /dev" "evidence dev ls command"
expect_contains "$evidence" "ls /log" "evidence log ls command"
expect_contains "$evidence" "cat /boot/mounts" "evidence VFS mounts command"
expect_contains "$evidence" $'device\ncat /boot/memory' "evidence device command"
expect_contains "$evidence" "cat /boot/memory" "evidence VFS memory command"
expect_contains "$evidence" "cat /boot/devices" "evidence VFS devices command"
expect_contains "$evidence" "cd /dev" "evidence cd dev command"
expect_contains "$evidence" $'cd /dev\npwd\nls\ncat uart0' "evidence relative ls command"
expect_contains "$evidence" "cat uart0" "evidence relative UART device command"
expect_contains "$evidence" "cd /" "evidence cd root command"
expect_contains "$evidence" "\`help\` / \`help clear\` / \`help screentest\` / \`clear\` / \`screentest\` prove shell help and erase-line mode diagnostics." "evidence shell usability fact"
expect_contains "$evidence" "\`help device\` / \`help launch\` / \`help reovim\` / \`help halt\` make boot inventory, payload, and shutdown commands self-describing." "evidence remaining command help fact"
expect_contains "$evidence" "\`pwd\` / \`ls\` / \`mount\` report the kernel VFS namespace and mounts." "evidence VFS namespace fact"
expect_contains "$evidence" "\`device\` / \`cat /boot/memory\` / \`cat /boot/devices\` report boot inventory." "evidence boot inventory fact"
expect_contains "$evidence" "\`cd /dev\` / \`ls\` / \`cat uart0\` prove relative VFS device-file access." "evidence relative device fact"
expect_contains "$evidence" "cat /boot/proof" "evidence VFS proof command"
expect_contains "$evidence" "cat /boot/help" "evidence VFS help command"
expect_contains "$evidence" "\`cat /boot/proof\` prints the VFS-backed proof pseudo-file." "evidence VFS proof fact"
expect_contains "$evidence" "\`cat /boot/help\` prints the VFS-backed detailed help catalog." "evidence VFS help fact"
expect_contains "$evidence" "\`help dmesg\` / \`ls /log\` make kernel log stats discoverable." "evidence log stats discovery fact"
expect_contains "$evidence" "\`help input\` / \`help proof\` make live input diagnostics and the proof checklist self-describing." "evidence input/proof targeted help fact"
expect_contains "$evidence" "\`help status\` / \`help probe\` make \`manual_next\` and probe catalog discovery self-describing." "evidence targeted help fact"
expect_contains "$evidence" "\`help pwd\` / \`help ls\` / \`help cd\` / \`help cat\` / \`help mount\` make VFS navigation self-describing." "evidence VFS targeted help fact"
expect_contains "$evidence" "cat /boot/status" "evidence VFS status command"
expect_contains "$evidence" "cat /boot/input" "evidence VFS input command"
expect_contains "$evidence" "cat /boot/probes" "evidence VFS probe catalog command"
expect_contains "$evidence" "launch" "evidence launch command"
expect_contains "$evidence" "reovim" "evidence reovim command"
expect_contains "$evidence" "dmesg --stats" "evidence dmesg stats command"
expect_contains "$evidence" "cat /log/stats" "evidence VFS log stats command"
expect_contains "$evidence" "## Shutdown Check" "evidence shutdown section"
expect_contains "$evidence" "halt" "evidence halt command"
expect_contains "$evidence" "\`usb_keyboard_last_poll=report-ready\`" "evidence USB report-ready fact"
expect_contains "$evidence" "\`dmesg\` contains \`input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready\`." "evidence USB readiness dmesg fact"
expect_contains "$evidence" "\`manual_next=type-shell-command\`" "evidence manual next fact"
expect_contains "$evidence" "\`status\` / \`cat /boot/status\` report image/profile identity and live ready source diagnostics." "evidence status identity/live diagnostics fact"
expect_contains "$evidence" "\`input\` / \`cat /boot/input\` report live input diagnostics." "evidence input live diagnostics fact"
expect_contains "$evidence" "\`cat /boot/probes\` prints the VFS-backed probe catalog." "evidence VFS probe catalog fact"
expect_contains "$evidence" "probe pcie" "evidence PCIe probe command"
expect_contains "$evidence" "\`probe pcie\` reports the read-only PCIe/xHCI state." "evidence PCIe probe fact"
expect_contains "$evidence" "\`cat /boot/profile\` reports \`profile=shell-only\`, \`launch=disabled\`, \`payloads=0\`, and \`input_mode=live\`." "evidence profile identity fact"
expect_contains "$evidence" "\`launch\` / \`reovim\` report shell-only payload launch disabled." "evidence shell-only launch-disabled fact"
expect_contains "$evidence" "\`dmesg --stats\` / \`cat /log/stats\` report kernel log ring stats with \`dropped_bytes=0\`." "evidence log stats fact"
expect_contains "$evidence" "\`dmesg\` has no \`[klog] dropped_bytes=\` retained-log wrap marker." "evidence retained dmesg no-wrap-marker fact"
expect_contains "$evidence" "\`shell.status=error\` audit lines for the disabled payload launch commands." "evidence disabled status audit fact"
expect_contains "$evidence" "\`halt\` was typed last and printed \`halt: ok\`." "evidence halt fact"
expect_contains "$evidence" "No new \`reovim-os>\` prompt appeared after \`halt: ok\`." "evidence halt no-prompt fact"
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
