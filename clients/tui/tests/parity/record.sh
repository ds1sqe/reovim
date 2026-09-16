#!/usr/bin/env bash
# What nvim does with a case: `out` and `exit` written beside its `in`
# and `keys`. These flags are the definition of "what nvim does".
#
#   clients/tui/tests/parity/record.sh CASE...     directories
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
for case in "$@"; do
    case="${case%/}"
    [[ -f "$case/in" && -f "$case/keys" ]] \
        || { echo "record.sh: $case needs in and keys" >&2; exit 1; }
    work="$(mktemp -d)"
    cp "$case/in" "$work/file"
    # The keys as a Vim string: `<` escaped, so `<Esc>` is `\<Esc>`.
    keys="$(sed 's/\\/\\\\/g; s/"/\\"/g; s/</\\</g' "$case/keys")"
    set +e
    nvim --headless --clean -i NONE -n "$work/file" \
        -c "call feedkeys(\"$keys\", 'tx')" -c 'cquit 2' 2>/dev/null
    code=$?
    set -e
    if [[ "$code" == 2 ]]; then
        rm -rf "$work"
        echo "record.sh: $case did not quit; keys must end in a quit" >&2
        exit 1
    fi
    cp "$work/file" "$case/out"
    printf '%s\n' "$code" >"$case/exit"
    rm -rf "$work"
    printf 'record.sh: %s exit %s, %s bytes\n' "$case" "$code" \
        "$(wc -c <"$case/out")"
done
