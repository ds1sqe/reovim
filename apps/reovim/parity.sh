#!/usr/bin/env bash
# The corpus through the binary in tmux: each case's keys typed into
# `reovim file`, then the file, the exit code and the screen cursor
# compared with what nvim recorded (`out`, `exit`, `state`). Needs
# tmux, no nvim. Cases named in parity.known may differ; the exit
# code is the number of others that do.
#
#   apps/reovim/parity.sh BINARY [CASE...]     default: every case
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
BIN="$(realpath "${1:-target/debug/reovim}")"
shift || true
CORPUS="${PARITY_CORPUS:-clients/tui/tests/parity}"
KNOWN="apps/reovim/parity.known"
DIR="$(mktemp -d /tmp/reovim-parity.XXXXXXXX)"
readonly BIN CORPUS KNOWN DIR
TMUX=(tmux -L "reovim-parity-$$")
# The session of the case under way, one per case on one server.
SESSION=""
cleanup() {
    "${TMUX[@]}" kill-server 2>/dev/null || true
    rm -rf -- "$DIR"
}
trap cleanup EXIT

# Wait up to two seconds for `pattern` on the pane.
shown() {
    local pattern="$1" i
    for ((i = 0; i < 400; i++)); do
        if "${TMUX[@]}" capture-pane -p -t "$SESSION" \
            | grep -q -- "$pattern"; then
            return
        fi
        sleep 0.005
    done
    return 1
}

# One key in Vim notation to tmux: a name, or a literal char.
send() {
    local key="$1"
    case "$key" in
        '<Esc>') "${TMUX[@]}" send-keys -t "$SESSION" Escape; sleep 0.02 ;;
        '<CR>') "${TMUX[@]}" send-keys -t "$SESSION" Enter ;;
        '<BS>') "${TMUX[@]}" send-keys -t "$SESSION" BSpace ;;
        '<Tab>') "${TMUX[@]}" send-keys -t "$SESSION" Tab ;;
        '<Left>' | '<Right>' | '<Up>' | '<Down>' | '<Home>' | '<End>')
            "${TMUX[@]}" send-keys -t "$SESSION" "${key:1:-1}" ;;
        '<C-'?'>') "${TMUX[@]}" send-keys -t "$SESSION" "C-${key:3:1}" ;;
        '<lt>') "${TMUX[@]}" send-keys -t "$SESSION" -l '<' ;;
        *) "${TMUX[@]}" send-keys -t "$SESSION" -l "$key" ;;
    esac
}

# The keys of `text`, one per line: `<Name>` or a char.
tokens() {
    local text="$1"
    while [[ -n "$text" ]]; do
        if [[ "$text" == '<'* && "$text" == *'>'* ]]; then
            printf '%s\n' "${text%%>*}>"
            text="${text#*>}"
        else
            printf '%s\n' "${text:0:1}"
            text="${text:1}"
        fi
    done
}

# The pane's cursor as `row col`, once it has settled.
cursor() {
    sleep 0.05
    "${TMUX[@]}" display -p -t "$SESSION" '#{cursor_y} #{cursor_x}'
}

run_case() {
    local case="$1" name work keys quit token
    name="$(basename "$case")"
    work="$DIR/$name"
    mkdir -p "$work"
    cp "$case/in" "$work/file"
    keys="$(tr -d '\n' <"$case/keys")"
    # The keys end in a quit; the cursor is read before it.
    for quit in ':wq<CR>' ':q!<CR>' ':q<CR>' ':x<CR>' ':cq<CR>' 'ZZ' 'ZQ'; do
        [[ "$keys" == *"$quit" ]] && break
        quit=""
    done
    [[ -n "$quit" ]] || { echo "$name: keys do not end in a quit"; return 1; }
    keys="${keys%"$quit"}"
    SESSION="$name"
    "${TMUX[@]}" new-session -d -s "$SESSION" -x 80 -y 24 -c "$work" \
        "$BIN file; echo \$? > '$work/exit'; sleep 1"
    shown 'file  [0-9]*:[0-9]*  r[0-9]*' \
        || { echo "$name: no frame"; return 1; }
    local run=""
    while IFS= read -r token; do
        if [[ ${#token} -eq 1 ]]; then
            run+="$token"
            continue
        fi
        [[ -z "$run" ]] || "${TMUX[@]}" send-keys -t "$SESSION" -l "$run"
        run=""
        send "$token"
    done < <(tokens "$keys")
    [[ -z "$run" ]] || "${TMUX[@]}" send-keys -t "$SESSION" -l "$run"
    local at
    at="$(cursor)"
    while IFS= read -r token; do send "$token"; done < <(tokens "$quit")
    local i
    for ((i = 0; i < 400; i++)); do
        [[ -f "$work/exit" ]] && break
        sleep 0.005
    done
    [[ -f "$work/exit" ]] || { echo "$name: did not exit"; return 1; }
    local code line vcol expect
    code="$(cat "$work/exit")"
    IFS=: read -r line _ vcol <"$case/state"
    expect="$((line - 1)) $((vcol - 1))"
    local bad=""
    cmp -s "$work/file" "$case/out" || bad+=" file"
    [[ "$code" == "$(cat "$case/exit")" ]] || bad+=" exit($code)"
    [[ "$at" == "$expect" ]] || bad+=" cursor($at, nvim $expect)"
    [[ -z "$bad" ]] || { echo "$name:$bad"; return 1; }
}

failed=0
unknown=0
[[ $# -gt 0 ]] || set -- "$CORPUS"/*/
for case in "$@"; do
    case="${case%/}"
    [[ -d "$case" ]] || case="$CORPUS/$case"
    if ! run_case "$case"; then
        failed=$((failed + 1))
        if ! grep -qx "$(basename "$case")" "$KNOWN" 2>/dev/null; then
            unknown=$((unknown + 1))
        fi
    fi
done
total="$(ls -d "$CORPUS"/*/ | wc -l)"
printf 'parity.sh: %s of %s cases differ from nvim, %s not known\n' \
    "$failed" "$total" "$unknown"
exit "$unknown"
