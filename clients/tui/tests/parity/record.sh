#!/usr/bin/env bash
# What nvim does with a case: `out`, `exit` and `state` written beside
# its `in` and `keys`; `state` is the cursor as line:column:cell (the
# column in chars, the cell on the screen, all from 1), the unnamed
# register's type (v, V, ^V{width} or empty) and its bytes at exit.
# These flags are the definition of "what nvim does".
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
    # The state at exit. Through --cmd: a -c before the feedkeys one
    # would leave the cursor at column 1, where nvim itself starts it
    # on the first non-blank.
    state="$work/state"
    # The cell the cursor is shown at: a tab's last, any other's first.
    cell="(strpart(getline('.'), col('.') - 1, 1) ==# \"\\t\" \
        ? virtcol('.', 1)[1] : virtcol('.', 1)[0])"
    leave="autocmd VimLeavePre * call writefile([line('.') . ':' \
        . charcol('.') . ':' . $cell, getregtype('\"')], '$state') \
        | call writefile(split(getreg('\"'), \"\\n\", 1), '$state', 'ab')"
    set +e
    (cd "$work" && nvim --headless --clean -i NONE -n file --cmd "$leave" \
        -c "call feedkeys(\"$keys\", 'tx')" -c 'cquit 2' 2>/dev/null)
    code=$?
    set -e
    if [[ "$code" == 2 ]]; then
        rm -rf "$work"
        echo "record.sh: $case did not quit; keys must end in a quit" >&2
        exit 1
    fi
    cp "$work/file" "$case/out"
    cp "$state" "$case/state"
    printf '%s\n' "$code" >"$case/exit"
    rm -rf "$work"
    printf 'record.sh: %s exit %s, %s bytes\n' "$case" "$code" \
        "$(wc -c <"$case/out")"
done
