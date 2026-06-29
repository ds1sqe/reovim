#!/usr/bin/env bash
# Extract a live `dump snapshot` artifact from a completed Pi 4 evidence file.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"
ANALYZER="$TARGET_DIR/analyze-dump.sh"

OUTPUT=""
ANALYZE=0
EVIDENCE=""

usage() {
    cat <<'USAGE'
Usage: apps/os/targets/raspi4b-aarch64/extract-dump-from-evidence.sh [options] <evidence-file>

Options:
  --output FILE  Write the extracted reovim-dump-v1 artifact to FILE.
  --analyze      Run analyze-dump.sh with Pi 4 shell-only defaults.
  -h, --help     Show this help text.

With no --output and no --analyze, the raw dump artifact is written to stdout.
This extracts a copied live transcript artifact; it is not SD-card persistence
proof unless dump sync also reported checked write/read-back storage.
USAGE
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --output)
            shift
            if [ "$#" -eq 0 ]; then
                fail "--output needs a file"
            fi
            OUTPUT="$1"
            ;;
        --analyze)
            ANALYZE=1
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*)
            fail "unknown argument: $1"
            ;;
        *)
            if [ -n "$EVIDENCE" ]; then
                fail "expected one evidence file"
            fi
            EVIDENCE="$1"
            ;;
    esac
    shift
done

if [ -z "$EVIDENCE" ]; then
    usage >&2
    exit 2
fi
if [ ! -f "$EVIDENCE" ]; then
    fail "evidence file does not exist: $EVIDENCE"
fi
if [ ! -s "$EVIDENCE" ]; then
    fail "evidence file is empty: $EVIDENCE"
fi
if [ -n "$OUTPUT" ]; then
    if [ -e "$OUTPUT" ]; then
        fail "output already exists: $OUTPUT"
    fi
    output_parent="$(dirname "$OUTPUT")"
    if [ ! -d "$output_parent" ]; then
        fail "output directory does not exist: $output_parent"
    fi
fi

dump_count="$(grep -cx 'reovim-dump-v1' "$EVIDENCE" || true)"
if [ "$dump_count" -eq 0 ]; then
    fail "no reovim-dump-v1 snapshot found in evidence"
fi
if [ "$dump_count" -gt 1 ]; then
    fail "multiple reovim-dump-v1 snapshots found; extract one manually"
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/reovim-dump-evidence.XXXXXX")"
cleanup() {
    rm -rf "$tmp_dir"
}
trap cleanup EXIT

dump_tmp="$tmp_dir/reovim-dump-v1.txt"
awk '
    $0 == "reovim-dump-v1" {
        in_dump = 1
    }
    in_dump && /^reovim-os> / {
        exit
    }
    in_dump && /^```$/ {
        exit
    }
    in_dump {
        print
    }
' "$EVIDENCE" >"$dump_tmp"

first_line="$(sed -n '1p' "$dump_tmp")"
if [ "$first_line" != "reovim-dump-v1" ]; then
    fail "extracted dump does not start with reovim-dump-v1"
fi
checksum_count="$(grep -Ec '^checksum=[0-9]+$' "$dump_tmp" || true)"
if [ "$checksum_count" -ne 1 ]; then
    fail "extracted dump must contain exactly one checksum row"
fi

dump_path="$dump_tmp"
if [ -n "$OUTPUT" ]; then
    cp "$dump_tmp" "$OUTPUT"
    dump_path="$OUTPUT"
fi

if [ -z "$OUTPUT" ] && [ "$ANALYZE" -eq 0 ]; then
    cat "$dump_tmp"
    exit 0
fi

bytes="$(wc -c <"$dump_path" | tr -d ' ')"
printf 'dump_extract=ok\n'
printf 'source=live-transcript\n'
printf 'persistent_sd_proof=false\n'
printf 'evidence=%s\n' "$EVIDENCE"
printf 'extracted_dump=%s\n' "$dump_path"
printf 'bytes=%s\n' "$bytes"

if [ "$ANALYZE" -eq 1 ]; then
    "$ANALYZER" \
        --expect-package reovim-os \
        --expect-target aarch64-unknown-none \
        --expect-profile shell-only \
        --require-zero-drops \
        "$dump_path"
fi
